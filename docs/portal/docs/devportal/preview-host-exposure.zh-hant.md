---
lang: zh-hant
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

DOCS‑SORA 路線圖要求每個公開預覽版都遵循相同的路線圖
審核者在本地執行的校驗和驗證包。使用此操作手冊
審閱者入職（和邀請批准票）完成後放置
測試版預覽版主機上線。

## 先決條件

- 審閱者入職波已獲得批准並登錄到預覽跟踪器中。
- `docs/portal/build/` 和校驗和下存在最新的門戶版本
  已驗證（`build/checksums.sha256`）。
- SoraFS 預覽憑證（Torii URL、權限、私鑰、已提交
  紀元）存儲在環境變量或 JSON 配置中，例如
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)。
- 使用所需主機名打開的 DNS 更改票證（`docs-preview.sora.link`，
  `docs.iroha.tech` 等）以及待命聯繫人。

## 步驟 1 – 構建並驗證捆綁包

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

當校驗和清單丟失或
篡改，對每個預覽工件進行審核。

## 步驟 2 – 打包 SoraFS 工件

將靜態站點轉換為確定性 CAR/清單對。 `ARTIFACT_DIR`
默認為 `docs/portal/artifacts/`。

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

附上生成的`portal.car`、`portal.manifest.*`、描述符和校驗和
清單到預覽波票。

## 步驟 3 – 發布預覽別名

準備好公開後，重新運行引腳助手 **不使用** `--skip-submit`
主機。提供 JSON 配置或顯式 CLI 標誌：

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

該命令寫入 `portal.pin.report.json`，
`portal.manifest.submit.summary.json` 和 `portal.submit.response.json`，其中
必須與邀請證據包一起發貨。

## 步驟 4 – 生成 DNS 切換計劃

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

與 Ops 共享生成的 JSON，以便 DNS 交換機引用準確的 JSON
明顯摘要。當重用較早的描述符作為回滾源時，
附加 `--previous-dns-plan path/to/previous.json`。

## 步驟 5 – 探測已部署的主機

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

探針確認所提供的發布標籤、CSP 標頭和簽名元數據。
從兩個區域重複該命令（或附加curl輸出）以便審核員可以看到
邊緣緩存是熱的。

## 證據包

將以下工件包含在預覽波票中並在
邀請電子郵件：

|文物|目的|
|----------|---------|
| `build/checksums.sha256` |證明捆綁包與 CI 構建相匹配。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` |規範 SoraFS 有效負載 + 清單。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` |顯示清單提交+別名綁定成功。 |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS 元數據（票證、窗口、聯繫人）、路由升級 (`Sora-Route-Binding`) 摘要、`route_plan` 指針（計劃 JSON + 標頭模板）、緩存清除信息以及 Ops 的回滾指令。 |
| `artifacts/sorafs/preview-descriptor.json` |將存檔 + 校驗和捆綁在一起的簽名描述符。 |
| `probe` 輸出 |確認實時主機公佈預期的發布標籤。 |

主持人上線後，請按照 [預覽邀請手冊](./public-preview-invite.md) 操作
分發鏈接、記錄邀請和監控遙測。