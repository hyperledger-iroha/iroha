---
lang: zh-hans
direction: ltr
source: docs/source/finance/repo_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 42c328443065e102a65180421d515e4e3040a35175c348ea25fd83edab1236b4
source_last_modified: "2026-01-22T16:26:46.567961+00:00"
translation_last_reviewed: 2026-02-07
title: Repo Operations & Evidence Guide
summary: Governance, lifecycle, and audit requirements for repo/reverse-repo flows (roadmap F1).
translator: machine-google-reviewed
---

# 回购操作和证据指南（路线图 F1）

回购计划以确定性方式解锁双边和三方融资
Norito 指令、CLI/SDK 帮助程序和 ISO 20022 奇偶校验。这篇笔记捕捉到了
满足路线图里程碑 **F1 — 回购协议所需的运营合同
生命周期文档和工具**。它补充了面向工作流程的
[`repo_runbook.md`](./repo_runbook.md) 通过阐明：

- 跨 CLI/SDK/运行时的生命周期表面（`crates/iroha_cli/src/main.rs:3821`，
  `python/iroha_python/iroha_python_rs/src/lib.rs:2216`，
  `crates/iroha_core/src/smartcontracts/isi/repo.rs:1`);
- 确定性证明/证据捕获（`integration_tests/tests/repo.rs:1`）；
- 三方托管和抵押品替代行为；和
- 治理期望（双重控制、审计跟踪、回滚手册）。

## 1. 范围和验收标准

路线图项目 F1 仍然有四个主题；该文件列举了
所需的工件以及已满足它们的代码/测试的链接：

|要求 |证据|
|----------|----------|
|回购→逆回购→替代的确定性结算证明 | `integration_tests/tests/repo.rs` 捕获端到端流程、重复 ID 防护、保证金节奏检查以及抵押品替代成功/失败案例。该套件作为 `cargo test --workspace` 的一部分运行。 `crates/iroha_core/src/smartcontracts/isi/repo.rs` (`repo_deterministic_lifecycle_proof_matches_fixture`) 上的确定性生命周期摘要工具可快照启动 → 余量 → 替换帧，以便审核员可以区分规范的有效负载。 |
|三方报道|运行时强制执行托管人感知流：`RepoAgreement::custodian` + `RepoAccountRole::Custodian` 事件（`crates/iroha_data_model/src/repo.rs:74`、`crates/iroha_data_model/src/events/data/events.rs:742`）。 |
|抵押替代测试|反向腿不变量拒绝抵押不足的替代（`crates/iroha_core/src/smartcontracts/isi/repo.rs:417`），并且集成测试断言在替代往返后账本正确清除（`integration_tests/tests/repo.rs:261`）。 |
|追加保证金节奏和参与者执行 | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` 练习 `RepoMarginCallIsi`，证明节奏一致的调度、拒绝过早调用以及仅限参与者授权。 |
|治理批准的操作手册 |本指南和 `repo_runbook.md` 提供 CLI/SDK 程序、欺诈/回滚步骤以及用于审计的证据捕获说明。 |

## 2. 生命周期表面

### 2.1 CLI 和 Norito 构建器

- `iroha app repo initiate|unwind|margin|margin-call` 包裹 `RepoIsi`，
  `ReverseRepoIsi` 和 `RepoMarginCallIsi`
  （`crates/iroha_cli/src/main.rs:3821`）。每个子命令支持 `--input` /
  `--output`，因此服务台可以在之前暂存指令有效负载以进行双重批准
  提交。托管人路由通过 `--custodian` 表示。
- `repo query list|get` 使用 `FindRepoAgreements` 来快照协议并且可以
  被重定向到 JSON 工件以获取证据包。
- `crates/iroha_cli/tests/cli_smoke.rs:2637`下的CLI烟雾测试确保
  对于审核员来说，发送到文件的路径保持稳定。

### 2.2 SDK 和自动化挂钩- Python 绑定公开 `RepoAgreementRecord`、`RepoCashLeg`、
  `RepoCollateralLeg`，以及便利建设者
  (`python/iroha_python/iroha_python_rs/src/lib.rs:2216`) 因此自动化可以
  在本地组装交易并评估 `next_margin_check_after`。
- JS/Swift 助手通过以下方式重用相同的 Norito 布局
  `javascript/iroha_js/src/instructionBuilders.js` 和
  `IrohaSwift/Sources/IrohaSwift/ConfidentialEncryptedPayload.swift` 用于备注
  处理； SDK 在线程化存储库治理旋钮时应参考此文档。

### 2.3 账本事件和遥测

每个生命周期操作都会发出 `AccountEvent::Repo(...)` 记录，其中包含
`RepoAccountEvent::{Initiated,Settled,MarginCalled}` 有效负载范围为
参与者角色 (`crates/iroha_data_model/src/events/data/events.rs:742`)。推
将这些事件放入您的 SIEM/日志聚合器中以获得防篡改的审核日志
用于桌面操作、追加保证金通知和托管人通知。

### 2.4 配置传播和验证

节点从 `[settlement.repo]` 节中摄取回购治理旋钮
`iroha_config` (`crates/iroha_config/src/parameters/user.rs:4071`)。对待那个
作为治理证据合约的一部分的片段——将其放入版本控制中
与存储库数据包一起并对其进行哈希处理，然后再将更改推送到您的
自动化或 ConfigMap。最小的配置文件如下所示：

```toml
[settlement.repo]
default_haircut_bps = 1500
margin_frequency_secs = 86400
eligible_collateral = ["4fEiy2n5VMFVfi6BzDJge519zAzg", "7dk8Pj8Bqo6XUqch4K2sF8MCM1zd"]

[settlement.repo.collateral_substitution_matrix]
"4fEiy2n5VMFVfi6BzDJge519zAzg" = ["7dk8Pj8Bqo6XUqch4K2sF8MCM1zd", "6zK1LDcJ3FvkpfoZQ8kHUaW6sA7F"]
```

操作清单：

1. 将上面的代码片段（或您的生产变体）提交到配置存储库中
   提供 `irohad` 并将其 SHA-256 记录在治理数据包中，以便
   审阅者可以区分您计划部署的字节。
2. 在整个队列中滚动更改（systemd 单元、Kubernetes ConfigMap 等）
   并重新启动每个节点。推出后立即捕获 Torii
   出处的配置快照：

   ```bash
   curl -sS "${TORII_URL}/v1/configuration" \
     -H "Authorization: Bearer ${TOKEN}" | jq .
   ```

   `ToriiClient.get_configuration()` 在 Python SDK 中可用，用于相同的
   自动化需要输入证据时的目的。【python/iroha_python/src/iroha_python/client.py:5791】
3. 通过查询证明运行时现在强制执行所请求的节奏/发型
   `FindRepoAgreements`（或 `iroha app repo margin --agreement-id ...`）和
   检查嵌入的 `RepoGovernance` 值。存储 JSON 响应
   在 `artifacts/finance/repo/<agreement>/agreements_after.json` 下；那些价值观
   源自 `[settlement.repo]`，因此当
   Torii 的 `/v1/configuration` 快照不足。
4. 将两个工件（TOML 片段和 Torii/CLI 快照）保留在
   在提交治理请求之前收集证据。审核员必须能够
   重放该代码片段，验证其哈希值，并将其与运行时视图关联起来。

此工作流程确保存储库台永远不会依赖临时环境变量，即
配置路径保持确定性，每个治理票证都带有
路线图 F1 中预计有相同的 `iroha_config` 样张集。

### 2.5 确定性证明工具

单元测试 `repo_deterministic_lifecycle_proof_matches_fixture`（参见
`crates/iroha_core/src/smartcontracts/isi/repo.rs`）序列化每个阶段
将 repo 生命周期转换为 Norito JSON 框架，将其与规范的固定装置进行比较
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.json`，并散列
捆绑（夹具摘要跟踪
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`）。通过以下方式在本地运行：

```bash
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```该测试现在作为默认 `cargo test -p iroha_core` 套件的一部分运行，因此 CI
自动保护快照。每当回购语义或固定装置发生变化时，
使用以下命令刷新 JSON 和摘要：

```bash
scripts/regen_repo_proof_fixture.sh
```

助手使用固定的 `rust-toolchain.toml` 通道，重写灯具
在 `crates/iroha_core/tests/fixtures/` 下，并重新运行确定性工具
因此签入的快照/摘要与运行时行为保持同步
审核员将重播。

### 2.4 Torii API 表面

- `GET /v1/repo/agreements` 返回带有可选分页、过滤的活动协议
  (`filter={...}`)、排序和地址格式化参数。使用它进行快速审核或
  当原始 JSON 有效负载足够时，仪表板。
- `POST /v1/repo/agreements/query` 接受结构化查询信封（分页、排序、
  `FilterExpr`、`fetch_size`），因此下游服务可以确定性地翻阅账本。
- JavaScript SDK 现在公开 `listRepoAgreements`、`queryRepoAgreements` 和迭代器
  帮助器，以便 browser/Node.js 工具接收与 Rust/Python 相同类型的 DTO。

### 2.4 配置默认值

节点将 `[settlement.repo]` 读入
启动期间`iroha_config::parameters::actual::Repo`；任何回购指令
使参数为零，并根据之前的默认值进行归一化
记录在链上。【crates/iroha_core/src/smartcontracts/isi/repo.rs:40】这个
让治理在不触及每个 SDK 的情况下提高（或降低）基准政策
呼叫站点，前提是政策变更已完整记录。

- `default_haircut_bps` – `RepoGovernance::haircut_bps()` 时的后备理发
  等于零。运行时将其限制在 10000bps 的硬上限以保持
  配置正常。【crates/iroha_core/src/smartcontracts/isi/repo.rs:44】
- `margin_frequency_secs` – `RepoMarginCallIsi` 的节奏。清零请求
  继承这个值，因此缩短节奏迫使办公桌留出更多余量
  默认情况下经常发生。【crates/iroha_core/src/smartcontracts/isi/repo.rs:49】
- `eligible_collateral` – `AssetDefinitionId` 的可选允许列表。当
  列表非空 `RepoIsi` 拒绝集合之外的任何质押，防止
  意外加入未经审查的债券。【crates/iroha_core/src/smartcontracts/isi/repo.rs:57】
- `collateral_substitution_matrix` – 原始抵押品地图 →
  允许的替代品。 `ReverseRepoIsi` 仅在以下情况下接受替换
  矩阵包含记录的定义作为键以及其替换
  值数组；否则，平仓失败，证明治理批准了
  梯子。【crates/iroha_core/src/smartcontracts/isi/repo.rs:74】

这些旋钮位于节点配置中的 `[settlement.repo]` 下，并且是
通过 `iroha_config::parameters::user::Repo` 解析，因此它们应该被捕获在
每个治理证据包。【crates/iroha_config/src/parameters/user.rs:3956】

```toml
[settlement.repo]
default_haircut_bps = 1750
margin_frequency_secs = 43200
eligible_collateral = ["4fEiy2n5VMFVfi6BzDJge519zAzg", "7dk8Pj8Bqo6XUqch4K2sF8MCM1zd"]

[settlement.repo.collateral_substitution_matrix]
"4fEiy2n5VMFVfi6BzDJge519zAzg" = ["7dk8Pj8Bqo6XUqch4K2sF8MCM1zd", "6zK1LDcJ3FvkpfoZQ8kHUaW6sA7F"]
```

**变更管理清单**1. 暂存提议的 TOML 片段（包括替换矩阵增量）、哈希
   使用 SHA-256，并将代码片段和哈希值附加到治理中
   票证，以便审阅者可以逐字重现字节。
2. 引用提案/公投中的片段（例如通过
   治理 CLI 上的 `--notes` 字段）并收集所需的批准
   对于F1。保留已签名的批准数据包并附加片段。
3. 在整个机群中滚动更改：更新 `[settlement.repo]`，重新启动每个
   节点，然后捕获 `GET /v1/configuration` 快照（或
   `ToriiClient.getConfiguration`) 证明每个对等点的应用值。
4.重新运行`integration_tests/tests/repo.rs` plus
   `repo_deterministic_lifecycle_proof_matches_fixture` 并接下来存储日志
   到配置差异，以便审核员可以看到新的默认值保留
   决定论。

如果没有矩阵条目，运行时将拒绝更改资产的替换
定义，即使通用 `eligible_collateral` 列表允许；提交
配置快照和回购证据，以便审计员可以重现准确的
预订回购时强制执行的政策。

### 2.5 配置证据和漂移检测

Norito/`iroha_config` 管道现在公开已解决的回购策略
`iroha_config::parameters::actual::Repo`，因此治理数据包必须证明
每个同行应用的值——不仅仅是提议的 TOML。捕获已解决的
每次推出后的配置及其摘要：

1. 从每个对等方获取配置（`GET /v1/configuration` 或
   `ToriiClient.getConfiguration`) 并隔离存储库节：

   ```bash
   curl -s http://<torii-host>/v1/configuration \
     | jq -cS '.settlement.repo' \
     > artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

2. 对规范 JSON 进行哈希处理并将其记录在证据清单中。当
   舰队运行状况良好，哈希值应该在对等点之间匹配，因为 `actual`
   将默认值与暂存的 `[settlement.repo]` 片段相结合：

   ```bash
   shasum -a 256 artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

3. 将 JSON + hash 附加到治理数据包中，并将该条目镜像到
   清单已上传到治理 DAG。如果任何同行报告有分歧
   消化、停止推出并协调之前的配置/状态漂移
   进行中。

### 2.6 治理批准和证据包

仅当存储库台将确定性数据包馈送到路线图 F1 时，路线图 F1 才会关闭
治理 DAG，因此每次更改（新的理发、托管政策或抵押品）
矩阵）必须在安排投票之前运送相同的物品。【docs/source/governance_playbook.md:1】

**摄入包**1. **跟踪模板** – 副本
   `docs/examples/finance/repo_governance_packet_template.md` 进入你的证据
   目录（例如
   `artifacts/finance/repo/<agreement-id>/packet.md`) 并填写元数据
   在开始对工件进行哈希处理之前进行阻止。模板保留治理
   理事会的节奏通过列出文件路径、SHA-256 摘要和
   审稿人致谢集中在一处。
2. **指令有效负载** – 阶段启动、平仓和追加保证金
   带有 `iroha app repo ... --output` 的说明，以便双控审批者审核
   字节相同的有效负载。散列每个文件并将其存储在
   `artifacts/finance/repo/<agreement-id>/` 位于桌子证据包旁边
   本笔记其他地方引用。【crates/iroha_cli/src/main.rs:3821】
3. **配置差异** – 包括确切的 `[settlement.repo]` TOML 片段
   （默认加上替换矩阵）及其 SHA-256。这证明了哪个
   一旦投票通过并反映结果，`iroha_config` 旋钮将被激活
   在准入时标准化回购指令的运行时字段。【crates/iroha_config/src/parameters/user.rs:3956】
4. **确定性测试** – 附上最新的
   `integration_tests/tests/repo.rs` 日志和输出
   `repo_deterministic_lifecycle_proof_matches_fixture` 所以审阅者看到
   与分阶段指令对应的生命周期证明哈希。【integration_tests/tests/repo.rs:1】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1450】
5. **事件/遥测快照** – 导出最近的 `AccountEvent::Repo(*)`
   范围内的服务台以及理事会需要的任何仪表板/指标的流
   判断风险（例如，保证金漂移）。这给了审计师同样的
   他们稍后会从 Torii 重建防篡改日志。【crates/iroha_data_model/src/events/data/events.rs:742】

**批准和记录**

- 参考治理票证或公投中的人工制品哈希值，以及
  链接到分阶段的数据包，以便理事会可以遵循标准仪式
  治理手册中概述，无需追逐临时路径。【docs/source/governance_playbook.md:8】
- 捕获哪些双重控制签名者审查了分阶段的指令文件并
  将他们的确认存储在哈希值旁边；这是链上证明
  回购台满足“两人规则”，尽管运行时也
  强制仅参与者执行。
- 当理事会发布治理批准记录 (GAR) 时，反映
  在证据目录中签署会议记录，以便将来替换或
  发型更新可以引用确切的决策包，而不是重述
  理由。

**批准后推出**1. 应用批准的 `[settlement.repo]` 配置并重新启动每个节点（或滚动
   通过您的自动化）。立即拨打 `GET /v1/configuration` 并存档
   每个节点的响应，以便治理包显示哪些对等点接受了
   改变。【crates/iroha_torii/src/lib.rs:3225】
2. 重新运行确定性存储库测试并附加新日志和构建
   元数据（git commit、工具链），以便审计员可以重现结算结果
   推出后的证明。
3. 使用证据存档路径、哈希值和更新治理跟踪器
   观察者联系，以便以后的回购台可以继承相同的流程，而不是
   重新导出清单。

**治理 DAG 出版物（必需）**

1. Tar 证据目录（配置片段、指令负载、证明日志、
   GAR/分钟）并将其作为治理 DAG 管道
   `GovernancePayloadKind::PolicyUpdate` 有效负载，带有注释
   `agreement_id`、`iso_week` 以及建议的折扣/保证金值；的
   管道规范和 CLI 界面位于
   `docs/source/sorafs_governance_dag_plan.md`。
2.发布者更新IPNS头后，记录块CID和头CID
   在治理跟踪器和 GAR 中，这样任何人都可以获取不可变的数据
   稍后包。 `sorafs governance dag head` 和 `sorafs governance dag list`
   让您在投票开始前确认节点已固定。
3. 将 CAR 文件或块有效负载存储在回购证据存档旁边，以便
   审计员可以将链上治理决策与确切的情况进行协调
   已批准的链外数据包。

### 2.7 生命周期快照刷新

每当回购语义发生变化（利率、结算数学、托管逻辑或
默认配置），刷新确定性生命周期快照，以便治理可以
引用新的摘要，无需对证明工具进行逆向工程。

1. 刷新固定工具链下的夹具：

   ```bash
   scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
     --bundle-dir artifacts/finance/repo/<agreement>
   ```

   助手将输出暂存在临时目录中，更新跟踪的装置
   在 `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.{json,digest}`，
   重新运行验证测试以进行验证，并且（当设置 `--bundle-dir` 时）
   将 `repo_proof_snapshot.json` 和 `repo_proof_digest.txt` 放入捆绑包中
   审计员目录。
2. 在不接触跟踪固定装置的情况下导出工件（例如，试运行
   证据），直接设置环境助手：

   ```bash
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<agreement>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<agreement>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```

   `REPO_PROOF_SNAPSHOT_OUT` 从证明中接收美化后的 Norito JSON
   线束，而 `REPO_PROOF_DIGEST_OUT` 存储大写十六进制摘要（带有
   为了方便起见，尾随换行符）。当以下情况时，助手拒绝覆盖文件
   父目录不存在，因此首先构建 `artifacts/...` 树。
3. 将两个导出的文件附加到协议包（参见§3）并重新生成
   通过 `scripts/repo_evidence_manifest.py` 的清单，因此治理数据包
   明确引用刷新的证据工件。回购内固定装置
   仍然是 CI 的真相来源。

### 2.8 应计利息和到期日治理**确定性利息数学。** `RepoIsi` 和 `ReverseRepoIsi` 得出现金
在放松时间欠 ACT/360 助手的
`compute_accrued_interest()`【crates/iroha_core/src/smartcontracts/isi/repo.rs:100】
以及 `expected_cash_settlement()` 内拒绝还款腿的守卫
其回报小于*本金+利息*。【crates/iroha_core/src/smartcontracts/isi/repo.rs:132】
帮助器将 `rate_bps` 标准化为四位小数分数，并将其乘以
`elapsed_ms / (360 * 24h)` 使用 18 位小数，最后四舍五入到
现金支线的 `NumericSpec` 声明的规模。保留治理包
可重现，捕获为助手提供的四个值：

1. `cash_leg.quantity`（本金），
2.`rate_bps`，
3. `initiated_timestamp_ms`，以及
4. 您打算使用的展开时间戳（对于计划的总帐条目，这是
   通常为 `maturity_timestamp_ms`，但紧急展开会记录实际的
   `ReverseRepoIsi::settlement_timestamp_ms`）。

将元组存储在分阶段展开指令旁边并附上简短的证明
片段如：

```python
from decimal import Decimal
ACT_360_YEAR_MS = 24 * 60 * 60 * 1000 * 360

principal = Decimal("1000")
rate_bps = Decimal("1500")  # 150 bps
elapsed_ms = Decimal(maturity_ms - initiated_ms)
interest = principal * (rate_bps / Decimal(10_000)) * (elapsed_ms / Decimal(ACT_360_YEAR_MS))
expected_cash = principal + interest.quantize(Decimal("0.01"))
```

舍入的 `expected_cash` 必须与反向编码的 `quantity` 匹配
回购指令。将脚本输出（或计算器工作表）保存在
`artifacts/finance/repo/<agreement>/interest.json`，以便审核员可以重新计算
无需解释您的交易电子表格即可得出数据。集成套件
已经强制执行相同的不变量
(`repo_roundtrip_transfers_balances_and_clears_agreement`)，但是操作证据
应该引用将要展开的确切值。【integration_tests/tests/repo.rs:1】

**保证金和应计节奏。** 每项协议都会暴露节奏助手
`RepoAgreement::next_margin_check_after()` 和缓存的
`last_margin_check_timestamp_ms`，使办公桌能够证明利润扫荡
甚至在提交 `RepoMarginCallIsi` 之前就根据政策进行了安排
交易。【crates/iroha_data_model/src/repo.rs:113】【crates/iroha_core/src/smartcontracts/isi/repo.rs:557】
每个追加保证金通知必须在证据包中包含三件文物：

1. `repo margin-call --agreement <id>` JSON输出（或等效的SDK
   Payload），记录了协议id、用于该协议的区块时间戳
   检查，以及触发它的权限。【crates/iroha_cli/src/main.rs:3821】
2. 协议快照（`repo query get --agreement-id <id>`）
   就在通话之前，以便审阅者可以确认节奏是否到期
   （比较 `current_timestamp_ms` 与 `next_margin_check_after()`）。
3. 发送到每个角色的 `AccountEvent::Repo::MarginCalled` SSE/NDJSON feed
   （发起人、交易对手和可选的托管人）因为运行时
   为每个参与者复制事件。【crates/iroha_data_model/src/events/data/events.rs:742】

CI 已经通过以下方式行使了这些规则
`repo_margin_call_enforces_cadence_and_participant_rules`，拒绝呼叫
提前到达或来自未经授权的帐户。【integration_tests/tests/repo.rs:395】
重复证据档案中的出处就是路线图 F1 的结束
文档差距：治理审核者可以看到与
运行时依赖，以及第 2.7 节中捕获的确定性证明哈希
以及第 3.2 节中讨论的清单。

### 2.8 三方托管批准和监控路线图 **F1** 还提出了三方回购协议，其中抵押品存放在
托管人而非交易对手。运行时强制执行托管路径
通过持久化 `RepoAgreement::custodian`，将质押资产路由至
托管人的帐户在启动和发出期间
`RepoAccountRole::Custodian` 每个生命周期步骤的事件，以便审核员可以看到
谁在每个时间戳持有抵押品。【crates/iroha_data_model/src/repo.rs:74】【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【integration_tests/tests/repo.rs:951】
除了上面列出的双边证据外，每个三方回购协议还必须
在治理数据包被视为完整之前捕获以下工件。

**额外摄入要求**

1. **保管人确认书。** 服务台必须存储来自以下机构的签名确认书：
   每个托管人确认回购标识符、托管窗口、路由
   账户和结算 SLA。附上已签署的文件
   （`artifacts/finance/repo/<agreement>/custodian_ack_<custodian>.md`）
   并在治理包中引用它，以便审阅者可以看到
   第三方同意发起者/交易对手批准的相同字节。
2. **托管账本快照。** 启动将抵押品移至托管人
   account 和 unwind 将其返回给发起者；捕捉相关的
   `FindAssets` 输出为每条腿之前和之后的托管人以便审核员
   可以确认余额与阶段指令相符。【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1641】
3. **事件收据。** 镜像所有角色的 `RepoAccountEvent` 流并
   将托管人有效负载与发起者/交易对手记录一起存储。
   运行时为每个角色发出单独的事件
   `RepoAccountRole::{Initiator,Counterparty,Custodian}`，因此附上原始数据
   SSE feed 证明所有三方都看到了相同的时间戳并且
   结算金额。【crates/iroha_data_model/src/events/data/events.rs:742】【integration_tests/tests/repo.rs:1508】
4. **托管人准备情况检查表。** 当回购引用操作时
   垫片（例如，托管调节或长期指示），记录
   用于排练工作流程的自动化联系人和命令（例如
   如 `iroha app repo initiate --custodian ... --dry-run`），因此审阅者可以到达
   演习期间的托管操作员。

|证据|命令/路径|目的|
|----------|----------------|---------|
|托管人确认 (`custodian_ack_<custodian>.md`) |链接到 `docs/examples/finance/repo_governance_packet_template.md` 中引用的签名注释（使用 `docs/examples/finance/repo_custodian_ack_template.md` 作为种子）。 |显示第三方在资产转移之前接受了 repo id、托管 SLA 和结算渠道。 |
|托管资产快照| `iroha json --query FindAssets '{ "id": "...#<custodian>" }' > artifacts/.../assets/custodian_<ts>.json` |证明留下/归还的抵押品与 `RepoIsi` 编码完全相同。 |
|托管人 `RepoAccountEvent` 饲料 | `torii-events --account <custodian> --event-type repo > artifacts/.../events/custodian.ndjson` |捕获为启动、追加保证金通知和展开而发出的运行时的 `RepoAccountRole::Custodian` 有效负载。 |
|托管演习日志| `artifacts/.../governance/drills/<timestamp>-custodian.log` |记录托管人执行回滚或结算脚本的试运行。 |重复使用相同的哈希工作流程 (`scripts/repo_evidence_manifest.py`)
托管人确认、资产快照和事件源保持三方关系
数据包可重现。当多个保管人参与一本书时，创建
每个保管人的子目录，以便清单突出显示哪些文件属于
各方；治理票证应引用每个清单哈希和
匹配的确认文件。集成测试涵盖
`repo_initiation_with_custodian_routes_collateral` 和
`reverse_repo_with_custodian_emits_events_for_all_parties` 已经强制执行
运行时行为——将他们的人工制品镜像到证据包中就是什么
让路线图 **F1** 为三方场景提供 GA 就绪文档。【integration_tests/tests/repo.rs:951】【integration_tests/tests/repo.rs:1508】

### 2.9 批准后配置快照

一旦治理批准变更并且 `[settlement.repo]` 节登陆
在集群中，从每个对等方捕获经过身份验证的配置快照，以便
审核员可以证明批准的值是有效的。 Torii 暴露了
用于此目的的 `/v1/configuration` 路线和所有 SDK 表面帮助程序，例如
`ToriiClient.getConfiguration`，因此捕获工作流程适用于桌面脚本，
CI，或手动操作员运行。【crates/iroha_torii/src/lib.rs:3225】【javascript/iroha_js/src/toriiClient.js:2115】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4681】

1. 在每个对等点之后立即调用 `GET /v1/configuration`（或 SDK 帮助程序）
   推出。将完整的 JSON 保留在下面
   `artifacts/finance/repo/<agreement>/config/peers/<peer-id>.json` 并记录
   `config/config_snapshot_index.md` 中的块高度/集群时间戳。
   ```bash
   mkdir -p artifacts/finance/repo/<slug>/config/peers
   curl -fsSL https://peer01.example/v1/configuration \
     | jq '.' \
     > artifacts/finance/repo/<slug>/config/peers/peer01.json
   ```
2. 对每个快照 (`sha256sum config/peers/*.json`) 进行哈希处理并记录下一个摘要
   到治理数据包模板中的对等 ID。这证明了哪些同行
   摄取策略以及哪个提交/工具链生成了快照。
3. 将每个快照中的 `.settlement.repo` 块与暂存的块进行比较
   `[settlement.repo]` TOML 片段；记录任何漂移并重新运行
   `repo query get --agreement-id <id> --pretty` 所以证据包显示
   运行时配置和标准化 `RepoGovernance` 值
   与协议一起存储。【crates/iroha_cli/src/main.rs:3821】
4. 将快照文件和摘要索引附加到证据清单（请参阅
   §3.2）因此治理记录将批准的变更链接到实际的同行
   配置字节。治理模板已更新以包含此内容
   表，因此每个未来的回购数据包都带有相同的证明。

捕获这些快照可以弥补 `iroha_config` 文档中指出的空白
路线图中：审阅者现在可以将分阶段的 TOML 与每个字节进行比较
同行报告，只要回购协议发生变化，审计师就可以重新进行比较
正在调查中。

## 3. 确定性证据工作流程1. **记录指令出处**
   - 通过 `iroha app repo ... --output` 生成存储库/展开有效负载。
   - 将 `InstructionBox` JSON 存储在
     `artifacts/finance/repo/<agreement-id>/initiation.json`。
2. **捕获账本状态**
   - 之前运行 `iroha app repo query list --pretty > artifacts/.../agreements.json`
     并在结算后证明余额已结清。
   - 可选择通过 `iroha json` 或 SDK 帮助程序查询 `FindAssets` 进行存档
     回购分支中触及的资产余额。
3. **持久化事件流**
   - 通过 Torii SSE 订阅 `AccountEvent::Repo` 或拉出并附加
     将 JSON 发送到证据目录。这满足了防篡改
     日志记录子句，因为事件是由观察到的对等方签名的
     每一次改变。
4. **运行确定性测试**
   - CI already runs `integration_tests/tests/repo.rs`;对于手动签核，
     执行`cargo test -p integration_tests repo::`并归档日志加上
     `target/debug/deps/repo-*` JUnit 输出。
5. **序列化治理和配置**
   - 签入（或附加）该期间使用的 `[settlement.repo]` 配置，
     包括理发/合格名单。这使得审核重播能够匹配
     运行时规范化治理记录在 `RepoAgreement` 中。

### 3.1 证据包布局

将本节中提到的所有工件存储在单一协议下
目录，以便治理可以归档或散列一棵树。推荐的布局是：

```
artifacts/finance/repo/<agreement-id>/
├── agreements_before.json
├── agreements_after.json
├── initiation.json
├── unwind.json
├── margin/
│   └── 2026-04-30.json
├── events/
│   └── repo-events.ndjson
├── config/
│   ├── settlement_repo.toml
│   └── peers/
│       ├── peer01.json
│       └── peer02.json
├── repo_proof_snapshot.json
├── repo_proof_digest.txt
└── tests/
    └── repo_lifecycle.log
```

- `agreements_before/after.json` 捕获 `repo query list` 输出，以便审核员可以
  证明账本已清除协议。
- `initiation.json`、`unwind.json` 和 `margin/*.json` 与 Norito 完全相同
  有效负载以 `iroha app repo ... --output` 上演。
- `events/repo-events.ndjson` 重播 `AccountEvent::Repo(*)` 流，同时
  `tests/repo_lifecycle.log` 保留 `cargo test` 证据。
- `repo_proof_snapshot.json` 和 `repo_proof_digest.txt` 来自快照
  刷新§2.7中的过程并让审阅者重新计算生命周期哈希
  无需重新运行线束。
- `config/settlement_repo.toml` 包含 `[settlement.repo]` 片段
  （理发、替换矩阵）在执行回购协议时处于活动状态。
- `config/peers/*.json` 捕获每个对等点的 `/v1/configuration` 快照，
  关闭暂存 TOML 和运行时值同行报告之间的循环
  超过 Torii。

### 3.2 哈希清单生成

将确定性清单附加到每个包，以便审核者可以验证哈希值
无需解压存档。 `scripts/repo_evidence_manifest.py` 的助手
走协议目录，记录`size`，`sha256`，`blake2b`，最后一个
修改每个文件的时间戳，并写入 JSON 摘要：

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/wonderland-2026q1 \
  --agreement-id 7mxD1tKRyv32je4kZwcWa9wa33bX \
  --output artifacts/finance/repo/wonderland-2026q1/manifest.json \
  --exclude 'scratch/*'
```

生成器按字典顺序对路径进行排序，在输出文件存在时跳过它
在同一目录中，并发出治理可以直接复制的总计
进入改签机票。当省略 `--output` 时，清单将打印到
标准输出，方便在案头审查期间进行快速比较。
使用 `--exclude <glob>` 省略临时材料（例如，`--exclude 'scratch/*' --exclude '*.tmp'`）
无需将文件移出捆绑包； glob 模式始终适用于
相对于 `--root` 的路径。示例清单（为简洁起见被截断）：

```json
{
  "agreement_id": "7mxD1tKRyv32je4kZwcWa9wa33bX",
  "generated_at": "2026-04-30T11:58:43Z",
  "root": "/var/tmp/repo/wonderland-2026q1",
  "file_count": 5,
  "total_bytes": 1898,
  "files": [
    {
      "path": "agreements_after.json",
      "size": 512,
      "sha256": "6b6ca81b00d0d889272142ce1e6456872dd6b01ce77fcd1905f7374fc7c110cc",
      "blake2b": "5f0c7f03d15cd2a69a120f85df2a4a4a219a716e1f2ec5852a9eb4cdb443cbfe3c1e8cd02b3b7dbfb89ab51a1067f4107be9eab7d5b46a957c07994eb60bb070",
      "modified_at": "2026-04-30T11:42:01Z"
    },
    {
      "path": "initiation.json",
      "size": 274,
      "sha256": "7a1a0ec8c8c5d43485c3fee2455f996191f0e17a9a7d6b25fc47df0ba8de91e7",
      "blake2b": "ce72691b4e26605f2e8a6486d2b43a3c2b472493efd824ab93683a1c1d77e4cff40f5a8d99d138651b93bcd1b1cb5aa855f2c49b5f345d8fac41f5b221859621",
      "modified_at": "2026-04-30T11:39:55Z"
    }
  ]
}
```

将清单包含在证据包旁边并引用其 SHA-256 哈希值
在治理提案中，各部门、运营商和审计员共享相同的信息
基本事实。

### 3.3 治理变更日志和回滚演练

金融委员会预计每一次回购请求、削减调整或替代
矩阵更改以可链接的可复制治理数据包的方式到达
直接来自公投记录。【docs/source/governance_playbook.md:1】

1. **构建治理包**
   - 将协议的证据包复制到
     `artifacts/finance/repo/<agreement-id>/governance/`。
   - 添加`gar.json`（理事会批准记录），`referendum.md`（谁批准
     以及他们审查了哪些哈希值），以及 `rollback_playbook.md`
     总结 `repo_runbook.md` 的逆转过程
     §§4–5.【docs/source/finance/repo_runbook.md:1】
   - 捕获第 3.2 节中的确定性清单哈希
     `hashes.txt`，以便审核者可以确认他们在 Torii 匹配中看到的有效负载
     分阶段的字节。
2. **参考公投中的数据包**
   - 运行 `iroha app governance referendum submit`（或等效的 SDK
     helper）将来自 `hashes.txt` 的清单哈希包含在 `--notes` 中
     有效负载，因此 GAR 指向不可变的数据包。
   - 在治理跟踪器或票务系统中归档相同的哈希值，以便
     审计跟踪不依赖于仪表板屏幕截图。
3. **记录演练和回滚**
   - 公投通过后，使用存储库更新 `ops/drill-log.md`
     协议 ID、部署的配置哈希、GAR ID 和操作员联系方式，以便
     季度演练记录包括财务行动。【ops/drill-log.md:1】
   - 如果执行回滚演习，请附上签名的
     `rollback_playbook.md` 和 `iroha app repo unwind` 下的 CLI 输出
     `governance/drills/<timestamp>.log` 并使用相同的信息通知理事会
     治理手册中描述的步骤。

布局示例：

```
artifacts/finance/repo/<agreement-id>/governance/
├── gar.json
├── hashes.txt
├── referendum.md
├── rollback_playbook.md
└── drills/
    └── 2026-05-12T09-00Z.log
```

将 GAR、公投和演练工件与生命周期保持一致
evidence guarantees that every repo change satisfies the roadmap F1 governance
酒吧，无需稍后购买定制门票。

### 3.4 生命周期治理清单

路线图 **F1** 提出了启动、应计/利润和的治理覆盖范围
三方放松。下表汇总了确定性的批准
工件，并测试每个生命周期步骤的参考，以便财务部门可以引用
组装数据包时的单一清单。|生命周期步骤|所需的批准和门票 |确定性工件和命令 |链接回归覆盖率 |
|----------------|------------------------------------------|------------------------------------------------|----------------------------|
| **发起（双边或三方）** |通过 `docs/examples/finance/repo_governance_packet_template.md` 记录的双控制签核、具有 `[settlement.repo]` diff 和 GAR ID 的治理票据、设置 `--custodian` 时的托管人确认。 |通过`iroha --config client.toml --output repo initiate ...` 暂存指令。发出生命周期证明快照（`REPO_PROOF_*` 环境变量）以及来自 `scripts/repo_evidence_manifest.py` 的捆绑清单。附加最新的 `FindRepoAgreements` JSON 和 `[settlement.repo]` 片段（理发、合格列表、替换矩阵）。 | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement`（双边）和 `integration_tests/tests/repo.rs::repo_roundtrip_with_custodian_routes_collateral`（三方）证明运行时与分阶段的有效负载匹配。 |
| **追加保证金应计节奏** |办公桌领导 + 风险经理批准治理包中记录的节奏窗口；票证引用预定的 `RepoMarginCallIsi`。 |在调用 `iroha app repo margin-call` 之前捕获 `iroha app repo margin --agreement-id` 输出，对生成的 JSON 进行哈希处理，并将 `RepoAccountEvent::MarginCalled` SSE 负载存档在证据包中。将 CLI 日志存储在确定性证明哈希旁边。 | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` 保证运行时拒绝过早调用和非参与者提交。 |
| **抵押品替代和到期解除** |治理变更记录引用了所需的 `collateral_substitution_matrix` 条目和削减政策；理事会会议记录列出了替换对 SHA-256 哈希值。 |使用 `iroha app repo unwind --output ... --settlement-timestamp-ms <planned>` 暂存展开部分，以便 ACT/360 计算 (§2.8) 和替换有效负载都可重现。将 `[settlement.repo]` TOML 片段、替换清单以及生成的 `RepoAccountEvent::Settled` 有效负载包含在工件包中。 | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` 内的替换往返执行不足与批准的替换流程，同时保持协议 ID 不变。 |
| **紧急放松/回滚演练** |事件指挥官 + 财务委员会批准 `docs/source/finance/repo_runbook.md`（第 4-5 节）中所述的回滚，并捕获 `ops/drill-log.md` 中的条目。 |使用分阶段回滚负载执行 `iroha app repo unwind`，将 CLI 日志 + GAR 引用附加到 `governance/drills/<timestamp>.log`，然后重新运行 `repo_deterministic_lifecycle_proof_matches_fixture` 和 `scripts/repo_evidence_manifest.py` 帮助程序以证明演练之前/之后的确定性。 |快乐路径展开由 `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` 涵盖；遵循演练步骤可以使治理工件与该测试所执行的运行时保证保持一致。 |

**桌面时间表。**1. 复制摄入模板，填写元数据块（协议 ID、GAR 票证、
   保管人、配置哈希），并创建证据目录。
2. 将每条指令（`initiate`、`margin-call`、`unwind`、替换）暂存在
   `--output` 模式，散列 JSON，并在每个散列旁边记录批准。
3. 在暂存后立即发出生命周期证明快照和清单，以便
   治理审核者可以使用相同的回购协议重新计算摘要。
4. 镜像受影响帐户的 `RepoAccountEvent::*` SSE 负载并删除
   `artifacts/finance/repo/<agreement-id>/events.ndjson` 中导出的 NDJSON
   在归档数据包之前。
5. 投票通过后，用 GAR 标识符更新 `hashes.txt`，
   配置哈希和清单校验和，以便理事会可以跟踪部署
   无需重新运行本地脚本。

### 3.5 治理包快速入门

路线图 F1 审阅者要求提供一份简明的清单，以便他们在
组装证据包。每当有回购请求时，请遵循以下顺序
或政策变化正走向治理：

1. **导出生命周期证明工件。**
   ```bash
   mkdir -p artifacts/finance/repo/<slug>
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```
   导出的 JSON + 摘要镜像了下签入的装置
   `crates/iroha_core/tests/fixtures/`，以便审阅者可以重新计算生命周期
   框架而无需重新运行整个套件（请参阅§2.7）。您也可以致电
   `scripts/regen_repo_proof_fixture.sh --bundle-dir artifacts/finance/repo/<slug>`
   一步刷新并复制相同的文件。
2. **暂存并散列每条指令。** 生成启动/保证金/展开
   有效负载为 `iroha app repo ... --output`。捕获每个文件的 SHA-256
   （存储在 `hashes/` 下）所以 `docs/examples/finance/repo_governance_packet_template.md`
   可以引用办公桌审查的相同字节。
3. **保存账本/配置快照。** 导出之前/之后的 `repo query list` 输出
   结算，转储将要应用的 `[settlement.repo]` TOML 块，以及
   将相关 `AccountEvent::Repo(*)` SSE 镜像到
   `artifacts/finance/repo/<slug>/events/repo-events.ndjson`。 GAR之后
   通过，捕获每个对等点的 `/v1/configuration` 快照（第 2.9 节）并存储它们
   在 `config/peers/` 下，因此治理数据包证明部署成功。
4. **生成证据清单。**
   ```bash
   python3 scripts/repo_evidence_manifest.py \
     --root artifacts/finance/repo/<slug> \
     --agreement-id <repo-id> \
     --output artifacts/finance/repo/<slug>/manifest.json
   ```
   将清单哈希包含在治理票证或 GAR 分钟内，以便
   审核员可以在不下载原始包的情况下区分数据包（请参阅第 3.2 节）。
5. **组装数据包。** 从以下位置复制模板
   `docs/examples/finance/repo_governance_packet_template.md`，填写元数据，
   附加证明快照/摘要、清单、配置哈希、SSE 导出和测试
   日志，然后在全民投票 `--notes` 字段中引用清单 SHA-256。
   将完成的 Markdown 存储在工件旁边，以便回滚继承
   您运送以供批准的确切证据。

在暂存存储库请求后立即运行上述步骤意味着
理事会一召开，治理包就已准备就绪，避免了最后一刻
争先恐后地重新创建哈希值或事件流。

## 4. 三方托管和抵押品替代- **托管人：** 通过 `--custodian <account>` 路线抵押品
  保管金库；运行时强制帐户存在并发出角色标记
  事件，以便保管人可以协调 (`RepoAccountRole::Custodian`)。国家
  机器拒绝托管人与任何一方匹配的协议。
- **抵押品替代：** 平仓部分可能会提供不同的抵押品
  替换期间的数量/系列，只要它**不小于**
  质押金额*和*替换矩阵允许配对； `ReverseRepoIsi`
  强制执行这两个条件
  (`crates/iroha_core/src/smartcontracts/isi/repo.rs:414`–`437`)。整合
  测试套件同时测试拒绝路径和成功替换
  往返 (`integration_tests/tests/repo.rs:261`–`359`)，而回购单元
  测试涵盖了新的矩阵策略。
- **ISO 20022 映射：** 构建 ISO 信封或协调外部时
  系统，重用记录在
  `docs/source/finance/settlement_iso_mapping.md` (`colr.007`, `sese.023`,
  `sese.025`），因此 Norito 有效负载和 ISO 确认保持同步。

## 5. 操作清单

### 每日开盘前

1. 通过`iroha app repo query list`导出未完成的协议。
2. 与库存库存进行比较并确保合格的抵押品配置
   与计划的书相符。
3. 使用 `--output` 暂存即将到来的回购/解除并收集双重批准。

### 日内监控

1、发起人/交易对手/托管人订阅`AccountEvent::Repo`
   账户；当意外启动发生时发出警报。
2. 使用 `iroha app repo margin --agreement-id ID`（或
   `RepoAgreementRecord::next_margin_check_after`) 每小时检测一次踏频
   漂移；当 `is_due = true` 时触发 `repo margin-call`。
3. Log all margin calls with operator initials and attach the CLI JSON output to
   证据目录。

### 日终+结算后

1. 重新运行 `repo query list` 并确认已解除的协议已被删除。
2. 归档 `RepoAccountEvent::Settled` 有效负载并交叉检查现金/抵押品
   通过 `FindAssets` 进行余额。
3. 当回购演习或事件测试时，在 `ops/drill-log.md` 中归档演习条目
   跑；重用 `scripts/telemetry/log_sorafs_drill.sh` 时间戳约定。

## 6. 欺诈和回滚程序

- **双重控制：**始终使用 `--output` 生成指令并存储
  用于共同签名的 JSON。在流程级别拒绝单方提交
  即使运行时强制执行发起者权限。
- **防篡改日志记录：** 将 `RepoAccountEvent` 流镜像到您的
  SIEM，因此任何伪造的指令都可以被检测到（缺少对等签名）。
- **回滚：** 如果必须提前解除仓库，请提交 `repo unwind`
  使用相同的协议 ID 并在您的事件中附加 `--notes` 字段
  跟踪器引用 GAR 批准的回滚剧本。
- **欺诈升级：** 如果出现未经授权的存储库，请导出违规内容
  `RepoAccountEvent` 有效负载，通过治理策略冻结帐户，以及
  根据回购治理 SOP 通知理事会。

## 7. 报告和跟进

### 7.1 财务对账和分类账证据路线图 **F1** 和全球定居点护栏 (roadmap.md#L1975-L1978)
要求每次回购审查都包括确定性的财务证明。制作一个
按照下面的清单按季度捆绑每本书。

1. **快照余额。** 使用支持的 `FindAssets` 查询
   `iroha ledger asset list` (`crates/iroha_cli/src/main_shared.rs`) 或
   `iroha_python` 帮助程序导出 `soraカタカナ...` 的异或余额，
   `soraカタカナ...`，以及参与审核的每个台账。商店
   下的 JSON
   `artifacts/finance/repo/<period>/treasury_assets.json`并记录git
   随附的 `README.md` 中的提交/工具链。
2. **交叉检查账本预测。** 重新运行
   `sorafs reserve ledger --quote <...> --json-out ...` 并标准化输出
   通过 `scripts/telemetry/reserve_ledger_digest.py`。将摘要放在旁边
   资产快照，以便审计人员可以将 XOR 总数与回购协议进行比较
   无需重放 CLI 即可进行账本投影。
3. **发布调节说明。** 总结增量
   `artifacts/finance/repo/<period>/treasury_reconciliation.md` 参考：
   资产快照哈希、账本摘要哈希以及所涵盖的协议。
   链接财务治理跟踪器中的注释，以便审阅者可以确认
   批准回购发行之前的财务覆盖范围。

### 7.2 演练和回滚演练证据

验收标准还要求分阶段回滚和事件演习。每个
演习或混乱排练必须收集以下文物：

1. `repo_runbook.md` 第 4-5 节检查表，由事故指挥官签署，并且
   财务委员会。
2. 排练的 CLI/SDK 日志 (`repo initiate|margin-call|unwind`) 以及
   已存储刷新的生命周期证明快照和证据清单 (§§2.7–3.2)
   在 `artifacts/finance/repo/drills/<timestamp>/` 下。
3. Alertmanager 或寻呼机转录显示注入的信号和
   确认轨迹。将成绩单放在钻探制品旁边，然后
   包括使用时的 Alertmanager 静音 ID。
4. 引用 GAR id、清单哈希和钻取的 `ops/drill-log.md` 条目
   捆绑路径，以便将来的审核可以跟踪排练，而无需抓取聊天日志。

### 7.3 治理跟踪器和文档卫生

- 将此文档 `repo_runbook.md` 和财务治理跟踪器保存在
  每当 CLI/SDK 或运行时行为发生变化时，都会保持同步；审稿人期望
  验收表以保持准确。
- 附上完整的证据包（`agreements.json`、分阶段说明、SSE
  成绩单、配置快照、协调、演练工件和测试
  记录）到跟踪器以进行每个季度的审查。
- 协调时参考 `docs/source/finance/settlement_iso_mapping.md`
  与 ISO 桥操作员保持一致，以便跨系统协调保持一致。

By following this guide, operators satisfy the roadmap F1 acceptance bar:
捕获确定性证明，三方和替代流
记录和治理程序（双重控制+事件记录）
编码树内。