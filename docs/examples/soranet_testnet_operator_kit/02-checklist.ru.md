---
lang: ru
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] Проверьте спецификацию оборудования: 8+ ядер, 16 GiB RAM, NVMe >= 500 MiB/s.
- [ ] Подтвердите два адреса IPv4 + IPv6 и что апстрим разрешает QUIC/UDP 443.
- [ ] Подготовьте HSM или выделенный защищенный enclave для ключей идентичности relay.
- [ ] Синхронизируйте канонический opt-out каталог (`governance/compliance/soranet_opt_outs.json`).
- [ ] Внесите блок compliance в конфиг orchestrator (см. `03-config-example.toml`).
- [ ] Соберите подтверждения compliance по юрисдикциям/глобально и заполните список `attestations`.
- [ ] Выполните `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (или ваш live snapshot) и проверьте отчет pass/fail.
- [ ] Сформируйте CSR допуска relay и получите конверт, подписанный governance.
- [ ] Импортируйте seed descriptor guard и проверьте по опубликованной hash chain.
- [ ] Выполните `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] Dry-run экспорта телеметрии: убедитесь, что Prometheus scrape проходит локально.
- [ ] Запланируйте окно brownout drill и зафиксируйте контакты для эскалации.
- [ ] Подпишите пакет доказательств drill: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
