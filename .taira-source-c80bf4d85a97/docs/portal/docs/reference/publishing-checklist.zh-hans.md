---
lang: zh-hans
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d7b44d46ef97c20058221aedf1f0b4a27ba85d204c3be4fe4933da31d9e207
source_last_modified: "2025-12-29T18:16:35.160066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 发布清单

每当您更新开发者门户时，请使用此清单。它确保了
CI 构建、GitHub Pages 部署和手动冒烟测试涵盖了每个部分
在发布或路线图里程碑落地之前。

## 1. 本地验证

- `npm run sync-openapi -- --version=current --latest`（添加一个或多个
  当 Torii OpenAPI 更改为冻结快照时，`--mirror=<label>` 进行标记）。
- `npm run build` – 确认 `Build on Iroha with confidence` 英雄副本仍然存在
  出现在 `build/index.html` 中。
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – 验证
  校验和清单（测试下载的 CI 时添加 `--descriptor`/`--archive`
  文物）。
- `npm run serve` – 启动校验和门控预览助手来验证
  在调用 `docusaurus serve` 之前查看清单，因此审阅者永远不会浏览
  未签名的快照（`serve:verified` 别名保留用于显式调用）。
- 抽查您通过 `npm run start` 触及的降价和实时重新加载
  服务器。

## 2. 拉取请求检查

- 验证 `docs-portal-build` 作业在 `.github/workflows/check-docs.yml` 中成功。
- 确认 `ci/check_docs_portal.sh` 运行（CI 日志显示英雄烟雾检查）。
- 确保预览工作流程上传清单 (`build/checksums.sha256`) 并
  预览验证脚本成功（CI 日志显示
  `scripts/preview_verify.sh` 输出）。
- 将已发布的预览 URL 从 GitHub Pages 环境添加到 PR
  描述。

## 3. 部分签核

|部分|业主|清单 |
|--------|---------|------------|
|主页 |开发版本 |英雄副本渲染、快速入门卡链接到有效路线、CTA 按钮解析。 |
| Norito | Norito 工作组 |概述和入门指南引用了最新的 CLI 标志和 Norito 架构文档。 |
| SoraFS |存储团队|快速启动运行完成，记录清单报告字段，验证获取模拟指令。 |
| SDK 指南 | SDK 线索 | Rust/Python/JS 指南编译当前示例并链接到实时存储库。 |
|参考|文档/开发版本 |索引列出了最新规格，Norito 编解码器参考与 `norito.md` 匹配。 |
|预览神器|文档/开发版本 | `docs-portal-preview` 工件附加到 PR、烟雾检查通过、与审阅者共享的链接。 |
|安全与尝试沙箱 |文档/DevRel · 安全 |配置 OAuth 设备代码登录 (`DOCS_OAUTH_*`)、执行 `security-hardening.md` 检查表、通过 `npm run build` 或 `npm run probe:portal` 验证 CSP/受信任类型标头。 |

将每一行标记为 PR 审核的一部分，或记下任何后续任务，以便了解状态
跟踪保持准确。

## 4. 发行说明

- 包括 `https://docs.iroha.tech/`（或环境 URL
  从部署作业）在发行说明和状态更新中。
- 明确指出任何新的或更改的部分，以便下游团队知道在哪里
  重新运行他们自己的冒烟测试。