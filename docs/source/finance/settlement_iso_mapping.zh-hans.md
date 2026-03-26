---
lang: zh-hans
direction: ltr
source: docs/source/finance/settlement_iso_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1f1005d6a273ab732a7c7a7adca349c17569fe2e2755b8daccf2186724044f8
source_last_modified: "2026-01-22T16:26:46.568382+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 结算 ↔ ISO 20022 字段映射

本注释捕获了 Iroha 结算指令之间的规范映射
（`DvpIsi`、`PvpIsi`、回购抵押品流量）和执行的 ISO 20022 消息
在桥边。它反映了在中实现的消息脚手架
`crates/ivm/src/iso20022.rs` 并作为生产或时的参考
验证 Norito 有效负载。

### 参考数据政策（标识符和验证）

该策略打包了标识符首选项、验证规则和参考数据
Norito ↔ ISO 20022 网桥在发出消息之前必须执行的义务。

**ISO 消息内的锚点：**
- **仪器标识符** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  （或同等仪器领域）。
- **当事方/代理人** → `DlvrgSttlmPties/Pty` 和 `RcvgSttlmPties/Pty` 为 `sese.*`，
  或 `pacs.009` 中的代理结构。
- **账户** → `…/Acct` 保管/现金账户要素；镜像账本
  `AccountId` 中的 `SupplementaryData`。
- **专有标识符** → `…/OthrId` 和 `Tp/Prtry` 并镜像
  `SupplementaryData`。切勿用专有标识符替换受监管的标识符。

#### 消息系列的标识符首选项

##### `sese.023` / `.024` / `.025`（证券结算）

- **仪器 (`FinInstrmId`)**
  - 首选：`…/ISIN` 下的 **ISIN**。它是 CSD / T2S 的规范标识符。[^anna]
  - 后备方案：
    - **CUSIP** 或 `…/OthrId/Id` 下的其他 NSIN，其中 `Tp/Cd` 从 ISO 外部设置
      代码列表（例如，`CUSP`）；必要时将发行人包含在 `Issr` 中。[^iso_mdr]
    - **Norito 资产 ID** 作为专有：`…/OthrId/Id`、`Tp/Prtry="NORITO_ASSET_ID"` 和
      在 `SupplementaryData` 中记录相同的值。
  - 可选描述符：**CFI** (`ClssfctnTp`) 和 **FISN**（支持缓解）
    和解。[^iso_cfi][^iso_fisn]
- **各方 (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - 首选：**BIC**（`AnyBIC/BICFI`，ISO 9362）。[^swift_bic]
  - 后备：**LEI**，其中消息版本公开专用 LEI 字段；如果
    不存在，携带带有清晰 `Prtry` 标签的专有 ID，并在元数据中包含 BIC。[^iso_cr]
- **定居点/场地** → **MIC** 代表场地，**BIC** 代表 CSD。[^iso_mic]

##### `colr.010` / `.011` / `.012` 和 `colr.007`（抵押品管理）

- 遵循与 `sese.*` 相同的仪器规则（首选 ISIN）。
- 各方默认使用**BIC**； **LEI** 在架构公开的情况下是可接受的。[^swift_bic]
- 现金金额必须使用 **ISO 4217** 货币代码和正确的小单位。[^iso_4217]

##### `pacs.009` / `camt.054`（PvP 资金和报表）- **代理人（`InstgAgt`、`InstdAgt`、债务人/债权人代理人）** → **BIC** 可选
  LEI 在允许的情况下。[^swift_bic]
- **账户**
  - 银行间：通过 **BIC** 和内部账户参考进行识别。
  - 面向客户的声明 (`camt.054`)：包括 **IBAN**（如果存在）并进行验证
    （长度、国家/地区规则、mod-97 校验和）。[^swift_iban]
- **货币** → **ISO 4217** 3 个字母代码，尊重小单位舍入。[^iso_4217]
- **Torii 摄取** → 通过 `POST /v1/iso20022/pacs009` 提交 PvP 资金；桥
  需要 `Purp=SECU`，现在在配置参考数据时强制执行 BIC 人行横道。

#### 验证规则（在发射前应用）

|标识符 |验证规则 |笔记|
|------------|-----------------|--------|
| **ISIN** |正则表达式 `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` 和 Luhn (mod-10) 校验位符合 ISO 6166 附录 C |桥接发射前拒绝；更喜欢上游富集。[^anna_luhn] |
| **CUSIP** |正则表达式 `^[A-Z0-9]{9}$` 和模数 10，权重为 2（字符映射到数字）|仅当 ISIN 不可用时；获取后通过 ANNA/CUSIP 人行横道绘制地图。[^cusip] |
| **雷** |正则表达式 `^[A-Z0-9]{18}[0-9]{2}$` 和 mod-97 校验位 (ISO 17442) |在接受之前对照 GLEIF 每日增量文件进行验证。[^gleif] |
| **BIC** |正则表达式 `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` |可选的分支代码（最后三个字符）。确认 RA 文件中的活动状态。[^swift_bic] |
| **麦克风** |根据 ISO 10383 RA 文件进行维护；确保场馆处于活动状态（无 `!` 终止标志）|在发射前标记退役的 MIC。[^iso_mic] |
| **国际银行账号** |国家/地区特定长度，大写字母数字，mod-97 = 1 |使用 SWIFT 维护的注册表；拒绝结构上无效的 IBAN。[^swift_iban] |
| **专有帐户/方 ID** | `Max35Text`（UTF-8，≤35 个字符），带有修剪的空格 |适用于 `GenericAccountIdentification1.Id` 和 `PartyIdentification135.Othr/Id` 字段。拒绝超过 35 个字符的条目，以便桥接有效负载符合 ISO 架构。 |
| **代理帐户标识符** | `…/Prxy/Id` 下的非空 `Max2048Text`，`…/Prxy/Tp/{Cd,Prtry}` 中具有可选类型代码 |与主要 IBAN 一起存储；验证仍然需要 IBAN，同时接受代理句柄（带有可选类型代码）以镜像 PvP 轨道。 |
| **CFI** |六字符代码，使用 ISO 10962 分类法的大写字母 |可选的丰富；确保字符与仪器类别匹配。[^iso_cfi] |
| **FISN** |最多 35 个字符，大写字母数字加有限标点符号 |选修的;根据 ISO 18774 指南进行截断/标准化。[^iso_fisn] |
| **货币** | ISO 4217 3 字母代码，由小单位确定的比例 |金额必须四舍五入到允许的小数位；在 Norito 端强制执行。[^iso_4217] |

#### 人行横道和数据维护义务- 维护 **ISIN ↔ Norito 资产 ID** 和 **CUSIP ↔ ISIN** 人行横道。每晚更新自
  ANNA/DSB 提供 CI 使用的快照并进行版本控制。[^anna_crosswalk]
- 刷新 GLEIF 公共关系文件中的 **BIC ↔ LEI** 映射，以便桥接器可以
  需要时同时发出。[^bic_lei]
- 将 **MIC 定义** 与桥元数据一起存储，以便场地验证
  即使 RA 文件在中午发生变化，也具有确定性。[^iso_mic]
- 在桥元数据中记录数据来源（时间戳+来源）以供审核。坚持
  快照标识符与发出的指令一起。
- 配置 `iso_bridge.reference_data.cache_dir` 以保留每个加载的数据集的副本
  以及出处元数据（版本、来源、时间戳、校验和）。这使得审核员
  即使在上游快照轮换之后，操作员也可以区分历史源。
- ISO 人行横道快照由 `iroha_core::iso_bridge::reference_data` 使用
  `iso_bridge.reference_data` 配置块（路径 + 刷新间隔）。仪表
  `iso_reference_status`、`iso_reference_age_seconds`、`iso_reference_records` 和
  `iso_reference_refresh_interval_secs` 公开运行时运行状况以进行警报。 Torii
  网桥拒绝其代理 BIC 不存在于配置中的 `pacs.008` 提交
  人行横道，当对手方处于
  未知。【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN 和 ISO 4217 绑定在同一层强制执行：现在 pacs.008/pacs.009 流
  当债务人/债权人 IBAN 缺少配置的别名或当
  `currency_assets` 中缺少结算货币，防止桥接畸形
  到达分类账的指令。 IBAN 验证也适用于特定国家/地区
  ISO 7064 mod-97 通过之前的长度和数字校验位，因此在结构上无效
  值被提前拒绝。【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- CLI结算助手继承相同的护栏：通过
  `--iso-reference-crosswalk <path>` 与 `--delivery-instrument-id` 一起获得 DvP
  在发出 `sese.023` XML 快照之前预览验证仪器 ID。【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint`（和 CI 包装器 `ci/check_iso_reference_data.sh`）lint
  人行横道快照和固定装置。该命令接受 `--isin`、`--bic-lei`、`--mic` 和
  运行时，`--fixtures` 标记并回退到 `fixtures/iso_bridge/` 中的示例数据集
  不带参数。【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM 帮助程序现在摄取真正的 ISO 20022 XML 信封（head.001 + `DataPDU` + `Document`）
  并通过 `head.001` 架构验证业务应用程序标头，因此 `BizMsgIdr`，
  `MsgDefIdr`、`CreDt` 和 BIC/ClrSysMmbId 代理被确定性地保留； XMLDSig/XAdES
  块仍然被故意跳过。回归测试消耗样本和新的用于保护映射的标头信封固定装置。【crates/ivm/src/iso20022.rs:265】【crates/ivm/src/iso20022.rs:3301】【crates/ivm/src/iso20022.rs:3703】

#### 监管和市场结构考虑因素

- **T+1结算**：美国/加拿大股票市场于2024年转向T+1；调整 Norito
  相应地安排和 SLA 警报。[^sec_t1][^csa_t1]
- **CSDR 处罚**：和解纪律规则强制执行现金处罚；确保 Norito
  元数据捕获用于调节的惩罚参考。[^csdr]
- **当日结算试点**：印度监管机构正在逐步推行T0/T+0结算；保留
  随着试点范围的扩大，桥梁日历也随之更新。[^india_t0]
- **抵押买入/持有**：监控 ESMA 关于买入时间表和可选持有的更新
  因此有条件交付 (`HldInd`) 与最新指南保持一致。[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### 货到付款 → `sese.023`| DvP 领域 | ISO 20022 路径 |笔记|
|--------------------------------------------------------------------|----------------------------------------|------|
| `settlement_id` | `TxId` |稳定的生命周期标识符 |
| `delivery_leg.asset_definition_id`（安全）| `SctiesLeg/FinInstrmId` |规范标识符（ISIN、CUSIP……）|
| `delivery_leg.quantity` | `SctiesLeg/Qty` |十进制字符串；荣誉资产精准 |
| `payment_leg.asset_definition_id`（货币）| `CashLeg/Ccy` | ISO 货币代码 |
| `payment_leg.quantity` | `CashLeg/Amt` |十进制字符串；根据数字规范进行四舍五入 |
| `delivery_leg.from`（卖家/送货方）| `DlvrgSttlmPties/Pty/Bic` |交付参与者的 BIC *（帐户规范 ID 目前在元数据中导出）* |
| `delivery_leg.from` 帐户标识符 | `DlvrgSttlmPties/Acct` |自由形式； Norito 元数据携带准确的帐户 ID |
| `delivery_leg.to`（买方/接收方）| `RcvgSttlmPties/Pty/Bic` |接收参与者的 BIC |
| `delivery_leg.to` 帐户标识符 | `RcvgSttlmPties/Acct` |自由形式；匹配接收帐户 ID |
| `plan.order` | `Plan/ExecutionOrder` |枚举：`DELIVERY_THEN_PAYMENT` 或 `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` |枚举：`ALL_OR_NOTHING`、`COMMIT_FIRST_LEG`、`COMMIT_SECOND_LEG` |
| **留言目的** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI`（发送）或 `RECE`（接收）；反映提交方执行的分支。 |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT`（付款）或 `FREE`（免付款）。 |
| `delivery_leg.metadata`，`payment_leg.metadata` | `SctiesLeg/Metadata`、`CashLeg/Metadata` |可选 Norito JSON 编码为 UTF-8 |

> **结算限定符** – 桥梁通过将结算条件代码 (`SttlmTxCond`)、部分结算指标 (`PrtlSttlmInd`) 以及 Norito 元数据中的其他可选限定符复制到 `sese.023/025`（如果存在）来反映市场惯例。强制执行 ISO 外部代码列表中发布的枚举，以便目标 CSD 识别这些值。

### 支付与支付资金 → `pacs.009`

为 PvP 指令提供资金的现金换现金部分以 FI-to-FI 信用形式发放
转移。该桥对这些付款进行了注释，以便下游系统识别
他们为证券结算提供资金。| PvP 资金领域 | ISO 20022 路径 |笔记|
|------------------------------------------------|--------------------------------------------------------|--------------------|
| `primary_leg.quantity` / {金额，货币} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` |从发起人处扣除的金额/货币。 |
|交易对手代理标识符 | `InstgAgt`，`InstdAgt` |发送和接收代理的 BIC/LEI。 |
|落户目的 | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` |设置为 `SECU` 用于证券相关的 PvP 资金。 |
| Norito 元数据（账户 ID、FX 数据）| `CdtTrfTxInf/SplmtryData` |包含完整的 AccountId、FX 时间戳、执行计划提示。 |
|指令标识符/生命周期链接| `CdtTrfTxInf/PmtId/InstrId`、`CdtTrfTxInf/RmtInf` |与 Norito `settlement_id` 相匹配，以便现金部分与证券方保持一致。 |

JavaScript SDK 的 ISO 桥通过默认的
`pacs.009` 类别目的为 `SECU`；调用者可以用另一个覆盖它
发出非证券信用转账时有效的 ISO 代码，但无效
值被预先拒绝。

如果基础设施需要明确的证券确认，那么桥梁
继续发出 `sese.025`，但该确认反映了证券腿
状态（例如，`ConfSts = ACCP`）而不是 PvP“目的”。

### 付款与付款确认 → `sese.025`

| PvP 领域 | ISO 20022 路径 |笔记|
|------------------------------------------------------------|----------------------------------------|------|
| `settlement_id` | `TxId` |稳定的生命周期标识符 |
| `primary_leg.asset_definition_id` | `SttlmCcy` |主要支线的货币代码 |
| `primary_leg.quantity` | `SttlmAmt` |发起人交付的金额 |
| `counter_leg.asset_definition_id` | `AddtlInf`（JSON 有效负载）|补充信息中嵌入的柜台货币代码 |
| `counter_leg.quantity` | `SttlmQty` |柜台金额|
| `plan.order` | `Plan/ExecutionOrder` |与 DvP | 相同的枚举集
| `plan.atomicity` | `Plan/Atomicity` |与 DvP | 相同的枚举集
| `plan.atomicity` 状态 (`ConfSts`) | `ConfSts` |匹配时为 `ACCP`；桥在拒绝​​时发出故障代码 |
|交易对手标识符| `AddtlInf` JSON |当前桥在元数据中序列化完整的 AccountId/BIC 元组 |

示例（具有链接、保留和市场 MIC 的 CLI ISO 预览）：

```sh
iroha app settlement dvp \
  --settlement-id DVP-FIXTURE-1 \
  --delivery-asset security#equities \
  --delivery-quantity 500 \
  --delivery-from <katakana-i105-account-id> \
  --delivery-to <katakana-i105-account-id> \
  --payment-asset usd#fi \
  --payment-quantity 1050000 \
  --payment-from <katakana-i105-account-id> \
  --payment-to <katakana-i105-account-id> \
  --delivery-instrument-id US0378331005 \
  --place-of-settlement-mic XNAS \
  --partial-indicator npar \
  --hold-indicator \
  --settlement-condition NOMC \
  --linkage WITH:PACS009-CLS \
  --linkage BEFO:SUBST-PAIR-B \
  --iso-xml-out sese023_preview.xml
```

### 回购抵押品替代 → `colr.007`|回购字段/上下文 | ISO 20022 路径 |笔记|
|------------------------------------------------|------------------------------------------------|--------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` |回购合约标识符 |
|抵押品替代交易标识符 | `TxId` |每次替换生成 |
|原始抵押品数量 | `Substitution/OriginalAmt` |比赛在替换前承诺抵押品|
|原始抵押币 | `Substitution/OriginalCcy` |货币代码 |
|替代抵押品数量 | `Substitution/SubstituteAmt` |更换金额 |
|替代抵押货币 | `Substitution/SubstituteCcy` |货币代码 |
|生效日期（治理保证金时间表）| `Substitution/EffectiveDt` | ISO 日期 (YYYY-MM-DD) |
|理发分类| `Substitution/Type` |目前基于治理策略的 `FULL` 或 `PARTIAL` |
|治理/理发注意| `Substitution/ReasonCd` |可选，承载治理原理|
|理发尺寸| `Substitution/Haircut` |数字;映射替换期间应用的发型 |
|原始/替代仪器 ID | `Substitution/OriginalFinInstrmId`、`Substitution/SubstituteFinInstrmId` |每条腿可选 ISIN/CUSIP |

### 资金和报表

| Iroha 上下文 | ISO 20022 消息 |地图位置 |
|----------------------------------|--------------------------------|------------------|
|回购现金腿点火/解除| `pacs.009` | `IntrBkSttlmAmt`、`IntrBkSttlmCcy`、`IntrBkSttlmDt`、`InstgAgt`、`InstdAgt` 从 DvP/PvP 分支填充 |
|结算后报表| `camt.054` |支付腿移动记录在 `Ntfctn/Ntry[*]` 下；桥在 `SplmtryData` 中注入账本/账户元数据 |

### 使用说明* 所有金额均使用 Norito 数字助手 (`NumericSpec`) 进行序列化
  确保资产定义之间的规模一致性。
* `TxId` 值为 `Max35Text` — 强制 UTF-8 长度≤35 个字符
  导出为 ISO 20022 消息。
* BIC 必须是 8 或 11 个大写字母数字字符 (ISO9362)；拒绝
  Norito 在发出付款或结算之前未通过此检查的元数据
  确认。
* 账户标识符（AccountId / ChainId）导出到补充中
  元数据，以便接收参与者可以根据其本地分类账进行核对。
* `SupplementaryData` 必须是规范的 JSON（UTF-8、排序键、JSON 原生
  逃跑）。 SDK 帮助程序强制执行此操作，以便签名、遥测哈希和 ISO
  有效负载档案在重建过程中保持确定性。
* 货币金额遵循 ISO4217 小数位（例如 JPY 为 0
  小数点，美元有 2);桥相应地钳位 Norito 数字精度。
* CLI 结算助手 (`iroha app settlement ... --atomicity ...`) 现在发出
  Norito 指令，其执行计划以 1:1 映射到 `Plan/ExecutionOrder`，并且
  `Plan/Atomicity` 以上。
* ISO 助手 (`ivm::iso20022`) 验证上面列出的字段并拒绝
  DvP/PvP 分支违反数字规范或交易对手互惠的消息。

### SDK 构建器助手

- JavaScript SDK 现在公开 `buildPacs008Message` /
  `buildPacs009Message`（参见 `javascript/iroha_js/src/isoBridge.js`）所以客户端
  自动化可以转换结构化结算元数据（BIC/LEI、IBAN、
  目的代码、补充 Norito 字段）转换为确定性 pacs XML
  无需重新实现本指南中的映射规则。
- 两个助手都需要明确的 `creationDateTime`（带时区的 ISO-8601）
  因此操作员必须从其工作流程中线程化确定性时间戳
  让 SDK 默认为挂钟时间。
- `recipes/iso_bridge_builder.mjs` 演示了如何将这些助手连接到
  合并环境变量或 JSON 配置文件的 CLI，打印
  生成的 XML，并可选择将其提交给 Torii (`ISO_SUBMIT=1`)，重用
  与 ISO 桥配方相同的等待节奏。


### 参考文献

- LuxCSD / Clearstream ISO 20022 结算示例显示 `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) 和 `Pmt` (`APMT`/`FREE`)。[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)
- Clearstream DCP 规范涵盖结算限定符（`SttlmTxCond`、`PrtlSttlmInd`）。[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)
- SWIFT PMPG 指南建议将 `pacs.009` 和 `CtgyPurp/Cd = SECU` 用于证券相关的 PvP 融资。[3](https://www.swift.com/swift-resource/251897/download)
- 针对标识符长度限制的 ISO 20022 消息定义报告（BIC、Max35Text）。[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)
- ANNA DSB 有关 ISIN 格式和校验和规则的指南。[5](https://www.anna-dsb.com/isin/)

### 使用技巧- 始终粘贴相关的 Norito 片段或 CLI 命令，以便 LLM 可以检查
  准确的字段名称和数字比例。
- 请求引用 (`provide clause references`) 以保留书面记录
  合规性和审计师审查。
- 在 `docs/source/finance/settlement_iso_mapping.md` 中捕获答案摘要
  （或链接的附录），这样未来的工程师就不需要重复查询。

## 事件排序手册（ISO 20022 ↔ Norito Bridge）

### 场景 A — 抵押品替代（回购/质押）

**参与者：** 抵押品给予者/接受者（和/或代理人）、托管人、CSD/T2S  
**时间：** 每个市场截止时间和 T2S 日/夜周期；协调两条腿，使它们在同一个结算窗口内完成。

#### 消息编排
1. `colr.010` 抵押品替代请求 → 抵押品给予者/接受者或代理人。  
2. `colr.011` 抵押替代响应 → 接受/拒绝（可选拒绝原因）。  
3. `colr.012` 抵押品替代确认 → 确认替代协议。  
4、`sese.023`指令（两条腿）：  
   - 返还原始抵押品（`SctiesMvmntTp=DELI`、`Pmt=FREE`、`SctiesTxTp=COLO`）。  
   - 交付替代抵押品（`SctiesMvmntTp=RECE`、`Pmt=FREE`、`SctiesTxTp=COLI`）。  
   链接该对（见下文）。  
5. `sese.024` 状态建议（已接受、匹配、待处理、失败、拒绝）。  
6. `sese.025` 预订后确认。  
7. 可选现金增量（费用/折扣） → `pacs.009` 通过 `CtgyPurp/Cd = SECU` 进行 FI 到 FI 信用转账；状态通过 `pacs.002`，通过 `pacs.004` 返回。

#### 所需的确认/状态
- 传输层：网关可能会在业务处理之前发出 `admi.007` 或拒绝。  
- 结算生命周期：`sese.024`（处理状态+原因代码）、`sese.025`（最终）。  
- 现金方面：`pacs.002`（`PDNG`、`ACSC`、`RJCT` 等）、`pacs.004` 用于退货。

#### 条件/展开字段
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) 链接两条指令。  
- `SttlmParams/HldInd` 保留直至满足标准；通过 `sese.030` 发布（`sese.031` 状态）。  
- `SttlmParams/PrtlSttlmInd` 控制部分沉降（`NPAR`、`PART`、`PARC`、`PARQ`）。  
- `SttlmParams/SttlmTxCond/Cd` 适用于市场特定条件（`NOMC` 等）。  
- 可选的 T2S 有条件证券交割 (CoSD) 规则（如果支持）。

#### 参考文献
- SWIFT 抵押品管理 MDR (`colr.010/011/012`)。  
- 用于链接和状态的 CSD/T2S 使用指南（例如 DNB、ECB Insights）。  
- SMPG 结算实践、Clearstream DCP 手册、ASX ISO 研讨会。

### 场景 B — 外汇窗口违规（PvP 资金失败）

**参与者：** 交易对手和现金代理人、证券托管人、CSD/T2S  
**时间安排：** FX PvP 窗口（CLS/双边）和 CSD 截止；在现金确认之前，保持证券交易状态。#### 消息编排
1. `pacs.009` 通过 `CtgyPurp/Cd = SECU` 每种货币进行 FI 到 FI 信用转账；通过 `pacs.002` 获取状态；通过 `camt.056`/`camt.029` 召回/取消；如果已经结算，则返回 `pacs.004`。  
2. `sese.023` 与 `HldInd=true` 的 DvP 指令，因此证券分支等待现金确认。  
3. 生命周期 `sese.024` 通知（已接受/已匹配/待定）。  
4. 如果 `pacs.009` 的两条腿在窗口到期前均达到 `ACSC` → 以 `sese.030` 释放 → `sese.031` (mod 状态) → `sese.025` (确认)。  
5. 如果外汇窗口被突破 → 取消/召回现金（`camt.056/029` 或 `pacs.004`）并取消证券（`sese.020` + `sese.027`，或 `sese.026` 逆转（如果已根据市场规则确认）。

#### 所需的确认/状态
- 现金：`pacs.002`（`PDNG`、`ACSC`、`RJCT`）、`pacs.004` 用于退货。  
- 证券：`sese.024`（待决/失败原因，如 `NORE`、`ADEA`）、`sese.025`。  
- 传输：`admi.007`/网关在业务处理之前拒绝。

#### 条件/展开字段
- `SttlmParams/HldInd` + `sese.030` 成功/失败时释放/取消。  
- `Lnkgs` 将证券指令与现金部分联系起来。  
- T2S CoSD 规则（如果使用有条件交付）。  
- `PrtlSttlmInd` 以防止意外部分。  
- 在 `pacs.009` 上，`CtgyPurp/Cd = SECU` 标记与证券相关的资金。

#### 参考文献
- PMPG / CBPR+ 证券流程支付指南。  
- SMPG 结算实践、关于链接/保留的 T2S 见解。  
- Clearstream DCP 手册、维护消息的 ECMS 文档。

### pacs.004 返回映射注释

- 退货固定装置现在规范化 `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) 和公开为 `TxInf[*]/RtrdRsn/Prtry` 的专有退货原因，因此桥梁消费者可以重播费用归属和运营商代码，而无需重新解析XML 信封。
- `DataPDU` 信封内的 AppHdr 签名块在摄取时仍然被忽略；审计应依赖于渠道来源而不是嵌入的 XMLDSIG 字段。

### 桥梁操作检查表
- 执行上述编排（抵押品：`colr.010/011/012 → sese.023/024/025`；外汇违规：`pacs.009 (+pacs.002) → sese.023 held → release/cancel`）。  
- 将 `sese.024`/`sese.025` 状态和 `pacs.002` 结果视为门控信号； `ACSC` 触发释放，`RJCT` 强制释放。  
- 通过 `HldInd`、`Lnkgs`、`PrtlSttlmInd`、`SttlmTxCond` 和可选 CoSD 规则对有条件交付进行编码。  
- 需要时，使用 `SupplementaryData` 关联外部 ID（例如 `pacs.009` 的 UETR）。  
- 通过市场日历/截止时间参数化持有/平仓时间；在取消截止日期之前发出 `sese.030`/`camt.056`，必要时退回退货。

### ISO 20022 有效负载示例（带注释）

#### 具有指令链接的附带替换对 (`sese.023`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```提交链接指令 `SUBST-2025-04-001-B`（替代抵押品的 FoP 接收），其中包含 `SctiesMvmntTp=RECE`、`Pmt=FREE` 和指向回 `SUBST-2025-04-001-A` 的 `WITH` 链接。一旦替换获得批准，用匹配的 `sese.030` 释放两条腿。

#### 证券腿处于等待外汇确认状态 (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

一旦 `pacs.009` 腿到达 `ACSC` 就释放：

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` 确认解除持有，一旦证券腿被预订，`sese.025` 随后确认。

#### PvP 资金来源（`pacs.009` 具有证券用途）

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` 跟踪付款状态（`ACSC` = 已确认，`RJCT` = 拒绝）。如果窗口被突破，请通过 `camt.056`/`camt.029` 调用或发送 `pacs.004` 以返还已结算资金。