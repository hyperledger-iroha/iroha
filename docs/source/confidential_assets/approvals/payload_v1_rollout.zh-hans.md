---
lang: zh-hans
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ Payload v1 推出批准（SDK 委员会，2026-04-28）。
//！
//！捕获 `roadmap.md:M1` 所需的 SDK 委员会决策备忘录，以便
//！加密有效负载 v1 部署具有可审核记录（可交付 M1.4）。

# Payload v1 推出决定 (2026-04-28)

- **主席：** SDK 委员会负责人 (M. Takemiya)
- **投票成员：** Swift Lead、CLI 维护者、Confidential Assets TL、DevRel WG
- **观察员：** 计划管理、遥测操作

## 已审核的输入

1. **Swift 绑定和提交者** — `ShieldRequest`/`UnshieldRequest`、异步提交者和 Tx 构建器助手通过奇偶校验测试和文档登陆。【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI 人体工程学** — `iroha app zk envelope` 帮助程序涵盖编码/检查工作流程以及故障诊断，符合路线图人体工程学要求。【crates/iroha_cli/src/zk.rs:1256】
3. **确定性固定装置和奇偶校验套件** - 共享固定装置 + Rust/Swift 验证以保留 Norito 字节/错误表面对齐。【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## 决定

- **批准 SDK 和 CLI 的有效负载 v1 推出**，使 Swift 钱包能够在无需定制管道的情况下生成机密信封。
- **条件：** 
  - 将奇偶校验装置置于 CI 漂移警报之下（与 `scripts/check_norito_bindings_sync.py` 相关）。
  - 在 `docs/source/confidential_assets.md` 中记录操作手册（已通过 Swift SDK PR 更新）。
  - 在翻转任何生产标志之前记录校准+遥测证据（在 M2 下跟踪）。

## 行动项目

|业主|项目 |到期|
|--------|------|-----|
|迅速领先 |宣布 GA 可用性 + 自述文件片段 | 2026-05-01 |
| CLI 维护者 |添加 `iroha app zk envelope --from-fixture` 帮助程序（可选）|积压（不阻塞）|
|开发相关工作组 |使用有效负载 v1 说明更新钱包快速入门 | 2026-05-05 |

> **注：** 本备忘录取代 `roadmap.md:2426` 中临时的“等待理事会批准”标注，并满足跟踪器项目 M1.4。每当后续行动项目结束时更新 `status.md`。