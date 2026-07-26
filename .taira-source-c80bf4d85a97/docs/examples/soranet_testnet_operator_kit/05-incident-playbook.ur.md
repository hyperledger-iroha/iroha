---
lang: ur
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md کا اردو ترجمہ -->

# Brownout / Downgrade رسپانس پلے بک

1. **شناخت**
   - `soranet_privacy_circuit_events_total{kind="downgrade"}` الرٹ فائر ہو یا governance سے brownout webhook ٹرگر ہو.
   - 5 منٹ کے اندر `kubectl logs soranet-relay` یا systemd journal سے تصدیق کریں.

2. **استحکام**
   - guard rotation کو فریز کریں (`relay guard-rotation disable --ttl 30m`).
   - متاثرہ کلائنٹس کے لئے direct-only override فعال کریں
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - موجودہ compliance config hash حاصل کریں (`sha256sum compliance.toml`).

3. **تشخیص**
   - تازہ ترین directory snapshot اور relay metrics bundle جمع کریں:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - PoW queue depth، throttling counters، اور GAR category spikes نوٹ کریں.
   - تعین کریں کہ واقعہ PQ deficit، compliance override، یا relay failure سے ہوا.

4. **ایسکلیشن**
   - governance bridge (`#soranet-incident`) کو خلاصہ اور bundle hash کے ساتھ مطلع کریں.
   - الرٹ سے لنک والا incident ticket کھولیں، timestamps اور mitigation steps شامل کریں.

5. **بحالی**
   - جب root cause حل ہو جائے تو rotation دوبارہ فعال کریں
     (`relay guard-rotation enable`) اور direct-only overrides واپس لیں.
   - 30 منٹ تک KPIs مانیٹر کریں؛ یقینی بنائیں کہ نئے brownouts ظاہر نہ ہوں.

6. **Postmortem**
   - governance ٹیمپلیٹ کے مطابق 48 گھنٹوں میں incident رپورٹ جمع کریں.
   - اگر نیا failure mode ملے تو runbooks اپڈیٹ کریں.

</div>
