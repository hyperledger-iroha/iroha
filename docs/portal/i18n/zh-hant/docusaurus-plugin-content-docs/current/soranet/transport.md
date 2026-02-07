---
id: transport
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet transport overview
sidebar_label: Transport Overview
description: Handshake, salt rotation, and capability guidance for the SoraNet anonymity overlay.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

SoraNet 是支持 SoraFS 範圍獲取、Norito RPC 流和未來 Nexus 數據通道的匿名覆蓋層。傳輸計劃（路線圖項目 **SNNet-1**、**SNNet-1a** 和 **SNNet-1b**）定義了確定性握手、後量子 (PQ) 能力協商和鹽輪換計劃，以便每個中繼、客戶端和網關觀察相同的安全態勢。

## 目標和網絡模型

- 在 QUIC v1 上構建三跳電路（入口 → 中間 → 出口），因此濫用者永遠不會直接到達 Torii。
- 在 QUIC/TLS 之上分層 Noise XX *混合*握手 (Curve25519 + Kyber768)，以將會話密鑰綁定到 TLS 轉錄本。
- 需要通告 PQ KEM/簽名支持、中繼角色和協議版本的功能 TLV；潤滑未知類型以保持未來擴展的可部署性。
- 每天輪換盲內容鹽，並在 30 天內固定保護繼電器，以便目錄流失無法使客戶端匿名。
- 將單元固定在 1024B，注入填充/虛擬單元，並導出確定性遙測數據，以便快速捕獲降級嘗試。

## 握手管道 (SNNet-1a)

1. **QUIC/TLS 信封** – 客戶端通過 QUIC v1 撥號中繼，並使用治理 CA 簽名的 Ed25519 證書完成 TLS1.3 握手。 TLS 導出器 (`tls-exporter("soranet handshake", 64)`) 播種噪聲層，因此轉錄本是不可分割的。
2. **噪聲 XX 混合** – 協議字符串 `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256`，帶有序言 = TLS 導出器。消息流程：

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH 輸出和兩種 Kyber 封裝都混合到最終的對稱密鑰中。如果未能協商 PQ 材料，握手會徹底中止——不允許僅使用經典的後備方案。

3. **謎題票和代幣** – 中繼可以在 `ClientHello` 之前要求 Argon2id 工作量證明票。票證是帶有長度前綴的幀，攜帶散列的 Argon2 解決方案並在策略範圍內過期：

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   當來自發行者的 ML-DSA-44 簽名根據活動策略和撤銷列表進行驗證時，前綴為 `SNTK` 的准入令牌會繞過難題。

4. **能力 TLV 交換** – 最終的噪聲有效負載傳輸如下所述的能力 TLV。如果任何強制功能（PQ KEM/簽名、角色或版本）丟失或與目錄條目不匹配，客戶端將中止連接。

5. **轉錄記錄** – 中繼記錄轉錄哈希、TLS 指紋和 TLV 內容，以提供降級檢測器和合規管道。

## 功能 TLV (SNNet-1c)

功能重用固定的 `typ/length/value` TLV 信封：

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

今天定義的類型：

- `snnet.pqkem` – Kyber 級別（當前推出的為 `kyber768`）。
- `snnet.pqsig` – PQ 簽名套件 (`ml-dsa-44`)。
- `snnet.role` – 繼電器角色（`entry`、`middle`、`exit`、`gateway`）。
- `snnet.version` – 協議版本標識符。
- `snnet.grease` – 保留範圍內的隨機填充條目，以確保容忍未來的 TLV。

客戶端維護所需 TLV 的允許列表，並在握手失敗時忽略或降級它們。中繼在其目錄微描述符中發布相同的集合，因此驗證是確定性的。

## 鹽輪換和 CID 致盲 (SNNet-1b)

- 治理髮布包含 `(epoch_id, salt, valid_after, valid_until)` 值的 `SaltRotationScheduleV1` 記錄。中繼和網關從目錄發布者獲取簽名的時間表。
- 客戶端在 `valid_after` 應用新的鹽，將之前的鹽保留 12 小時寬限期，並保留 7 紀元歷史記錄以容忍延遲更新。
- 規範盲標識符使用：

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  網關通過 `Sora-Req-Blinded-CID` 接受盲密鑰，並在 `Sora-Content-CID` 中回顯它。電路/請求致盲 (`CircuitBlindingKey::derive`) 在 `iroha_crypto::soranet::blinding` 中發貨。
- 如果繼電器錯過了一個紀元，它會停止新的電路，直到下載時間表並發出 `SaltRecoveryEventV1`，待命儀表板將其視為尋呼信號。

## 目錄數據和保護策略

- 微描述符攜帶中繼身份（Ed25519 + ML-DSA-65）、PQ 密鑰、能力 TLV、區域標籤、警衛資格和當前公佈的鹽紀元。
- 客戶端將保護設置保留 30 天，並將 `guard_set` 緩存與簽名的目錄快照一起保留。 CLI 和 SDK 包裝器顯示緩存指紋，因此可以將推出證據附加到更改評論中。

## 遙測和部署清單

- 生產前導出的指標：
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- 警報閾值與鹽輪換 SOP SLO 矩陣 (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) 一起存在，並且必須在網絡升級之前在 Alertmanager 中進行鏡像。
- 警報：5 分鐘內故障率 >5%、鹽滯後 >15 分鐘或生產中觀察到的功能不匹配。
- 推出步驟：
  1. 在啟用混合握手和 PQ 堆棧的情況下對分段進行中繼/客戶端互操作性測試。
  2. 演練鹽輪換 SOP (`docs/source/soranet_salt_plan.md`) 並將鑽孔工件附加到變更記錄中。
  3. 在目錄中啟用能力協商，然後推出到入口中繼、中間中繼、出口，最後是客戶端。
  4. 記錄每個階段的守衛緩存指紋、鹽計劃和遙測儀表板；將證據包附加到 `status.md`。

遵循此清單可以讓運營商、客戶和 SDK 團隊同步採用 SoraNet 傳輸，同時滿足 SNNet 路線圖中捕獲的確定性和審核要求。