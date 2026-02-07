---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17fcb22d5be25f601d4096c3a3488b7be2dd92dcf27019b678634590cd3bdde4
source_last_modified: "2025-12-29T18:16:35.197199+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

> 改编自 [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md)。

# SoraFS 提供者准入和身份政策（SF-2b 草案）

本说明捕获了 **SF-2b** 的可操作交付成果：定义和
执行准入工作流程、身份要求和证明
SoraFS 存储提供商的有效负载。它扩展了高层流程
SoraFS 架构 RFC 中概述了剩余工作
可跟踪的工程任务。

## 政策目标

- 确保只有经过审查的运营商才能发布 `ProviderAdvertV1` 记录
  网络会接受。
- 将每个广告密钥绑定到经政府批准的身份文件，
  经证明的端点和最低股权贡献。
- 提供确定性验证工具，以便 Torii、网关和
  `sorafs-node` 强制执行相同的检查。
- 支持续订和紧急撤销，而不破坏确定性或
  工装工效学。

## 身份和权益要求

|要求 |描述 |可交付成果 |
|----------|-------------|----------|
|广告关键出处|提供商必须注册用于签署每个广告的 Ed25519 密钥对。准入包将公钥与治理签名一起存储。 |使用 `advert_key`（32 字节）扩展 `ProviderAdmissionProposalV1` 架构，并从注册表 (`sorafs_manifest::provider_admission`) 引用它。 |
|桩指针|入场需要一个指向活跃质押池的非零 `StakePointer`。 |在 `sorafs_manifest::provider_advert::StakePointer::validate()` 中添加验证并在 CLI/测试中显示错误。 |
|司法管辖区标签 |提供商声明管辖权+法律联系方式。 |使用 `jurisdiction_code` (ISO 3166-1 alpha-2) 和可选的 `contact_uri` 扩展提案架构。 |
|端点证明 |每个公布的端点必须由 mTLS 或 QUIC 证书报告支持。 |定义 `EndpointAttestationV1` Norito 有效负载并将每个端点存储在准入捆绑包内。 |

## 入学流程

1. **提案创建**
   - CLI：添加 `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …`
     生成 `ProviderAdmissionProposalV1` + 证明包。
   - 验证：确保必填字段、权益 > 0、`profile_id` 中的规范分块器句柄。
2. **治理认可**
   - 理事会使用现有的 `blake3("sorafs-provider-admission-v1" || canonical_bytes)` 签名
     信封工具（`sorafs_manifest::governance` 模块）。
   - 信封保留为 `governance/providers/<provider_id>/admission.json`。
3. **注册中心摄取**
   - 实现共享验证器（`sorafs_manifest::provider_admission::validate_envelope`）
     Torii/gateways/CLI 重用。
   - 更新 Torii 准入路径以拒绝摘要或到期时间与信封不同的广告。
4. **续订和撤销**
   - 添加 `ProviderAdmissionRenewalV1` 以及可选端点/权益更新。
   - 公开记录吊销原因并推送治理事件的 `--revoke` CLI 路径。

## 实施任务

|面积 |任务|所有者 |状态 |
|------|------|----------|--------|
|架构|在 `crates/sorafs_manifest/src/provider_admission.rs` 下定义 `ProviderAdmissionProposalV1`、`ProviderAdmissionEnvelopeV1`、`EndpointAttestationV1` (Norito)。在 `sorafs_manifest::provider_admission` 中实现，带有验证助手。【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 |存储/治理| ✅ 已完成 |
| CLI 工具 |使用子命令扩展 `sorafs_manifest_stub`：`provider-admission proposal`、`provider-admission sign`、`provider-admission verify`。 |工具工作组 | ✅ |

CLI 流程现在接受中间证书包 (`--endpoint-attestation-intermediate`)，发出
规范提案/信封字节，并在 `sign`/`verify` 期间验证理事会签名。运营商可以
直接提供广告主体，或重复使用签名广告，签名文件可以通过配对提供
`--council-signature-public-key` 和 `--council-signature-file` 实现自动化友好。

### CLI 参考

通过 `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …` 运行每个命令。

- `proposal`
  - 所需标志：`--provider-id=<hex32>`、`--chunker-profile=<namespace.name@semver>`、
    `--stake-pool-id=<hex32>`、`--stake-amount=<amount>`、`--advert-key=<hex32>`、
    `--jurisdiction-code=<ISO3166-1>`，以及至少一个 `--endpoint=<kind:host>`。
  - 每个端点认证需要 `--endpoint-attestation-attested-at=<secs>`，
    `--endpoint-attestation-expires-at=<secs>`，证书通过
    `--endpoint-attestation-leaf=<path>`（加上可选的 `--endpoint-attestation-intermediate=<path>`
    对于每个链元素）和任何协商的 ALPN ID
    （`--endpoint-attestation-alpn=<token>`）。 QUIC 端点可以提供传输报告
    `--endpoint-attestation-report[-hex]=…`。
  - 输出：规范的 Norito 提案字节 (`--proposal-out`) 和 JSON 摘要
    （默认标准输出或 `--json-out`）。
- `sign`
  - 输入：提案 (`--proposal`)、签名广告 (`--advert`)、可选广告正文
    (`--advert-body`)、保留纪元和至少一个理事会签名。可提供签名
    内联 (`--council-signature=<signer_hex:signature_hex>`) 或通过文件组合
    `--council-signature-public-key` 与 `--council-signature-file=<path>`。
  - 生成经过验证的信封 (`--envelope-out`) 和指示摘要绑定的 JSON 报告，
    签名者计数和输入路径。
- `verify`
  - 验证现有信封 (`--envelope`)，可选择检查匹配的提案，
    广告，或广告正文。 JSON 报告突出显示摘要值、签名验证状态、
    以及哪些可选工件相匹配。
- `renewal`
  - 将新批准的信封链接到先前批准的摘要。需要
    `--previous-envelope=<path>` 和后继 `--envelope=<path>`（均为 Norito 有效负载）。
    CLI 验证配置文件别名、功能和广告键是否保持不变，同时
    允许权益、端点和元数据更新。输出规范
    `ProviderAdmissionRenewalV1` 字节 (`--renewal-out`) 加上 JSON 摘要。
- `revoke`
  - 向提供商发出紧急 `ProviderAdmissionRevocationV1` 捆绑包，其信封必须
    被撤回。需要 `--envelope=<path>`、`--reason=<text>`，至少一个
    `--council-signature`，以及可选的 `--revoked-at`/`--notes`。 CLI 签署并验证
    撤销摘要，通过 `--revocation-out` 写入 Norito 有效负载，并打印 JSON 报告
    捕获摘要和签名计数。
|验证|实现 Torii、网关和 `sorafs-node` 使用的共享验证器。提供单元+CLI集成测试。【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 |网络 TL / 存储 | ✅ 已完成 |
| Torii 集成 |将验证程序引入 Torii 广告摄取，拒绝不符合策略的广告，发出遥测数据。 |网络 TL | ✅ 已完成 | Torii 现在加载治理信封 (`torii.sorafs.admission_envelopes_dir`)，在摄取期间验证摘要/签名匹配，并显示准入遥测。【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
|续订 |添加续订/撤销架构 + CLI 帮助程序，在文档中发布生命周期指南（请参阅下面的运行手册和中的 CLI 命令） `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 |存储/治理| ✅ 已完成 |
|遥测|定义 `provider_admission` 仪表板和警报（缺少续订、信封到期）。 |可观察性| 🟠 进行中 |计数器 `torii_sorafs_admission_total{result,reason}` 存在；仪表板/警报待处理。【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### 续订和撤销操作手册

#### 预定更新（权益/拓扑更新）
1. 使用 `provider-admission proposal` 和 `provider-admission sign` 构建后续提案/广告对，增加 `--retention-epoch` 并根据需要更新权益/端点。
2. 执行  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   该命令通过以下方式验证未更改的功能/配置文件字段
   `AdmissionRecord::apply_renewal`，发出 `ProviderAdmissionRenewalV1`，并打印摘要
   治理日志。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. 替换 `torii.sorafs.admission_envelopes_dir` 中的先前信封，将更新 Norito/JSON 提交到治理存储库，并将更新哈希 + 保留纪元附加到 `docs/source/sorafs/migration_ledger.md`。
4. 通知操作员新信封已生效并监控 `torii_sorafs_admission_total{result="accepted",reason="stored"}` 以确认摄入。
5. 通过 `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` 重新生成并提交规范装置； CI (`ci/check_sorafs_fixtures.sh`) 验证 Norito 输出保持稳定。

#### 紧急撤销
1. 识别受损的信封并发出撤销：
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI 对 `ProviderAdmissionRevocationV1` 进行签名，通过以下方式验证签名集
   `verify_revocation_signatures`，并报告撤销摘要。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. 从 `torii.sorafs.admission_envelopes_dir` 中取出信封，将撤销 Norito/JSON 分发到准入缓存，并在治理分钟中记录原因哈希。
3.观看`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`以确认缓存删除了已撤销的广告；将撤销工件保留在事件回顾中。

## 测试和遥测- 在下面添加录取建议和信封的黄金装置
  `fixtures/sorafs_manifest/provider_admission/`。
- 扩展 CI (`ci/check_sorafs_fixtures.sh`) 以重新生成提案并验证信封。
- 生成的赛程包括带有规范摘要的 `metadata.json`；下游测试断言
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`。
- 提供集成测试：
  - Torii 拒绝录取信封丢失或过期的广告。
  - CLI 往返提案 → 信封 → 验证。
  - 治理续订轮换端点证明而不更改提供商 ID。
- 遥测要求：
  - 在 Torii 中发出 `provider_admission_envelope_{accepted,rejected}` 计数器。 ✅ `torii_sorafs_admission_total{result,reason}` 现在显示接受/拒绝的结果。
  - 在可观察性仪表板中添加到期警告（7 天内到期更新）。

## 后续步骤

1. ✅ 完成了 Norito 架构更改并在
   `sorafs_manifest::provider_admission`。不需要功能标志。
2. ✅ CLI 工作流程（`proposal`、`sign`、`verify`、`renewal`、`revoke`）通过集成测试进行记录和执行；使治理脚本与运行手册保持同步。
3. ✅ Torii 入场/发现摄取信封并暴露遥测计数器以进行接受/拒绝。
4. 注重可观察性：完成准入仪表板/警报，以便在 7 天内到期的续订引发警告（`torii_sorafs_admission_total`，到期仪表）。