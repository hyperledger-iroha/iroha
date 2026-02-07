---
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7334c5f2ccfa93c15a0827390e78b6026bb65e80ac9d624321da84f2287ce581
source_last_modified: "2026-01-05T09:28:11.911258+00:00"
translation_last_reviewed: 2026-02-07
id: constant-rate-profiles
title: SoraNet constant-rate profiles
sidebar_label: Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production relays plus the SNNet-17A2 null dogfood profile, with tick->bandwidth math, CLI helpers, and MTU guardrails.
translator: machine-google-reviewed
---

:::注意規範來源
:::

SNNet-17B 引入了固定速率傳輸通道，因此中繼在 1,024 B 單元中移動流量，無論
有效負載大小。操作員從三個預設中進行選擇：

- **核心** - 數據中心或專業託管的中繼，可以專用 >=30 Mbps 來覆蓋
  交通。
- **home** - 仍然需要匿名獲取的住宅或低上行鏈路運營商
  隱私關鍵電路。
- **null** - SNNet-17A2 狗糧預設。它保留了相同的 TLV/信封，但擴展了
  低帶寬分級的刻度和上限。

## 預設摘要

|簡介 |刻度線（毫秒）|細胞（B）|車道帽 |虛擬地板|每通道有效負載 (Mb/s) |有效負載上限 (Mb/s) |上行鏈路百分比上限 |建議上行鏈路 (Mb/s) |鄰居帽 |自動禁用觸發器 (%) |
|------|---------|----------|----------|--------------|--------------------------|------------------------|----------------------|----------------------------|--------------|----------------------------|
|核心| 5.0 | 1024 | 1024 12 | 12 4 | 1.64 | 1.64 19.50 | 19.50 65 | 65 30.0 | 30.0 8 | 85 | 85
|首頁 | 10.0 | 1024 | 1024 4 | 2 | 0.82 | 0.82 4.00 | 40 | 40 10.0 | 2 | 70 | 70
|空 | 20.0 | 20.0 1024 | 1024 2 | 1 | 0.41 | 0.41 0.75 | 0.75 15 | 15 5.0 | 1 | 55 | 55

- **車道上限** - 最大並發恆定速率鄰居。繼電器拒絕額外電路一次
  達到上限並遞增 `soranet_handshake_capacity_reject_total`。
- **虛擬樓層** - 即使在實際情況下也能保持虛擬交通的最小車道數
  需求較低。
- **有效負載上限** - 應用上限後專用於恆定速率通道的上行鏈路預算
  分數。即使有額外的帶寬可用，運營商也不應超出此預算。
- **自動禁用觸發器** - 持續飽和百分比（每個預設的平均值），導致
  運行時掉落到虛擬地板上。容量在恢復閾值後恢復
  （對於 `core` 為 75%，對於 `home` 為 60%，對於 `null` 為 45%）。

**重要提示：** `null` 預設僅用於暫存和功能測試；它不符合
生產電路所需的隱私保證。

## 勾選->帶寬表

每個有效負載單元攜帶 1,024 B，因此 KiB/sec 列等於每個有效負載單元發射的單元數
第二。使用幫助程序通過自定義刻度來擴展表格。

|刻度線（毫秒）|單元/秒 |有效負載 KiB/秒 |有效負載 Mb/s |
|----------|----------|-----------------|--------------|
| 5.0 | 200.00 | 200.00 | 1.64 | 1.64
| 7.5 | 7.5 133.33 | 133.33 | 1.09 | 1.09
| 10.0 | 100.00 | 100.00 | 0.82 | 0.82
| 15.0 | 15.0 66.67 | 66.67 66.67 | 66.67 0.55 | 0.55
| 20.0 | 20.0 50.00 | 50.00 | 0.41 | 0.41

公式：

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI 助手：

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` 為預設摘要和可選的刻度作弊生成 GitHub 樣式的表格
工作表，以便您可以將確定性輸出粘貼到門戶中。與 `--json-out` 配對進行存檔
治理證據的渲染數據。

## 配置和覆蓋

`tools/soranet-relay` 在配置文件和運行時覆蓋中公開預設：

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

配置密鑰接受 `core`、`home` 或 `null`（默認為 `core`）。 CLI 覆蓋對於
暫存演練或 SOC 請求可暫時減少佔空比，而無需重寫配置。

## MTU 護欄

- 有效負載單元使用 1,024 B 加上約 96 B 的 Norito+噪聲幀和最小的 QUIC/UDP 標頭，
  將每個數據報保持在 IPv6 1,280 B 最小 MTU 以下。
- 當隧道（WireGuard/IPsec）添加額外封裝時，您**必須**減少 `padding.cell_size`
  所以 `cell_size + framing <= 1,280 B`。中繼驗證器強制執行
  `padding.cell_size <= 1,136 B`（1,280 B - 48 B UDP/IPv6 開銷 - 96 B 成幀）。
- `core` 配置文件應固定 >=4 個鄰居，即使在閒置時也是如此，因此虛擬通道始終覆蓋
  PQ守衛。 `home` 配置文件可能會限制錢包/聚合器的恆定速率電路，但必須適用
  三個遙測窗口的飽和度超過 70% 時的背壓。

## 遙測和警報

繼電器根據預設導出以下指標：

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

出現以下情況時發出警報：

1. 虛擬比率低於預設下限（`core >= 4/8`、`home >= 2/2`、`null >= 1/1`）的時間超過
   兩個窗戶。
2. `soranet_constant_rate_ceiling_hits_total` 的增長速度超過每五分鐘一次點擊。
3. `soranet_constant_rate_degraded` 在計劃演練之外翻轉為 `1`。

在事件報告中記錄預設標籤和鄰居列表，以便審核員可以證明恆定速率
政策符合路線圖要求。