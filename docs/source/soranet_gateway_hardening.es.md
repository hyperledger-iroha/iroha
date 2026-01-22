---
lang: es
direction: ltr
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

# Endurecimiento del Gateway SoraGlobal (SNNet-15H)

El helper de endurecimiento captura evidencias de seguridad y privacidad antes de promocionar builds del Gateway.

## Comando
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## Salidas
- `gateway_hardening_summary.json` — estado por entrada (SBOM, reporte de vulnerabilidades, política de HSM, perfil de sandbox) más la señal de retención. Entradas ausentes muestran `warn` o `error`.
- `gateway_hardening_summary.md` — resumen legible para humanos para paquetes de gobernanza.

## Notas de aceptación
- Los informes de SBOM y vulnerabilidades deben existir; entradas ausentes degradan el estado.
- Retención mayor a 30 días marca `warn` para revisión; use valores por defecto más estrictos antes de GA.
- Use los artefactos de resumen como adjuntos para revisiones GAR/SOC y runbooks de incidentes.
