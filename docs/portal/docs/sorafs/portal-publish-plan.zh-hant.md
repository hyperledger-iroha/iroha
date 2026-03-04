---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd5fff7302924f71ca19593cbbcc29352c00f286ab5bc555d4654e2dc43c3daa
source_last_modified: "2026-01-22T16:26:46.525444+00:00"
translation_last_reviewed: 2026-02-07
id: portal-publish-plan
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
translator: machine-google-reviewed
---

:::注意規範來源
鏡子 `docs/source/sorafs/portal_publish_plan.md`。當工作流程發生變化時更新兩個副本。
:::

路線圖項目 DOCS-7 需要每個文檔 artefact（門戶構建、OpenAPI 規範、
SBOM）流經 SoraFS 清單管道並通過 `docs.sora` 提供服務
帶有 `Sora-Proof` 標頭。該清單將現有的助手縫合在一起
因此 Docs/DevRel、Storage 和 Ops 可以運行版本而無需搜索
多個運行手冊。

## 1. 構建和打包有效負載

運行打包助手（試運行時可以使用跳過選項）：

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- 如果 CI 已經生成了 `--skip-build`，則重用 `docs/portal/build`。
- 當 `syft` 不可用時添加 `--skip-sbom`（例如，氣隙排練）。
- 該腳本運行門戶測試，發出 `portal` 的 CAR + 清單對，
  `openapi`、`portal-sbom` 和 `openapi-sbom`，在以下情況下驗證每個 CAR：
  設置 `--proof`，並在設置 `--sign` 時丟棄 Sigstore 捆綁包。
- 輸出結構：

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

保留整個文件夾（或通過 `artifacts/devportal/sorafs/latest` 的符號鏈接）
治理審查者可以跟踪構建工件。

## 2. Pin 清單 + 別名

使用 `sorafs_cli manifest submit` 將清單推送到 Torii 並綁定別名。
將 `${SUBMITTED_EPOCH}` 設置為最新的共識紀元（從
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` 或您的儀表板）。

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="ih58..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- 對 `openapi.manifest.to` 和 SBOM 清單重複（省略別名標誌）
  SBOM 捆綁，除非治理分配命名空間）。
- 替代方案：`iroha app sorafs pin register` 適用於提交的摘要
  摘要（如果二進製文件已安裝）。
- 驗證註冊表狀態
  `iroha app sorafs pin list --alias docs:portal --format json | jq`。
- 要觀看的儀表板：`sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  指標）。

## 3. 網關標頭和證明

生成HTTP標頭塊+綁定元數據：

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- 模板包括 `Sora-Name`、`Sora-CID`、`Sora-Proof` 和
  `Sora-Proof-Status` 標頭加上默認的 CSP/HSTS/Permissions-Policy。
- 使用 `--rollback-manifest-json` 渲染成對的回滾標頭集。

在公開流量之前，運行：

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- 探針強制執行 GAR 簽名新鮮度、別名策略和 TLS 證書
  指紋。
- 自認證線束下載帶有 `sorafs_fetch` 的清單並存儲
  CAR重播日誌；保留審計證據的輸出。

## 4. DNS 和遙測護欄

1. 刷新 DNS 框架，以便治理可以證明綁定：

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. 部署期間監控：

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   儀表板：`sorafs_gateway_observability.json`，
   `sorafs_fetch_observability.json`，以及引腳註冊板。

3. 刪除警報規則 (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) 並
   捕獲發布存檔的日誌/屏幕截圖。

## 5. 證據包

在發布票證或治理包中包含以下內容：

- `artifacts/devportal/sorafs/<stamp>/`（CAR、清單、SBOM、證明、
  Sigstore 捆綁包，提交摘要）。
- 網關探頭+自認證輸出
  （`artifacts/sorafs_gateway_probe/<stamp>/`，
  `artifacts/sorafs_gateway_self_cert/<stamp>/`）。
- DNS 骨架 + 標頭模板 (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`、`portal.dns-cutover.json`）。
- 儀表板屏幕截圖+警報確認。
- `status.md` 更新引用清單摘要和別名綁定時間。

遵循此清單可提供 DOCS-7：portal/OpenAPI/SBOM 有效負載是
確定性打包，用別名固定，由 `Sora-Proof` 保護
標頭，並通過現有的可觀察性堆棧進行端到端監控。