---
lang: zh-hant
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 數據可用性威脅模型自動化 (DA-1)

路線圖項目 DA-1 和 `status.md` 要求確定性自動化循環
產生 Norito PDP/PoTR 威脅模型摘要
`docs/source/da/threat_model.md` 和 Docusaurus 鏡像。這個目錄
捕獲以下所引用的文物：

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model`（運行 `scripts/docs/render_da_threat_model_tables.py`）
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## 流量

1. **生成報告**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON摘要記錄了模擬的複制失敗率，chunker
   閾值以及 PDP/PoTR 工具檢測到的任何策略違規
   `integration_tests/src/da/pdp_potr.rs`。
2. **渲染 Markdown 表格**
   ```bash
   make docs-da-threat-model
   ```
   這運行 `scripts/docs/render_da_threat_model_tables.py` 來重寫
   `docs/source/da/threat_model.md` 和 `docs/portal/docs/da/threat-model.md`。
3. **通過將 JSON 報告（和可選的 CLI 日誌）複製到
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`。當
   治理決策依賴於特定的運行，包括 git 提交哈希和
   模擬器種子位於同級 `<timestamp>-metadata.md` 中。

## 證據預期

- JSON 文件應保持 <100 KiB，以便它們可以存在於 git 中。更大的執行力
  痕跡屬於外部存儲——在元數據中引用它們的簽名哈希
  如果需要請注意。
- 每個存檔文件必須列出種子、配置路徑和模擬器版本，以便
  在審核 DA 發布門時可以準確地重現重播。
- 隨時從 `status.md` 或路線圖條目鏈接回存檔文件
  DA-1 驗收標準提前，確保評審員能夠驗證
  基線，無需重新運行線束。

## 承諾協調（排序器省略）

使用 `cargo xtask da-commitment-reconcile` 將 DA 攝取收據與
DA承諾記錄，捕獲定序器遺漏或篡改：

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- 接受 Norito 或 JSON 形式的收據和承諾
  `SignedBlockWire`、`.norito` 或 JSON 捆綁包。
- 當塊日誌中缺少任何票據或散列出現分歧時失敗；
  當您有意範圍時，`--allow-unexpected` 會忽略僅塊票證
  收據集。
- 將發出的 JSON 附加到治理數據包/Alertmanager 以進行省略
  警報；默認為 `artifacts/da/commitment_reconciliation.json`。

## 權限審核（季度訪問審核）

使用 `cargo xtask da-privilege-audit` 掃描 DA 清單/重播目錄
（加上可選的額外路徑）用於丟失、非目錄或全局可寫
條目：

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- 從提供的 Torii 配置中讀取 DA 攝取路徑並檢查 Unix
  權限（如果有）。
- 標記缺失/非目錄/全局可寫路徑並返回非零退出
  存在問題時的代碼。
- 簽署並附加 JSON 捆綁包 (`artifacts/da/privilege_audit.json` by
  默認）每季度訪問審查數據包和儀表板。