---
lang: pt
direction: ltr
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

# Endurecimento do Gateway SoraGlobal (SNNet-15H)

O helper de endurecimento captura evidências de segurança e privacidade antes de promover builds do Gateway.

## Comando
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## Saídas
- `gateway_hardening_summary.json` — status por entrada (SBOM, relatório de vulnerabilidades, política de HSM, perfil de sandbox) mais o sinal de retenção. Entradas ausentes exibem `warn` ou `error`.
- `gateway_hardening_summary.md` — resumo legível para humanos para pacotes de governança.

## Notas de aceitação
- Relatórios de SBOM e vulnerabilidades devem existir; entradas ausentes rebaixam o status.
- Retenção acima de 30 dias marca `warn` para revisão; forneça padrões mais rígidos antes da GA.
- Use os artefatos de resumo como anexos para revisões GAR/SOC e runbooks de incidentes.
