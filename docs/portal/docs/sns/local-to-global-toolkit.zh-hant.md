---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4493e69ce57c4f691f368fb13c1bbe96e2c73991dfb39045753b5652d2f10a9
source_last_modified: "2026-01-28T17:11:30.702818+00:00"
translation_last_reviewed: 2026-02-07
title: Local → Global Address Toolkit
translator: machine-google-reviewed
---

此頁面鏡像 [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
來自單一倉庫。它打包了路線圖項 **ADDR-5c** 所需的 CLI 幫助程序和 Runbook。

## 概述

- `scripts/address_local_toolkit.sh` 包裝 `iroha` CLI 以生成：
  - `audit.json` — `iroha tools address audit --format json` 的結構化輸出。
  - `normalized.txt` — 每個本地域選擇器的已轉換首選 IH58/第二佳壓縮 (`sora`) 文字。
- 將腳本與地址提取儀表板配對 (`dashboards/grafana/address_ingest.json`)
  和Alertmanager規則（`dashboards/alerts/address_ingest_rules.yml`）來證明Local-8 /
  Local-12 切換是安全的。觀看 Local-8 和 Local-12 碰撞面板以及
  `AddressLocal8Resurgence`、`AddressLocal12Collision` 和 `AddressInvalidRatioSlo` 之前的警報
  促進明顯的變化。
- 參考[地址顯示指南](address-display-guidelines.md) 和
  [地址清單操作手冊](../../../source/runbooks/address_manifest_ops.md)，用於用戶體驗和事件響應上下文。

## 用法

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format ih58
```

選項：

- `--format compressed` 用於 `sora…` 輸出而不是 IH58。
- `--no-append-domain` 發出裸文字。
- `--audit-only` 跳過轉換步驟。
- `--allow-errors` 在出現格式錯誤的行時繼續掃描（與 CLI 行為匹配）。

該腳本在運行結束時寫入工件路徑。將兩個文件附加到
您的變更管理票以及證明為零的 Grafana 屏幕截圖
≥30 天的 Local-8 檢測和零 Local-12 衝突。

## CI 集成

1. 在專用作業中運行腳本並上傳其輸出。
2. 當 `audit.json` 報告本地選擇器 (`domain.kind = local12`) 時阻止合併。
   以其默認的 `true` 值（僅在開發/測試集群上覆蓋 `false` 時）
   診斷回歸）並添加
   `iroha tools address normalize --fail-on-warning --only-local` 到 CI 所以回歸
   在投入生產之前嘗試失敗。

請參閱源文檔以了解更多詳細信息、示例證據清單以及在向客戶宣布切換時可以重複使用的發行說明片段。