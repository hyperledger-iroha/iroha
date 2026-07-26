---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w0/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w0-summary
כותרת: Resumen de feedback de mitad de W0
sidebar_label: משוב W0 (mitad)
תיאור: Puntos de control, hallazgos y acciones de mitad de ola para la ola de preview de mantenedores core.
---

| פריט | פרטים |
| --- | --- |
| אולה | W0 - ליבת Mantenedores |
| Fecha del resume | 27-03-2025 |
| Ventana de revision | 2025-03-25 -> 2025-04-08 |
| משתתפים | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Tag de artefacto | `preview-2025-03-24` |

## Destacados

1. **Flujo de checksum** - Todos los revisores confirmaron que `scripts/preview_verify.sh`
   tuvo exito contra el par descriptor/archivo compartido. אין דרישה לכך
   עוקף מדריכים.
2. **משוב de navegacion** - Se registraron dos problemas menores de orden del sidebar
   (`docs-preview/w0 #1-#2`). Ambos se asignaron a Docs/DevRel y no bloquean la
   אולה.
3. **Paridad de runbooks de SoraFS** - sorafs-ops-01 pidio enlaces cruzados mas claros
   entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. Se abrio un
   issue de seguimiento; se atendera antes de W1.
4. **Revision de telemetria** - observability-01 confirmo que `docs.preview.integrity`,
   `TryItProxyErrors` y los logs del proxy Try-it se mantuvieron en verde; לא סה
   התראות דיספאררון.

## פעולות

| תעודה מזהה | תיאור | אחראי | Estado |
| --- | --- | --- | --- |
| W0-A1 | Reordenar entradas del sidebar del devportal para destacar docs enfocados en reviewers (`preview-invite-*` agrupados). | Docs-core-01 | Completado - el sidebar ahora list los docs de reviewers de forma contigua (`docs/portal/sidebars.js`). |
| W0-A2 | Agregar enlace cruzado explicito entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Completado - cada runbook ahora enlaza al otro para que los operadores vean ambas guias durante rollouts. |
| W0-A3 | השווה צילומי מצב של טלמטריה + חבילת שאילתות עם מעקב אחר גוברננזה. | צפיות-01 | Completado - paquete adjunto a `DOCS-SORA-Preview-W0`. |

## קורות חיים (2025-04-08)

- Los cinco revisores confirmaron la finalizacion, limpiaron בונה מקומות y salieron de la
  ventana de preview; las revocaciones de acceso quedaron registradas en `DOCS-SORA-Preview-W0`.
- No hubo incidentes ni alertas durante la ola; los dashboards de telemetria se mantuvieron
  en verde todo el periodo.
- Las acciones de navegacion + enlaces cruzados (W0-A1/A2) estan implementadas y reflejadas en
  los docs de arriba; la evidencia de telemetria (W0-A3) esta adjunta al tracker.
- Paquete de evidencia archivado: צילומי מסך של טלמטריה, אקוסאות הזמנה ואודות
  קורות חיים estan enlazados desde el issue del tracker.

## Siguiente pasos

- פריטי פעולה מיושמים של W0 antes de abrir W1.
- Obtener aprobacion legal y un slot de staging para el proxy, luego seguir los pasos de
  preflight de la ola de partners detallados en el [תצוגה מקדימה של זרימת הזמנה](../../preview-invite-flow.md).

_Este resumen esta enlazado desde el [עקוב אחר הזמנה מקדימה](../../preview-invite-tracker.md) para
mantener el מפת הדרכים DOCS-SORA ניתן לשינוי._