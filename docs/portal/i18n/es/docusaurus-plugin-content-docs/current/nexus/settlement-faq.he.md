---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 85dd7cf140dc484b632b250f53b7dae427fd74a628c55c39edb03bff4c62f185
source_last_modified: "2025-11-14T04:43:20.567029+00:00"
translation_last_reviewed: 2026-01-30
---

Esta pagina refleja el FAQ interno de settlement (`docs/source/nexus_settlement_faq.md`) para que los lectores del portal revisen la misma guia sin hurgar en el mono-repo. Explica como el Settlement Router procesa pagos, que metricas monitorear y como los SDK deben integrar los payloads de Norito.

## Destacados

1. **Mapeo de lanes** - cada dataspace declara un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` o `xor_dual_fund`). Consulta el catalogo de lanes mas reciente en `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversion deterministica** - el router convierte todos los settlements a XOR a traves de las fuentes de liquidez aprobadas por gobernanza. Las lanes privadas prefinancian buffers XOR; los haircuts aplican solo cuando los buffers se desvian de la politica.
3. **Telemetria** - vigila `nexus_settlement_latency_seconds`, contadores de conversion y medidores de haircut. Los dashboards viven en `dashboards/grafana/nexus_settlement.json` y las alertas en `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidencia** - archiva configs, logs del router, exportaciones de telemetria e informes de conciliacion para auditorias.
5. **Responsabilidades de SDK** - cada SDK debe exponer helpers de settlement, IDs de lane y codificadores de payloads Norito para mantener paridad con el router.

## Flujos de ejemplo

| Tipo de lane | Evidencia a capturar | Lo que demuestra |
|-----------|--------------------|----------------|
| Privada `xor_hosted_custody` | Log del router + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Los buffers CBDC debitan XOR deterministica y los haircuts se mantienen dentro de la politica. |
| Publica `xor_global` | Log del router + referencia DEX/TWAP + metricas de latencia/conversion | La ruta de liquidez compartida fijo el precio de la transferencia al TWAP publicado con cero haircut. |
| Hibrida `xor_dual_fund` | Log del router que muestra la division publica vs shielded + contadores de telemetria | La mezcla shielded/publica respeto los ratios de gobernanza y registro el haircut aplicado a cada tramo. |

## Necesitas mas detalle?

- FAQ completo: `docs/source/nexus_settlement_faq.md`
- Especificacion del settlement router: `docs/source/settlement_router.md`
- Playbook de politica CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operaciones: [Operaciones de Nexus](./nexus-operations)
