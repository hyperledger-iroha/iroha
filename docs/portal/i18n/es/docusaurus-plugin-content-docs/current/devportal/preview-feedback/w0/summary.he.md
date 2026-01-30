---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w0/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26a65d4538dd113e3158eec3a91f3360231f2699be9895453e6505654957ae5f
source_last_modified: "2025-11-14T04:43:19.818958+00:00"
translation_last_reviewed: 2026-01-30
---

| Item | Detalles |
| --- | --- |
| Ola | W0 - Mantenedores core |
| Fecha del resumen | 2025-03-27 |
| Ventana de revision | 2025-03-25 -> 2025-04-08 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Tag de artefacto | `preview-2025-03-24` |

## Destacados

1. **Flujo de checksum** - Todos los revisores confirmaron que `scripts/preview_verify.sh`
   tuvo exito contra el par descriptor/archivo compartido. No se requirieron
   overrides manuales.
2. **Feedback de navegacion** - Se registraron dos problemas menores de orden del sidebar
   (`docs-preview/w0 #1-#2`). Ambos se asignaron a Docs/DevRel y no bloquean la
   ola.
3. **Paridad de runbooks de SoraFS** - sorafs-ops-01 pidio enlaces cruzados mas claros
   entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. Se abrio un
   issue de seguimiento; se atendera antes de W1.
4. **Revision de telemetria** - observability-01 confirmo que `docs.preview.integrity`,
   `TryItProxyErrors` y los logs del proxy Try-it se mantuvieron en verde; no se
   dispararon alertas.

## Acciones

| ID | Descripcion | Responsable | Estado |
| --- | --- | --- | --- |
| W0-A1 | Reordenar entradas del sidebar del devportal para destacar docs enfocados en reviewers (`preview-invite-*` agrupados). | Docs-core-01 | Completado - el sidebar ahora lista los docs de reviewers de forma contigua (`docs/portal/sidebars.js`). |
| W0-A2 | Agregar enlace cruzado explicito entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Completado - cada runbook ahora enlaza al otro para que los operadores vean ambas guias durante rollouts. |
| W0-A3 | Compartir snapshots de telemetria + paquete de queries con el tracker de gobernanza. | Observability-01 | Completado - paquete adjunto a `DOCS-SORA-Preview-W0`. |

## Resumen de cierre (2025-04-08)

- Los cinco revisores confirmaron la finalizacion, limpiaron builds locales y salieron de la
  ventana de preview; las revocaciones de acceso quedaron registradas en `DOCS-SORA-Preview-W0`.
- No hubo incidentes ni alertas durante la ola; los dashboards de telemetria se mantuvieron
  en verde todo el periodo.
- Las acciones de navegacion + enlaces cruzados (W0-A1/A2) estan implementadas y reflejadas en
  los docs de arriba; la evidencia de telemetria (W0-A3) esta adjunta al tracker.
- Paquete de evidencia archivado: screenshots de telemetria, acuses de invitacion y este
  resumen estan enlazados desde el issue del tracker.

## Siguientes pasos

- Implementar los action items de W0 antes de abrir W1.
- Obtener aprobacion legal y un slot de staging para el proxy, luego seguir los pasos de
  preflight de la ola de partners detallados en el [preview invite flow](../../preview-invite-flow.md).

_Este resumen esta enlazado desde el [preview invite tracker](../../preview-invite-tracker.md) para
mantener el roadmap DOCS-SORA trazable._
