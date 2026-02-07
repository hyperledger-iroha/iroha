---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 掉電/降級響應手冊

1. **檢測**
   - 警報 `soranet_privacy_circuit_events_total{kind="downgrade"}` 觸發或
     治理中的 Brownout Webhook 觸發器。
   - 在 5 分鐘內通過 `kubectl logs soranet-relay` 或 systemd 日誌確認。

2. **穩定**
   - 防凍罩旋轉 (`relay guard-rotation disable --ttl 30m`)。
   - 為受影響的客戶端啟用直接覆蓋
     （`sorafs fetch --transport-policy direct-only --write-mode read-only`）。
   - 捕獲當前合規性配置哈希 (`sha256sum compliance.toml`)。

3. **診斷**
   - 收集最新的目錄快照和中繼指標包：
     `soranet-relay support-bundle --output /tmp/bundle.tgz`。
   - 注意 PoW 隊列深度、節流計數器和 GAR 類別峰值。
   - 確定事件是否由 PQ 不足、合規性覆蓋或繼電器故障引起。

4. **升級**
   - 通知治理橋 (`#soranet-incident`) 摘要和捆綁哈希。
   - 打開鏈接到警報的事件票證，包括時間戳和緩解步驟。

5. **恢復**
   - 解決根本原因後，重新啟用輪換
     (`relay guard-rotation enable`) 並恢復僅直接覆蓋。
   - 監控 KPI 30 分鐘；確保不會出現新的停電情況。

6. **事後分析**
   - 使用治理模板在 48 小時內提交事件報告。
   - 如果發現新的故障模式，請更新操作手冊。