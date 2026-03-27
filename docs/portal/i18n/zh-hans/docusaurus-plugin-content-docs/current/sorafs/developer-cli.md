---
id: developer-cli
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

合并的 `sorafs_cli` 表面（由 `sorafs_car` 板条箱提供，
启用 `cli` 功能）公开准备 SoraFS 所需的每个步骤
文物。使用这本食谱可以直接跳转到常见的工作流程；将其与
用于操作上下文的清单管道和编排器运行手册。

## 包有效负载

使用 `car pack` 生成确定性 CAR 档案和块计划。的
除非提供句柄，否则命令会自动选择 SF-1 分块器。

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- 默认分块器句柄：`sorafs.sf1@1.0.0`。
- 目录输入按字典顺序排列，因此校验和保持稳定
  跨平台。
- JSON 摘要包括有效负载摘要、每个块元数据和根
  由注册表和协调器识别的 CID。

## 构建清单

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` 选项直接映射到 `PinPolicy` 字段
  `sorafs_manifest::ManifestBuilder`。
- 当您希望 CLI 重新计算 SHA3 块时提供 `--chunk-plan`
  提交前先进行消化；否则它会重用嵌入的摘要
  总结。
- JSON 输出镜像 Norito 有效负载，以便在
  评论。

## 在没有长期密钥的情况下签署清单

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- 接受内联标记、环境变量或基于文件的源。
- 添加出处元数据（`token_source`、`token_hash_hex`、块摘要）
  除非 `--include-token=true`，否则不会保留原始 JWT。
- 在 CI 中运行良好：通过设置与 GitHub Actions OIDC 结合
  `--identity-token-provider=github-actions`。

## 提交清单至 Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- 对别名证明执行 Norito 解码并验证它们是否匹配
  在发布到 Torii 之前清单摘要。
- 根据计划重新计算块 SHA3 摘要以防止不匹配攻击。
- 响应摘要捕获 HTTP 状态、标头和注册表有效负载
  后期审核。

## 验证CAR内容和证明

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- 重建 PoR 树并将有效负载摘要与清单摘要进行比较。
- 捕获提交复制证明时所需的计数和标识符
  到治理。

## 流证明遥测

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- 为每个流式证明发出 NDJSON 项目（禁用重播）
  `--emit-events=false`）。
- 聚合成功/失败计数、延迟直方图和采样失败
  摘要 JSON，以便仪表板可以绘制结果而无需抓取日志。
- 当网关报告故障或本地 PoR 验证时退出非零
  （通过 `--por-root-hex`）拒绝证明。调整阈值
  `--max-failures` 和 `--max-verification-failures` 用于排练。
- 今天支持PoR； PDP 和 PoTR 在 SF-13/SF-14 后重复使用相同的信封
  土地。
- `--governance-evidence-dir` 写入渲染的摘要、元数据（时间戳、
  CLI 版本、网关 URL、清单摘要）以及清单的副本
  提供的目录，以便治理数据包可以存档证明流
  无需重播运行即可获得证据。

## 其他参考资料

- `docs/source/sorafs_cli.md` — 详尽的标志文档。
- `docs/source/sorafs_proof_streaming.md` — 证明遥测模式和 Grafana
  仪表板模板。
- `docs/source/sorafs/manifest_pipeline.md` — 深入探讨分块、清单
  组成和 CAR 处理。