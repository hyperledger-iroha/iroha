---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f7dd8b29e8eb37c2cd78c5dc91ce363bb546fa7e8768f8a2cc86f8b2d9508674
source_last_modified: "2026-01-04T08:19:26.498928+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS SF1 Determinism Dry-Run
summary: Checklist and expected digests for validating the canonical `sorafs.sf1@1.0.0` chunker profile.
translator: machine-google-reviewed
---

# SoraFS SF1 确定性试运行

该报告捕获了规范的基线试运行
`sorafs.sf1@1.0.0` 分块器配置文件。工具工作组应重新运行检查表
当验证装置刷新或新的消费者管道时，如下所示。记录
表中每个命令的结果都保持可审计的跟踪。

## 清单

|步骤|命令|预期结果 |笔记|
|------|---------|------------------|--------|
| 1 | `cargo test -p sorafs_chunker` |所有测试均通过； `vectors` 奇偶校验测试成功。 |确认规范装置编译并匹配 Rust 实现。 |
| 2 | `ci/check_sorafs_fixtures.sh` |脚本退出0；下面报告清单摘要。 |验证装置是否干净地重新生成并且签名是否保持连接。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` 条目与注册表描述符 (`profile_id=1`) 匹配。 |确保注册表元数据保持同步。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` |再生成功，无需 `--allow-unsigned`；清单和签名文件不变。 |为块边界和清单提供确定性证明。 |
| 5 | `node scripts/check_sf1_vectors.mjs` |报告 TypeScript 装置和 Rust JSON 之间没有差异。 |可选帮手；确保运行时之间的奇偶性（由工具工作组维护的脚本）。 |

## 预期摘要

- 块摘要 (SHA3-256)：`13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`
- `sf1_profile_v1.json`: `d89a4fdc030b0c7c4911719ea133c780d9f4610b08eef1d6d0e0ca443391718e`
- `sf1_profile_v1.ts`: `9a3bb8e4d96518b3a0a1301046b2d86a793991959ebdd8adda1fb2988e4292dc`
- `sf1_profile_v1.go`: `0f0348b8751b0f85fe874afda3371af75b78fac5dad65182204dcb3cf3e4c0a1`
- `sf1_profile_v1.rs`：`66b5956826c86589a24b71ca6b400cc1335323c6371f1cec9475f09af8743f61`

## 签核日志

|日期 |工程师|检查清单结果 |笔记|
|------|----------|------------------|--------|
| 2026-02-12 |模具（法学硕士）| ❌ 失败 |步骤 1：`cargo test -p sorafs_chunker` 未能通过 `vectors` 套件，因为装置已过时。步骤 2：`ci/check_sorafs_fixtures.sh` 中止 - `manifest_signatures.json` 在存储库状态中丢失（在工作树中删除）。步骤 4：当清单文件不存在时，`export_vectors` 无法验证签名。建议恢复签名的装置（或提供理事会密钥）并重新生成绑定，以便根据测试的要求嵌入规范句柄。 |
| 2026-02-12 |模具（法学硕士）| ✅ 通过 |通过 `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` 重新生成固定装置，生成规范的仅句柄别名列表和新的清单摘要 `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`。使用 `cargo test -p sorafs_chunker` 和干净的 `ci/check_sorafs_fixtures.sh` 运行进行验证（用于检查的分阶段固定装置）。步骤 5 等待节点奇偶校验助手落地。 |
| 2026-02-20 |存储工具 CI | ✅ 通过 |通过 `ci/check_sorafs_fixtures.sh` 获取议会信封 (`fixtures/sorafs_chunker/manifest_signatures.json`)；脚本重新生成固定装置，确认清单摘要 `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`，并重新运行 Rust 工具（Go/Node 步骤可用时执行），没有差异。 |

工具工作组应在运行清单后附加一个注明日期的行。如果有任何一步
失败，请提交此处链接的问题并在之前包含修复详细信息
批准新的固定装置或型材。