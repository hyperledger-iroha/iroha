---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 182b1616ff32d0be52d0a6e33178fac4261e7e92a088048b11e5f93e4b005750
source_last_modified: "2026-01-05T09:28:11.838358+00:00"
translation_last_reviewed: 2026-02-07
id: preview-integrity-plan
title: Checksum-Gated Preview Plan
sidebar_label: Preview Integrity Plan
description: Implementation roadmap for securing the docs portal preview pipeline with checksum validation and notarised artefacts.
translator: machine-google-reviewed
---

该计划概述了使每个门户预览制品在发布前可验证所需的剩余工作。目标是保证审阅者下载 CI 中内置的准确快照、校验和清单不可变，并且可通过 SoraFS 和 Norito 元数据发现预览。

## 目标

- **确定性构建：** 确保 `npm run build` 产生可重现的输出并始终发出 `build/checksums.sha256`。
- **经过验证的预览：** 要求每个预览制品都附带校验和清单，并在验证失败时拒绝发布。
- **Norito-发布的元数据：** 将预览描述符（提交元数据、校验和摘要、SoraFS CID）保留为 Norito JSON，以便治理工具可以审核版本。
- **运营商工具：**提供消费者可以在本地运行的一步验证脚本（`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`）；该脚本现在端到端地包装了校验和+描述符验证流程。标准预览命令 (`npm run serve`) 现在会在 `docusaurus serve` 之前自动调用此帮助程序，因此本地快照保持校验和门控（将 `npm run serve:verified` 保留为显式别名）。

## 第一阶段——CI 执行

1. 将 `.github/workflows/docs-portal-preview.yml` 更新为：
   - 在 Docusaurus 构建之后运行 `node docs/portal/scripts/write-checksums.mjs`（已在本地调用）。
   - 执行 `cd build && sha256sum -c checksums.sha256` 并因不匹配而使作业失败。
   - 将构建目录打包为 `artifacts/preview-site.tar.gz`，复制校验和清单，调用 `scripts/generate-preview-descriptor.mjs`，并使用 JSON 配置执行 `scripts/sorafs-package-preview.sh`（请参阅 `docs/examples/sorafs_preview_publish.json`），以便工作流程发出元数据和确定性 SoraFS 包。
   - 上传静态站点、元数据工件（`docs-portal-preview`、`docs-portal-preview-metadata`）和 SoraFS 捆绑包（`docs-portal-preview-sorafs`），以便可以检查清单、CAR 摘要和计划，而无需重新运行构建。
2. 添加 CI 徽章注释，总结拉取请求中的校验和验证结果（✅ 通过 `docs-portal-preview.yml` GitHub 脚本注释步骤实现）。
3. 在 `docs/portal/README.md`（CI 部分）中记录工作流程，并链接到发布清单中的验证步骤。

## 验证脚本

`docs/portal/scripts/preview_verify.sh` 验证下载的预览工件，无需手动 `sha256sum` 调用。共享本地快照时，使用 `npm run serve`（或显式 `npm run serve:verified` 别名）运行脚本并一步启动 `docusaurus serve`。验证逻辑：

1. 针对 `build/checksums.sha256` 运行适当的 SHA 工具（`sha256sum` 或 `shasum -a 256`）。
2. （可选）比较预览描述符的 `checksums_manifest` 摘要/文件名和预览存档摘要/文件名（如果提供）。
3. 当检测到任何不匹配时退出非零，以便审阅者可以阻止被篡改的预览。

示例用法（提取 CI 工件后）：

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI 和发布工程师在下载预览包或将工件附加到发布票证时都应该调用该脚本。

## 第 2 阶段 — SoraFS 发布

1. 通过以下作业扩展预览工作流程：
   - 使用 `sorafs_cli car pack` 和 `manifest submit` 将构建的站点上传到 SoraFS 临时网关。
   - 捕获返回的清单摘要和 SoraFS CID。
   - 将 `{ commit, branch, checksum_manifest, cid }` 序列化为 Norito JSON (`docs/portal/preview/preview_descriptor.json`)。
2. 将描述符与构建工件一起存储，并在拉取请求注释中公开 CID。
3. 添加在空运行模式下执行 `sorafs_cli` 的集成测试，以确保未来的更改保持元数据架构稳定。

## 第 3 阶段 — 治理和审计

1. 发布 Norito 模式 (`PreviewDescriptorV1`)，描述 `docs/portal/schemas/` 下的描述符结构。
2. 更新 DOCS-SORA 发布清单以要求：
   - 针对上传的 CID 运行 `sorafs_cli manifest verify`。
   - 在发布 PR 描述中记录校验和清单摘要和 CID。
3. 连接治理自动化，以在发布投票期间根据校验和清单交叉检查描述符。

## 可交付成果和所有权

|里程碑|所有者 |目标|笔记|
|------------|----------|--------|--------|
| CI 校验和执行落地 |文档 基础设施 |第 1 周 |添加故障门+工件上传。 |
| SoraFS 预览发布 |文档 基础设施/存储团队 |第 2 周 |需要访问暂存凭据和 Norito 架构更新。 |
|治理整合|文档/DevRel 主管/治理工作组 |第 3 周 |发布架构+更新清单和路线图条目。 |

## 开放问题

- 哪个 SoraFS 环境应保存预览工件（暂存与专用预览通道）？
- 发布前预览描述符是否需要双重签名（Ed25519 + ML-DSA）？
- 在运行 `sorafs_cli` 时，CI 工作流程是否应该固定协调器配置 (`orchestrator_tuning.json`) 以保持清单的可重复性？

在 `docs/portal/docs/reference/publishing-checklist.md` 中捕获决策，并在解决未知问题后更新此计划。