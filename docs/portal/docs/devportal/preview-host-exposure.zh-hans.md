---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82939c8aa73add3f3817490ab1a24bef5388c2fc5a00d19d76e4be6a3fa9c559
source_last_modified: "2025-12-29T18:16:35.111267+00:00"
translation_last_reviewed: 2026-02-07
id: preview-host-exposure
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
translator: machine-google-reviewed
---

DOCS‑SORA 路线图要求每个公开预览版都遵循相同的路线图
审核者在本地执行的校验和验证包。使用此操作手册
审阅者入职（和邀请批准票）完成后放置
测试版预览版主机上线。

## 先决条件

- 审阅者入职波已获得批准并登录到预览跟踪器中。
- `docs/portal/build/` 和校验和下存在最新的门户版本
  已验证（`build/checksums.sha256`）。
- SoraFS 预览凭证（Torii URL、权限、私钥、已提交
  纪元）存储在环境变量或 JSON 配置中，例如
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)。
- 使用所需主机名打开的 DNS 更改票证（`docs-preview.sora.link`，
  `docs.iroha.tech` 等）以及待命联系人。

## 步骤 1 – 构建并验证捆绑包

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

当校验和清单丢失或
篡改，对每个预览工件进行审核。

## 步骤 2 – 打包 SoraFS 工件

将静态站点转换为确定性 CAR/清单对。 `ARTIFACT_DIR`
默认为 `docs/portal/artifacts/`。

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

附上生成的`portal.car`、`portal.manifest.*`、描述符和校验和
清单到预览波票。

## 步骤 3 – 发布预览别名

准备好公开后，重新运行引脚助手 **不使用** `--skip-submit`
主机。提供 JSON 配置或显式 CLI 标志：

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

该命令写入 `portal.pin.report.json`，
`portal.manifest.submit.summary.json` 和 `portal.submit.response.json`，其中
必须与邀请证据包一起发货。

## 步骤 4 – 生成 DNS 切换计划

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

与 Ops 共享生成的 JSON，以便 DNS 交换机引用准确的 JSON
明显摘要。当重用较早的描述符作为回滚源时，
附加 `--previous-dns-plan path/to/previous.json`。

## 步骤 5 – 探测已部署的主机

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

探针确认所提供的发布标签、CSP 标头和签名元数据。
从两个区域重复该命令（或附加curl输出）以便审核员可以看到
边缘缓存是热的。

## 证据包

将以下工件包含在预览波票中并在
邀请电子邮件：

|文物|目的|
|----------|---------|
| `build/checksums.sha256` |证明捆绑包与 CI 构建相匹配。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` |规范 SoraFS 有效负载 + 清单。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` |显示清单提交+别名绑定成功。 |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS 元数据（票证、窗口、联系人）、路由升级 (`Sora-Route-Binding`) 摘要、`route_plan` 指针（计划 JSON + 标头模板）、缓存清除信息以及 Ops 的回滚指令。 |
| `artifacts/sorafs/preview-descriptor.json` |将存档 + 校验和捆绑在一起的签名描述符。 |
| `probe` 输出 |确认实时主机公布预期的发布标签。 |

主持人上线后，请按照 [预览邀请手册](./public-preview-invite.md) 操作
分发链接、记录邀请和监控遥测。