---
lang: ur
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] ہارڈویئر اسپیک کا جائزہ: 8+ cores, 16 GiB RAM, NVMe >= 500 MiB/s.
- [ ] دو IPv4 + IPv6 ایڈریسز کی تصدیق کریں اور یہ یقینی بنائیں کہ upstream QUIC/UDP 443 کی اجازت دیتا ہے.
- [ ] relay شناختی keys کے لئے HSM یا dedicated secure enclave فراہم کریں.
- [ ] canonical opt-out catalog sync کریں (`governance/compliance/soranet_opt_outs.json`).
- [ ] compliance block کو orchestrator config میں merge کریں (دیکھیں `03-config-example.toml`).
- [ ] jurisdiction/global compliance attestations جمع کریں اور `attestations` لسٹ پاپولیٹ کریں.
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (یا اپنی live snapshot) چلائیں اور pass/fail رپورٹ دیکھیں.
- [ ] relay admission CSR بنائیں اور governance-signed envelope حاصل کریں.
- [ ] guard descriptor seed امپورٹ کریں اور published hash chain کے خلاف ویریفائی کریں.
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` چلائیں.
- [ ] telemetry export کا dry-run کریں: یقینی بنائیں کہ Prometheus scrape لوکلی کامیاب ہو.
- [ ] brownout drill window شیڈول کریں اور escalation contacts ریکارڈ کریں.
- [ ] drill evidence bundle پر دستخط کریں: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
