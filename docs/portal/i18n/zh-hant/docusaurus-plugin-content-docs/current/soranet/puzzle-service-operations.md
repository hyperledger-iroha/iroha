---
id: puzzle-service-operations
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Puzzle Service Operations Guide
sidebar_label: Puzzle Service Ops
description: Operating the `soranet-puzzle-service` daemon for Argon2/ML-DSA admission tickets.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

# Puzzle 服務操作指南

`soranet-puzzle-service` 守護程序 (`tools/soranet-puzzle-service/`) 問題
Argon2 支持的入場券反映了中繼的 `pow.puzzle.*` 政策
配置後，代表邊緣中繼代理 ML-DSA 准入令牌。
它公開了五個 HTTP 端點：

- `GET /healthz` – 活性探針。
- `GET /v2/puzzle/config` – 返回拉取的有效 PoW/謎題參數
  來自繼電器 JSON（`handshake.descriptor_commit_hex`、`pow.*`）。
- `POST /v2/puzzle/mint` – 鑄造一張 Argon2 票證；可選的 JSON 正文
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  請求更短的 TTL（限製到策略窗口），將票證綁定到
  成績單哈希，並返回中繼簽名票+簽名指紋
  配置簽名密鑰時。
- `GET /v2/token/config` – 當 `pow.token.enabled = true` 時，返回活動的
  准入令牌策略（發行者指紋、TTL/時鐘偏差邊界、中繼 ID、
  和合併的撤銷集）。
- `POST /v2/token/mint` – 鑄造一個與所提供的綁定的 ML-DSA 准入令牌
  恢復哈希；請求正文接受 `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`。

該服務生成的票證在
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`
集成測試，還在容量 DoS 期間執行中繼節流
場景。 【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## 配置代幣發行

設置 `pow.token.*` 下的中繼 JSON 字段（請參閱
以 `tools/soranet-relay/deploy/config/relay.entry.json` 為例）啟用
ML-DSA 代幣。至少提供發行者公鑰和可選的
撤銷清單：

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

拼圖服務重用這些值並自動重新加載 Norito
運行時的 JSON 撤銷文件。使用 `soranet-admission-token` CLI
(`cargo run -p soranet-relay --bin soranet_admission_token`) 鑄造和檢驗
脫機令牌，將 `token_id_hex` 條目附加到吊銷文件中，並進行審核
將更新推送到生產之前的現有憑據。

通過 CLI 標誌將發行者密鑰傳遞給拼圖服務：

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

當機密由帶外管理時，`--token-secret-hex` 也可用
工具管道。吊銷文件監視程序使 `/v2/token/config` 保持最新狀態；
使用 `soranet-admission-token revoke` 命令協調更新以避免滯後
撤銷狀態。

在中繼 JSON 中設置 `pow.signed_ticket_public_key_hex` 以通告 ML-DSA-44 公共
用於驗證簽名 PoW 票據的密鑰； `/v2/puzzle/config` 與該鍵及其 BLAKE3 相呼應
指紋 (`signed_ticket_public_key_fingerprint_hex`)，以便客戶端可以固定驗證器。
簽名票證根據中繼 ID 和轉錄本綁定進行驗證，並共享相同的內容
撤銷存儲；當簽名票證驗證者處於有效狀態時，原始 74 字節 PoW 票證仍然有效
配置。通過 `--signed-ticket-secret-hex` 傳遞簽名者機密或
`--signed-ticket-secret-path` 啟動拼圖服務時；啟動拒絕不匹配
密鑰對（如果密鑰未根據 `pow.signed_ticket_public_key_hex` 進行驗證）。
`POST /v2/puzzle/mint` 接受 `"signed": true`（和可選的 `"transcript_hash_hex"`）
返回 Norito 編碼的簽名票證以及原始票證字節；回應包括
`signed_ticket_b64` 和 `signed_ticket_fingerprint_hex` 幫助跟踪重放指紋。
如果未配置簽名者密鑰，則 `signed = true` 的請求將被拒絕。

## 密鑰輪換手冊

1. **收集新的描述符提交。 ** 治理髮布中繼
   目錄包中的描述符提交。將十六進製字符串複製到
   `handshake.descriptor_commit_hex` 繼電器內部 JSON 配置共享
   與拼圖服務。
2. **查看拼圖策略範圍。 ** 確認更新
   `pow.puzzle.{memory_kib,time_cost,lanes}` 值與版本一致
   計劃。運營商應保持 Argon2 配置的確定性
   繼電器（最小 4MiB 內存，1≤通道≤16）。
3. **分階段重啟。 ** 治理後重新加載 systemd 單元或容器
   宣布輪換切換。該服務不支持熱重載；一個
   需要重新啟動才能獲取新的描述符提交。
4. **驗證。 ** 通過 `POST /v2/puzzle/mint` 出票並確認
   返回的 `difficulty` 和 `expires_at` 與新策略匹配。浸泡報告
   (`docs/source/soranet/reports/pow_resilience.md`) 捕獲預期延遲
   界限供參考。啟用令牌後，獲取 `/v2/token/config` 到
   確保公佈的發行者指紋和撤銷計數匹配
   預期值。

## 緊急禁用程序

1. 在共享繼電器配置中設置 `pow.puzzle.enabled = false`。保留
   `pow.required = true` 如果 hashcash 後備票證必須保持強制性。
2. 可選擇強制 `pow.emergency` 條目拒絕過時的描述符，同時
   Argon2 門已離線。
3. 重新啟動中繼和拼圖服務以應用更改。
4. 監控`soranet_handshake_pow_difficulty`，確保難度降至
   預期的 hashcash 值，並驗證 `/v2/puzzle/config` 報告
   `puzzle = null`。

## 監控和警報

- **延遲 SLO：** 跟踪 `soranet_handshake_latency_seconds` 並保留 P95
  低於 300 毫秒。浸泡測試偏移為防護裝置提供校準數據
  節流閥。 【docs/source/soranet/reports/pow_resilience.md:1】
- **配額壓力：** 使用 `soranet_guard_capacity_report.py` 和中繼指標
  調整 `pow.quotas` 冷卻時間 (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **拼圖對齊：** `soranet_handshake_pow_difficulty` 應匹配
  `/v2/puzzle/config` 返回的難度。分歧表明中繼失效
  配置或重啟失敗。
- **令牌準備情況：** 如果 `/v2/token/config` 降至 `enabled = false`，則發出警報
  意外地或者如果 `revocation_source` 報告過時的時間戳。運營商
  每當令牌被使用時，應通過 CLI 輪換 Norito 吊銷文件
  已退休以保持此端點的準確性。
- **服務運行狀況：** 以通常的活躍節奏和警報探測 `/healthz`
  如果 `/v2/puzzle/mint` 返回 HTTP 500 響應（表示 Argon2 參數
  不匹配或 RNG 失敗）。通過 HTTP 4xx/5xx 出現令牌鑄造錯誤
  `/v2/token/mint` 上的回复；將重複失敗視為尋呼條件。

## 合規性和審計日誌記錄

繼電器發出結構化 `handshake` 事件，其中包括節流原因和
冷卻時間。確保中描述的合規管道
`docs/source/soranet/relay_audit_pipeline.md` 攝取這些日誌所以令人困惑
政策變化仍可審計。當拼圖門啟用時，存檔
鑄造的票據樣本和 Norito 配置快照隨推出
未來審計的票據。在維護窗口之前鑄造的入場代幣
應使用其 `token_id_hex` 值進行跟踪並插入到
一旦過期或被撤銷，撤銷文件。