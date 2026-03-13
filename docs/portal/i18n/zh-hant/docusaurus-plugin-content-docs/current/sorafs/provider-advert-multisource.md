---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 多源提供商廣告和日程安排

此頁面提煉了規範規範
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
使用該文檔逐字記錄 Norito 架構和變更日誌；門戶副本
使操作員指南、SDK 說明和遙測參考與其他內容保持一致
SoraFS 運行手冊。

## Norito 架構添加

### 範圍能力 (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – 每個請求的最大連續跨度（字節），`≥ 1`。
- `min_granularity` – 尋求分辨率，`1 ≤ value ≤ max_chunk_span`。
- `supports_sparse_offsets` – 允許一個請求中的非連續偏移。
- `requires_alignment` – 當為 true 時，偏移量必須與 `min_granularity` 對齊。
- `supports_merkle_proof` – 表示 PoR 見證支持。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` 強制規範編碼
因此八卦有效負載仍然是確定性的。

### `StreamBudgetV1`
- 字段：`max_in_flight`、`max_bytes_per_sec`、可選 `burst_bytes`。
- 驗證規則（`StreamBudgetV1::validate`）：
  - `max_in_flight ≥ 1`，`max_bytes_per_sec > 0`。
  - `burst_bytes`（如果存在）必須是 `> 0` 和 `≤ max_bytes_per_sec`。

### `TransportHintV1`
- 字段：`protocol: TransportProtocol`、`priority: u8`（0-15 窗口由
  `TransportHintV1::validate`）。
- 已知協議：`torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- 每個提供商的重複協議條目將被拒絕。

### `ProviderAdvertBodyV1` 添加
- 可選 `stream_budget: Option<StreamBudgetV1>`。
- 可選 `transport_hints: Option<Vec<TransportHintV1>>`。
- 這兩個領域現在都流經 `ProviderAdmissionProposalV1`，治理
  信封、CLI 裝置和遙測 JSON。

## 驗證和治理綁定

`ProviderAdvertBodyV1::validate` 和 `ProviderAdmissionProposalV1::validate`
拒絕格式錯誤的元數據：

- 範圍能力必須解碼並滿足跨度/粒度限制。
- 流預算/傳輸提示需要匹配
  `CapabilityType::ChunkRangeFetch` TLV 和非空提示列表。
- 重複的傳輸協議和無效的優先級提高了驗證性
  廣告八卦之前的錯誤。
- 錄取信封通過以下方式比較範圍元數據的提案/廣告
  `compare_core_fields` 因此不匹配的八卦有效負載會被提前拒絕。

回歸覆蓋率位於
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## 工裝和固定裝置

- 提供商廣告負載必須包含 `range_capability`、`stream_budget` 和
  `transport_hints` 元數據。通過 `/v2/sorafs/providers` 響應進行驗證並
  入場裝置； JSON 摘要應包括解析的功能，
  流預算和用於遙測攝取的提示數組。
- `cargo xtask sorafs-admission-fixtures` 表面流預算和傳輸
  其 JSON 工件內部有提示，以便儀表板跟踪功能採用情況。
- `fixtures/sorafs_manifest/provider_admission/` 下的賽程現在包括：
  - 規範的多源廣告，
  - `multi_fetch_plan.json`，因此 SDK 套件可以重播確定性的多點
    獲取計劃。

## Orchestrator 和 Torii 集成

- Torii `/v2/sorafs/providers` 返回解析的範圍能力元數據
  與 `stream_budget` 和 `transport_hints`。降級警告在以下情況下觸發：
  提供者省略新的元數據，網關範圍端點強制執行相同的元數據
  直接客戶的限制。
- 多源編排器 (`sorafs_car::multi_fetch`) 現在強制範圍
  分配工作時的限制、能力調整和流預算。單位
  測試涵蓋了塊太大、稀疏搜索和限制場景。
- `sorafs_car::multi_fetch` 流降級信號（對齊失敗、
  限制請求），以便操作員可以追踪特定提供商的原因
  計劃時跳過。

## 遙測參考

Torii 範圍獲取儀器提供 **SoraFS 獲取可觀測性**
Grafana 儀表板 (`dashboards/grafana/sorafs_fetch_observability.json`) 和
配對警報規則 (`dashboards/alerts/sorafs_fetch_rules.yml`)。

|公制|類型 |標籤|描述 |
|--------|------|--------|------------|
| `torii_sorafs_provider_range_capability_total` |儀表| `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`) |提供商廣告範圍能力特徵。 |
| `torii_sorafs_range_fetch_throttle_events_total` |專櫃| `reason`（`quota`、`concurrency`、`byte_rate`）|按策略分組的限制範圍提取嘗試。 |
| `torii_sorafs_range_fetch_concurrency_current` |儀表| — |主動保護的流消耗共享並發預算。 |

PromQL 片段示例：

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

在啟用之前使用限制計數器確認配額強制執行
多源協調器默認值，並在並發接近時發出警報
為您的車隊提供最大預算。