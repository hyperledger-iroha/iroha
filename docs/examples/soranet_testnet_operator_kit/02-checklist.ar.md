---
lang: ar
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] راجع مواصفات العتاد: 8+ نوى، 16 GiB RAM، NVMe >= 500 MiB/s.
- [ ] تاكد من عنوانين IPv4 + IPv6 وان المزود يسمح بـ QUIC/UDP 443.
- [ ] جهز HSM او حاوية امنة مخصصة لمفاتيح هوية relay.
- [ ] زامن كتالوج opt-out الرسمي (`governance/compliance/soranet_opt_outs.json`).
- [ ] ادمج كتلة الامتثال في اعدادات orchestrator (انظر `03-config-example.toml`).
- [ ] وثق اقرارات الامتثال القضائية/العالمية واملأ قائمة `attestations`.
- [ ] شغل `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (او لقطة حية لديك) وراجع تقرير pass/fail.
- [ ] انشئ CSR قبول relay واحصل على ظرف موقع من الحوكمة.
- [ ] استورد seed لمعرف guard وتحقق مقابل سلسلة الهاش المنشورة.
- [ ] شغل `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] نفذ dry-run لتصدير التليمترى: تاكد من نجاح Prometheus scrape محليا.
- [ ] حدد نافذة drill للbrownout وسجل جهات التصعيد.
- [ ] وقع حزمة ادلة drill: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
