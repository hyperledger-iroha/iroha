---
lang: he
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] בדקו מפרט חומרה: 8+ ליבות, 16 GiB RAM, NVMe >= 500 MiB/s.
- [ ] אשרו שתי כתובות IPv4 + IPv6 ושספק התשתית מאפשר QUIC/UDP 443.
- [ ] הקצו HSM או מובלעת מאובטחת ייעודית למפתחות זהות relay.
- [ ] סנכרנו את קטלוג ה-opt-out הקנוני (`governance/compliance/soranet_opt_outs.json`).
- [ ] מיזוג בלוק התאימות לתצורת orchestrator (ראו `03-config-example.toml`).
- [ ] אספו הצהרות תאימות אזוריות/גלובליות ומלאו את רשימת `attestations`.
- [ ] הריצו `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (או snapshot חי אצלכם) ובדקו דוח pass/fail.
- [ ] צרו CSR קבלה ל-relay וקבלו מעטפה חתומה על ידי הממשל.
- [ ] יבאו seed של descriptor guard ואמתו מול שרשרת ה-hash שפורסמה.
- [ ] הריצו `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] בצעו dry-run לייצוא טלמטריה: ודאו ש-scrape של Prometheus מצליח מקומית.
- [ ] תזמנו חלון drill ל-brownout ורשמו אנשי קשר להסלמה.
- [ ] חתמו על חבילת הראיות של drill: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
