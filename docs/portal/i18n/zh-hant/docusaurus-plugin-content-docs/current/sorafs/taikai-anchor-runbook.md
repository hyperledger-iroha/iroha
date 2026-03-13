---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Taikai Anchor 可觀測性運行手冊

此門戶副本反映了規范運行手冊
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md)。
在排練 SN13-C 路由清單 (TRM) 錨點時使用它，以便 SoraFS/SoraNet
操作員可以將線軸偽影、Prometheus 遙測和治理關聯起來
無需離開門戶預覽版本即可獲得證據。

## 範圍和所有者

- **程序：** SN13-C — Taikai 清單和 SoraNS 錨。
- **所有者：** 媒體平台工作組、DA 計劃、網絡 TL、文檔/開發版本。
- **目標：** 為 Sev1/Sev2 警報、遙測提供確定性劇本
  Taikai 路由清單前滾時的驗證和證據捕獲
  跨別名。

## 快速入門 (Sev1/Sev2)

1. **捕獲線軸工件** — 複製最新的
   `taikai-anchor-request-*.json`、`taikai-trm-state-*.json` 和
   `taikai-lineage-*.json` 文件來自
   重新啟動工作程序之前的 `config.da_ingest.manifest_store_dir/taikai/`。
2. **轉儲 `/status` 遙測** — 記錄
   `telemetry.taikai_alias_rotations` 數組來證明哪個清單窗口
   活躍：
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **檢查儀表板和警報** — 加載
   `dashboards/grafana/taikai_viewer.json`（集群+流過濾器）和註釋
   是否有任何規則
   `dashboards/alerts/taikai_viewer_rules.yml` 已解僱（`TaikaiLiveEdgeDrift`，
   `TaikaiIngestFailure`、`TaikaiCekRotationLag`、SoraFS 健康證明事件）。
4. **檢查 Prometheus** — 運行§“指標參考”中的查詢以確認
   攝取延遲/漂移和別名旋轉計數器的行為符合預期。升級
   如果 `taikai_trm_alias_rotations_total` 因多個窗口而停頓或者如果
   錯誤計數器增加。

## 指標參考

|公制|目的|
| ---| ---|
| `taikai_ingest_segment_latency_ms` |每個集群/流的 CMAF 攝取延遲直方圖（目標：p95<750ms，p99<900ms）。 |
| `taikai_ingest_live_edge_drift_ms` |編碼器和錨定工作人員之間的實時邊緣漂移（p99>1.5 秒的頁面持續 10 分鐘）。 |
| `taikai_ingest_segment_errors_total{reason}` |按原因列出的錯誤計數器（`decode`、`manifest_mismatch`、`lineage_replay`，...）。任何增加都會觸發 `TaikaiIngestFailure`。 |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` |每當 `/v2/da/ingest` 接受別名的新 TRM 時遞增；使用 `rate()` 驗證旋轉節奏。 |
| `/status → telemetry.taikai_alias_rotations[]` |包含 `window_start_sequence`、`window_end_sequence`、`manifest_digest_hex`、`rotations_total` 和證據包時間戳的 JSON 快照。 |
| `taikai_viewer_*`（再緩衝、CEK 輪換年齡、PQ 運行狀況、警報）|查看器端 KPI，確保 CEK 輪換 + PQ 電路在錨定期間保持健康。 |

### PromQL 片段

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## 儀表板和警報

- **Grafana 查看器板：** `dashboards/grafana/taikai_viewer.json` — p95/p99
  延遲、實時邊緣漂移、段錯誤、CEK 旋轉年齡、查看器警報。
- **Grafana 緩存板：** `dashboards/grafana/taikai_cache.json` — 熱/暖/冷
  別名窗口輪換時的提升和 QoS 拒絕。
- **Alertmanager 規則：** `dashboards/alerts/taikai_viewer_rules.yml` — 漂移
  尋呼、攝取失敗警告、CEK 輪換延遲和 SoraFS 健康證明
  懲罰/冷卻時間。確保每個生產集群都存在接收器。

## 證據包清單

- 線軸文物（`taikai-anchor-request-*`、`taikai-trm-state-*`、
  `taikai-lineage-*`）。
- 運行 `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` 以發出待處理/已交付信封的簽名 JSON 庫存，並將請求/SSM/TRM/沿襲文件複製到鑽取包中。默認假脫機路徑是 `torii.toml` 中的 `storage/da_manifests/taikai`。
- `/status` 快照覆蓋 `telemetry.taikai_alias_rotations`。
- Prometheus 通過事件窗口導出上述指標 (JSON/CSV)。
- Grafana 屏幕截圖，過濾器可見。
- 引用相關規則的 Alertmanager ID 會觸發。
- 鏈接至 `docs/examples/taikai_anchor_lineage_packet.md` 描述
  規範證據包。

## 儀表板鏡像和練習節奏

滿足SN13-C路線圖要求意味著證明Taikai
查看器/緩存儀表板反映在門戶**和錨點內部**
證據演習按照可預測的節奏進行。

1. **門戶鏡像。 ** 每當 `dashboards/grafana/taikai_viewer.json` 或
   `dashboards/grafana/taikai_cache.json` 更改，總結增量
   `sorafs/taikai-monitoring-dashboards`（此門戶）並記下 JSON
   門戶 PR 描述中的校驗和。突出顯示新面板/閾值，以便
   審閱者可以與託管的 Grafana 文件夾關聯。
2. **每月演習。 **
   - 在每月第一個星期二 15:00 UTC 進行演習，以便提供證據
     在 SN13 治理同步之前著陸。
   - 捕獲線軸文物、`/status` 遙測和 Grafana 內部屏幕截圖
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`。
   - 記錄執行情況
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`。
3. **審核並發布。 ** 在 48 小時內，通過
   DA計劃+NetOps，在演練日誌中記錄後續項目，並鏈接
   治理存儲桶從 `docs/source/sorafs/runbooks-index.md` 上傳。

如果儀表板或鑽頭落後，SN13-C 無法退出🈺；保留這個
每當節奏或證據預期發生變化時，該部分都會更新。

## 有用的命令

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

每當 Taikai 時，請保持此門戶副本與規範操作手冊同步
錨定遙測、儀表板或治理證據需求發生變化。