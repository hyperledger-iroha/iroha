---
lang: uz
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] Uskuna xususiyatlarini ko‘rib chiqing: 8+ yadro, 16 GiB RAM, NVMe xotirasi ≥ 500 MiB/s.
- [ ] Ikkita IPv4 + IPv6 manzillarini tasdiqlang va yuqori oqim QUIC/UDP 443 ga ruxsat beradi.
- [ ] Relay identifikatori kalitlari uchun HSM yoki maxsus xavfsiz anklavni ta'minlash.
- [ ] Sinxronlash kanonik rad etish katalogi (`governance/compliance/soranet_opt_outs.json`).
- [ ] Muvofiqlik blokini orkestr konfiguratsiyasiga birlashtiring (qarang: `03-config-example.toml`).
- [ ] Yurisdiktsiya/global muvofiqlik sertifikatlarini oling va `attestations` ro'yxatini to'ldiring.
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (yoki jonli suratingiz) ni ishga tushiring va o'tish/qobiliyatsiz hisobotni ko'rib chiqing.
- [ ] Relay qabul qilish CSR ni yarating va boshqaruv imzolangan konvertni oling.
- [ ] Guard deskriptor urug'ini import qiling va chop etilgan xesh zanjiri bilan tekshiring.
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` ishga tushiring.
- [ ] Quruq ishlaydigan telemetriya eksporti: Prometheus qirqish mahalliy darajada muvaffaqiyatli bo'lishini ta'minlang.
- [ ] Jigarrang burg'ulash oynasini rejalashtiring va eskalatsiya kontaktlarini yozing.
- [ ] Matkap dalillar to'plamini imzolang: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.