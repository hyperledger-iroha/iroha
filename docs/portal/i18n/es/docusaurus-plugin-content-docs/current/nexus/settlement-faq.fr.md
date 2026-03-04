---
lang: es
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preguntas frecuentes sobre nexus-settlement
título: Acuerdo de preguntas frecuentes
Descripción: Respuestas para los operadores que cubren la liquidación de rutas, la conversión XOR, la telemetría y las precauciones de auditoría.
---

Esta página contiene las preguntas frecuentes internas de liquidación (`docs/source/nexus_settlement_faq.md`) para que los lectores del portal puedan consultar las memes indicaciones sin seguir el mono-repo. Elle comentario explícito sobre el Settlement Router analiza los pagos, las mediciones que vigilan y comenta el SDK que integra las cargas útiles Norito.

## Puntos claves1. **Mapa de carriles** - cada espacio de datos declara un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` o `xor_dual_fund`). Consulte el último catálogo de carriles en `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversión determinista**: el enrutador convierte todos los acuerdos en XOR a través de las fuentes de liquidez aprobadas por el gobierno. Les lanes privées prefinancent des buffers XOR; les haircuts ne s'appliquent que lorsque les buffers derivent hors de la politique.
3. **Telemetría** - Vigile `nexus_settlement_latency_seconds`, los contadores de conversión y los indicadores de corte de pelo. Los paneles de control se encuentran en `dashboards/grafana/nexus_settlement.json` y las alertas en `dashboards/alerts/nexus_audit_rules.yml`.
4. **Preuves**: archiva las configuraciones, los registros del enrutador, las exportaciones de telemetría y los informes de reconciliación para las auditorías.
5. **Responsabilidades del SDK**: cada SDK expone los asistentes de liquidación, los ID de carril y los codificadores de cargas útiles Norito para restablecer la alineación con el enrutador.

## Flujo de ejemplo| Tipo de carril | Preuves un coleccionista | Ce que cela prouve |
|-----------|--------------------|----------------|
| Privée `xor_hosted_custody` | Registro del enrutador + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Los buffers CBDC deben a un XOR determinista y los recortes reposan en la política. |
| Público `xor_global` | Registro del enrutador + referencia DEX/TWAP + medidas de latencia/conversión | El camino de liquidez parte a precio fijo de transferencia al TWAP público con corte cero. |
| Híbrido `xor_dual_fund` | Registro del enrutador monta la partición pública vs blindada + ordenadores de telemetría | Le mix blinded/public a respecte les ratios de gouvernance et registre le haircut applique a chaque jambe. |

## ¿Besoin de plus de detalles?

- Preguntas frecuentes completas: `docs/source/nexus_settlement_faq.md`
- Especificaciones del enrutador de asentamiento: `docs/source/settlement_router.md`
- Guía política de CBDC: `docs/source/cbdc_lane_playbook.md`
- Operaciones de Runbook: [Operaciones Nexus](./nexus-operations)