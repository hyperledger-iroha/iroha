---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 掉电/降级响应手册

1. **检测**
   - 警报 `soranet_privacy_circuit_events_total{kind="downgrade"}` 触发或
     治理中的 Brownout Webhook 触发器。
   - 在 5 分钟内通过 `kubectl logs soranet-relay` 或 systemd 日志确认。

2. **稳定**
   - 防冻罩旋转 (`relay guard-rotation disable --ttl 30m`)。
   - 为受影响的客户端启用直接覆盖
     （`sorafs fetch --transport-policy direct-only --write-mode read-only`）。
   - 捕获当前合规性配置哈希 (`sha256sum compliance.toml`)。

3. **诊断**
   - 收集最新的目录快照和中继指标包：
     `soranet-relay support-bundle --output /tmp/bundle.tgz`。
   - 注意 PoW 队列深度、节流计数器和 GAR 类别峰值。
   - 确定事件是否由 PQ 不足、合规性覆盖或继电器故障引起。

4. **升级**
   - 通知治理桥 (`#soranet-incident`) 摘要和捆绑哈希。
   - 打开链接到警报的事件票证，包括时间戳和缓解步骤。

5. **恢复**
   - 解决根本原因后，重新启用轮换
     (`relay guard-rotation enable`) 并恢复仅直接覆盖。
   - 监控 KPI 30 分钟；确保不会出现新的停电情况。

6. **事后分析**
   - 使用治理模板在 48 小时内提交事件报告。
   - 如果发现新的故障模式，请更新操作手册。