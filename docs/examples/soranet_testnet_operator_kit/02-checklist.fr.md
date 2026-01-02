---
lang: fr
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] Revoir la spec hardware: 8+ cores, 16 GiB RAM, NVMe >= 500 MiB/s.
- [ ] Confirmer deux adresses IPv4 + IPv6 et que l'upstream autorise QUIC/UDP 443.
- [ ] Provisionner un HSM ou un enclave securisee dediee pour les cles d'identite du relay.
- [ ] Synchroniser le catalogue canonical d'opt-out (`governance/compliance/soranet_opt_outs.json`).
- [ ] Integrer le bloc compliance dans la config de l'orchestrator (voir `03-config-example.toml`).
- [ ] Capturer les attestations de compliance juridiction/global et remplir la liste `attestations`.
- [ ] Executer `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (ou votre snapshot live) et revoir le rapport pass/fail.
- [ ] Generer le CSR d'admission du relay et obtenir l'enveloppe signee par governance.
- [ ] Importer le seed du descriptor guard et verifier contre la chaine hash publiee.
- [ ] Executer `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] Faire un dry-run d'export de telemetrie: verifier que le scrape Prometheus reussit localement.
- [ ] Planifier la fenetre du drill brownout et noter les contacts d'escalade.
- [ ] Signer le bundle d'evidence du drill: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
