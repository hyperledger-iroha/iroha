---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ad407370f375e45f0143f082b33a5ea61698825c8cd92dac402f656fb0f61a2
source_last_modified: "2026-01-22T16:26:46.524254+00:00"
translation_last_reviewed: 2026-02-07
id: dispute-revocation-runbook
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
translator: machine-google-reviewed
---

:::注意規範來源
:::

## 目的

本操作手冊指導治理操作員提交 SoraFS 容量爭議、協調撤銷並確保確定性地完成數據疏散。

## 1. 評估事件

- **觸發條件：** 檢測到 SLA 違規（正常運行時間/PoR 故障）、複製不足或計費不一致。
- **確認遙測：** 為提供商捕獲 `/v1/sorafs/capacity/state` 和 `/v1/sorafs/capacity/telemetry` 快照。
- **通知利益相關者：** 存儲團隊（提供商運營）、治理委員會（決策機構）、可觀察性（儀表板更新）。

## 2. 準備證據包

1. 收集原始工件（遙測 JSON、CLI 日誌、審核員註釋）。
2. 規範化為確定性存檔（例如 tarball）；記錄：
   - BLAKE3-256 摘要 (`evidence_digest`)
   - 媒體類型（`application/zip`、`application/jsonl` 等）
   - 託管 URI（對象存儲、SoraFS 引腳或 Torii 可訪問端點）
3. 將捆綁包存儲在具有一次寫入訪問權限的治理證據收集存儲桶中。

## 3. 提出爭議

1. 為 `sorafs_manifest_stub capacity dispute` 創建規範 JSON：

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. 運行 CLI：

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3. 審查 `dispute_summary.json`（確認類型、證據摘要、時間戳）。
4. 通過治理事務隊列向 Torii `/v1/sorafs/capacity/dispute` 提交請求 JSON。捕獲`dispute_id_hex`響應值；它是後續撤銷行動和審計報告的基礎。

## 4. 撤消和撤銷

1. **寬限期：**通知提供商即將撤銷；在策略允許的情況下允許撤出固定數據。
2. **生成`ProviderAdmissionRevocationV1`：**
   - 使用 `sorafs_manifest_stub provider-admission revoke` 並提供批准的理由。
   - 驗證簽名和撤銷摘要。
3. **發布撤銷：**
   - 向 Torii 提交撤銷請求。
   - 確保提供商廣告被屏蔽（預計 `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` 會攀升）。
4. **更新儀表板：** 將提供商標記為已撤銷，引用爭議 ID，並鏈接證據包。

## 5. 驗屍和後續行動

- 在治理事件跟踪器中記錄時間線、根本原因和補救措施。
- 確定賠償（削減權益、費用回扣、客戶退款）。
- 記錄學習內容；如果需要，更新 SLA 閾值或監控警報。

## 6. 參考資料

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md`（爭議部分）
- `docs/source/sorafs/provider_admission_policy.md`（撤銷工作流程）
- 可觀測性儀表板：`SoraFS / Capacity Providers`

## 清單

- [ ] 捕獲並散列的證據包。
- [ ] 爭議負載已在本地驗證。
- [ ] Torii 爭議交易已接受。
- [ ] 已執行撤銷（如果獲得批准）。
- [ ] 儀表板/操作手冊已更新。
- [ ] 向治理委員會提交屍檢報告。