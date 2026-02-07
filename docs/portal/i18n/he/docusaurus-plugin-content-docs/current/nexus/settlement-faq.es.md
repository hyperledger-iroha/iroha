---
lang: he
direction: rtl
source: docs/portal/docs/nexus/settlement-faq.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-settlement-faq
כותרת: שאלות נפוצות על הסדר
תיאור: תשובות עבור מפעילים que cubren el enrutamiento de settlement, המרת XOR, telemetria y evidencia de auditoria.
---

Esta page refleja el FAQ interno de settlement (`docs/source/nexus_settlement_faq.md`) para que los lectores del portal revisen la misma guia sin hurgar en el mono-repo. הסבר כמו נתב התיישבות מעבד עמודים, עבור מוניטור מדדים כמו SDK שילוב של עומסי מטענים של Norito.

## Destacados

1. **Mapeo de lanes** - cada dataspace declara un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` או `xor_dual_fund`). Consulta el catalogo de lanes mas reciente en `docs/source/project_tracker/nexus_config_deltas/`.
2. **Cversion deterministica** - el router convierte todos los settlements a XOR a traves de las fuentes de liquidez aprobadas por gobernanza. Las lanes privadas prefinancian buffers XOR; los haircuts aplican solo cuando los buffers se desvian de la politica.
3. **Telemetria** - vigila `nexus_settlement_latency_seconds`, contadores de conversion y medidores de haircut. Los dashboards viven en `dashboards/grafana/nexus_settlement.json` y las alertas en `dashboards/alerts/nexus_audit_rules.yml`.
4. **Evidencia** - תצורות ארכיון, יומני נתב, ייצוא של טלמטריה ומידע על פשרה עבור אודיטוריאס.
5. **Responsabilidades de SDK** - cada SDK debe exponer helpers de settlement, IDs de lane y codificadores de payloads Norito para mantener paridad con el router.

## פליליות

| טיפו דה ליין | Evidencia a capturar | Lo que demuestra |
|-----------|------------------------|----------------|
| Privada `xor_hosted_custody` | Log del נתב + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Los buffers CBDC debitan XOR deterministica y los haircuts se mantienen dentro de la politica. |
| Publica `xor_global` | יומן נתב + הפניה DEX/TWAP + מדדי עכבות/המרה | La ruta de liquidez compartida fijo el precio de la transferencia al TWAP publicado con cero hair hair. |
| Hibrida `xor_dual_fund` | Log del Router que muestra la division publica vs shielded + contadores de telemetria | La mezcla shielded/publica respeto los ratios de gobernanza y registro el haircut aplicado a cada tramo. |

## Necesitas mas detalle?

- שאלות נפוצות מלאות: `docs/source/nexus_settlement_faq.md`
- נתב יישוב ספציפי: `docs/source/settlement_router.md`
- Playbook de politica CBDC: `docs/source/cbdc_lane_playbook.md`
- ספר הפעלה: [Operaciones de Nexus](./nexus-operations)