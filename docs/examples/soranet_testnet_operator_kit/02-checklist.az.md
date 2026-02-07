---
lang: az
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] Aparat xüsusiyyətlərini nəzərdən keçirin: 8+ nüvə, 16 GiB RAM, NVMe yaddaş ≥ 500 MiB/s.
- [ ] İki IPv4 + IPv6 ünvanını təsdiqləyin və yuxarı axın QUIC/UDP 443-ə icazə verir.
- [ ] Relay şəxsiyyət açarları üçün HSM və ya xüsusi təhlükəsiz anklav təmin edin.
- [ ] Kanonik imtina kataloqunu sinxronlaşdırın (`governance/compliance/soranet_opt_outs.json`).
- [ ] Uyğunluq blokunu orkestr konfiqurasiyasına birləşdirin (bax: `03-config-example.toml`).
- [ ] Yurisdiksiya/qlobal uyğunluq sertifikatlarını əldə edin və `attestations` siyahısını doldurun.
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (və ya canlı snapshot) proqramını işə salın və keçid/uğursuzluq hesabatını nəzərdən keçirin.
- [ ] Relay qəbulu CSR yaradın və idarəetmə tərəfindən imzalanmış zərf alın.
- [ ] Qoruyucu deskriptor toxumunu idxal edin və dərc edilmiş hash zəncirinə qarşı yoxlayın.
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`-i işə salın.
- [ ] Quru işlənmiş telemetriya ixracı: Prometheus qırıntısının yerli olaraq uğurlu olmasını təmin edin.
- [ ] Qazma pəncərəsini planlaşdırın və eskalasiya kontaktlarını qeyd edin.
- [ ] Qazma sübut paketini imzalayın: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.