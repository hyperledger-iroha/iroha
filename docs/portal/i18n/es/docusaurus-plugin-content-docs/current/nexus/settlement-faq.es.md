---
lang: es
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preguntas frecuentes sobre nexus-settlement
título: Preguntas frecuentes sobre liquidación
descripción: Respuestas para operadores que cubren el enrutamiento de liquidación, conversión a XOR, telemetría y evidencia de auditoría.
---

Esta página refleja el FAQ interno de asentamiento (`docs/source/nexus_settlement_faq.md`) para que los lectores del portal revisen la misma guía sin hurgar en el mono-repo. Explica cómo el Settlement Router procesa pagos, que métricas monitorean y como los SDK deben integrar las cargas útiles de Norito.

## Destacados1. **Mapeo de carriles** - cada espacio de datos declara un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` o `xor_dual_fund`). Consulta el catálogo de carriles más reciente en `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversión determinística** - el enrutador convierte todos los asentamientos a XOR a través de las fuentes de liquidez aprobadas por gobernanza. Las carriles privados colchones prefinancieros XOR; Los cortes de pelo se aplican solo cuando los buffers se desvian de la política.
3. **Telemetria** - vigila `nexus_settlement_latency_seconds`, contadores de conversión y medidores de corte de pelo. Los tableros viven en `dashboards/grafana/nexus_settlement.json` y las alertas en `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidencia** - configuraciones de archivo, registros del enrutador, exportaciones de telemetría e informes de conciliación para auditorías.
5. **Responsabilidades de SDK** - cada SDK debe exponer ayudantes de liquidación, ID de carril y codificadores de cargas útiles Norito para mantener la paridad con el enrutador.

## Flujos de ejemplo| Tipo de carril | Pruebas de captura | Lo que demuestra |
|-----------|--------------------|----------------|
| Privada `xor_hosted_custody` | Registro del enrutador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Los buffers CBDC debitan XOR determinística y los cortes de pelo se mantienen dentro de la política. |
| Publica `xor_global` | Log del router + referencia DEX/TWAP + métricas de latencia/conversión | La ruta de liquidez compartida fija el precio de la transferencia al TWAP publicado con cero corte de pelo. |
| Híbrida `xor_dual_fund` | Log del router que muestra la división pública vs blindada + contadores de telemetria | La mezcla blindada/publica respeto los ratios de gobernanza y registro el corte de pelo aplicado a cada tramo. |

## ¿Necesitas más detalles?

- Preguntas frecuentes completas: `docs/source/nexus_settlement_faq.md`
- Especificación del router de liquidación: `docs/source/settlement_router.md`
- Manual de estrategia política CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook de operaciones: [Operaciones de Nexus](./nexus-operations)