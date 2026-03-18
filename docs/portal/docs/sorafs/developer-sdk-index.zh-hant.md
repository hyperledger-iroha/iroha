---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 28cc43e407412d66481f25146c19d35f0e102523d22f954be3c106231d95e891
source_last_modified: "2026-01-05T09:28:11.868629+00:00"
translation_last_reviewed: 2026-02-07
id: developer-sdk-index
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
---

:::注意規範來源
:::

使用此中心跟踪 SoraFS 工具鏈附帶的每種語言幫助程序。
對於 Rust 特定的代碼片段，請跳轉到 [Rust SDK 代碼片段](./developer-sdk-rust.md)。

## 語言助手

- **Python** — `sorafs_multi_fetch_local`（本地協調器冒煙測試）和
  `sorafs_gateway_fetch`（網關E2E練習）現在接受可選
  `telemetry_region` 加上 `transport_policy` 覆蓋
  （`"soranet-first"`、`"soranet-strict"` 或 `"direct-only"`），鏡像 CLI
  推出旋鈕。當本地 QUIC 代理啟動時，
  `sorafs_gateway_fetch` 返回瀏覽器清單
  `local_proxy_manifest` 因此測試可以將信任包交給瀏覽器適配器。
- **JavaScript** — `sorafsMultiFetchLocal` 鏡像 Python 幫助器，返回
  有效負載字節和接收摘要，而 `sorafsGatewayFetch` 練習
  Torii 網關、線程本地代理清單，並公開相同的內容
  遙測/傳輸覆蓋 CLI。
- **Rust** — 服務可以直接通過嵌入調度程序
  `sorafs_car::multi_fetch`；請參閱 [Rust SDK 片段](./developer-sdk-rust.md)
  證明流助手和協調器集成的參考。
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` 重用 Torii HTTP
  執行人並獲得榮譽 `GatewayFetchOptions`。將其與
  `ClientConfig.Builder#setSorafsGatewayUri` 和 PQ 上傳提示
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) 上傳時必須堅持
  僅 PQ 路徑。

## 記分板和政策旋鈕

Python (`sorafs_multi_fetch_local`) 和 JavaScript
(`sorafsMultiFetchLocal`) 幫助程序公開遙測感知調度程序記分板
CLI 使用：

- 生產二進製文件默認啟用記分板；設置 `use_scoreboard=True`
  （或提供 `telemetry` 條目）在重放賽程時以便幫助程序派生
  根據廣告元數據和最近的遙測快照對提供商進行加權排序。
- 設置 `return_scoreboard=True` 以接收計算出的權重以及塊
  收據，以便 CI 日誌可以捕獲診斷信息。
- 使用 `deny_providers` 或 `boost_providers` 陣列拒絕對等點或添加
  當調度程序選擇提供者時，`priority_delta`。
- 保持默認的 `"soranet-first"` 姿勢，除非進行降級；供應
  `"direct-only"` 僅當合規區域必須避免中繼或當
  演練 SNNet-5a 後備，並保留 `"soranet-strict"` 僅用於 PQ
  經政府批准的試點。
- 網關助手還公開 `scoreboardOutPath` 和 `scoreboardNowUnixSecs`。
  設置 `scoreboardOutPath` 以保留計算的記分板（鏡像 CLI
  `--scoreboard-out` 標誌），因此 `cargo xtask sorafs-adoption-check` 可以驗證
  SDK工件，當燈具需要穩定時使用`scoreboardNowUnixSecs`
  `assume_now` 可重現元數據的值。在 JavaScript 幫助器中，您
  可另外設置`scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`；
  當省略標籤時，它派生出 `region:<telemetryRegion>` （回退
  至 `sdk:js`）。 Python 幫助程序自動發出 `telemetry_source="sdk:python"`
  每當它保留記分板並禁用隱式元數據時。

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```