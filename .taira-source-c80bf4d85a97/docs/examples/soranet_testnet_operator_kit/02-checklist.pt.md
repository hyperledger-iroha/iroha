---
lang: pt
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-11-21T14:24:59.232929+00:00"
translation_last_reviewed: 2026-01-01
---

- [ ] Revisar especificacao de hardware: 8+ cores, 16 GiB RAM, NVMe >= 500 MiB/s.
- [ ] Confirmar dois enderecos IPv4 + IPv6 e que o upstream permita QUIC/UDP 443.
- [ ] Provisionar HSM ou enclave seguro dedicado para as chaves de identidade do relay.
- [ ] Sincronizar o catalogo canonical de opt-out (`governance/compliance/soranet_opt_outs.json`).
- [ ] Mesclar o bloco de compliance na config do orchestrator (ver `03-config-example.toml`).
- [ ] Capturar atestados de compliance jurisdicional/global e preencher a lista `attestations`.
- [ ] Executar `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (ou seu snapshot live) e revisar o relatorio pass/fail.
- [ ] Gerar o CSR de admissao do relay e obter o envelope assinado pela governance.
- [ ] Importar o seed do descriptor guard e verificar contra a hash chain publicada.
- [ ] Executar `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] Fazer dry-run do export de telemetria: garantir que o scrape do Prometheus funcione localmente.
- [ ] Agendar a janela do drill de brownout e registrar contatos de escalacao.
- [ ] Assinar o bundle de evidencia do drill: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.
