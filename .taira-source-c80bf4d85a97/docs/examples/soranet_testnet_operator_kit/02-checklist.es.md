---
lang: es
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] Revisar especificacion de hardware: 8+ cores, 16 GiB RAM, NVMe >= 500 MiB/s.
- [ ] Confirmar dos direcciones IPv4 + IPv6 y que el upstream permita QUIC/UDP 443.
- [ ] Provisionar HSM o enclave seguro dedicado para las claves de identidad del relay.
- [ ] Sincronizar el catalogo canonico de opt-out (`governance/compliance/soranet_opt_outs.json`).
- [ ] Integrar el bloque de compliance en la config del orchestrator (ver `03-config-example.toml`).
- [ ] Capturar atestaciones de compliance jurisdiccional/global y completar la lista `attestations`.
- [ ] Ejecutar `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (o tu snapshot en vivo) y revisar el reporte pass/fail.
- [ ] Generar el CSR de admision del relay y obtener el sobre firmado por governance.
- [ ] Importar el seed del descriptor guard y verificar contra la hash chain publicada.
- [ ] Ejecutar `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] Hacer dry-run del export de telemetria: asegurar que el scrape de Prometheus funcione localmente.
- [ ] Programar la ventana del drill de brownout y registrar contactos de escalacion.
- [ ] Firmar el bundle de evidencia del drill: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
