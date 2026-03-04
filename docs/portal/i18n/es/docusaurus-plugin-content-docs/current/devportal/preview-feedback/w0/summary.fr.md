---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w0-resumen
título: Digest des retours mi-parcours W0
sidebar_label: Retours W0 (mi-parcours)
Descripción: Puntos de control, constantes y acciones de mi-parcours para la vaga vista previa de los mantenedores principales.
---

| Elemento | Detalles |
| --- | --- |
| Vago | W0 - Núcleo de mantenedores |
| Fecha del resumen | 2025-03-27 |
| Ventana de revisión | 2025-03-25 -> 2025-04-08 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidad-01 |
| Etiqueta de artefacto | `preview-2025-03-24` |

## Puntos navegantes

1. **Flujo de trabajo de suma de verificación** - Todos los revisores no confirmaron que `scripts/preview_verify.sh`
   a reussi contre le Couple descriptor/archivo partage. Aucun anula a manuel requis.
2. **Retornos de navegación** - Dos problemas relacionados con el orden de la barra lateral en las señales
   (`docs-preview/w0 #1-#2`). Las dos rutas son hacia Docs/DevRel y no están bloqueadas por la
   vago.
3. **Parite des runbooks SoraFS** - sorafs-ops-01 a demande des gravámenes croises plus clairs
   entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. Issue de suivi overte;
   un traidor avant W1.
4. **Revue de telemetrie** - observability-01 a confirme que `docs.preview.integrity`,
   `TryItProxyErrors` y los registros del proxy Try-it están en blanco; aucune alerta n'a
   ete declenchee.

## Acciones| identificación | Descripción | Responsable | Estatuto |
| --- | --- | --- | --- |
| W0-A1 | Reordene las entradas de la barra lateral del portal de desarrollo para agregarlos antes de los documentos para los revisores (`preview-invite-*` reagrupados). | Documentos-core-01 | Termine: la lista de la barra lateral mantiene los documentos revisores de facon contigue (`docs/portal/sidebars.js`). |
| W0-A2 | Agregue un gravamen croise explícito entre `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Termine: cada punto de runbook desormais vers el otro para que los operadores voient les dos guías durante los lanzamientos. |
| W0-A3 | Comparta instantáneas de telemetría + paquete de solicitudes con el rastreador de gobierno. | Observabilidad-01 | Termine - paquete adjunto a `DOCS-SORA-Preview-W0`. |

## Reanudar la salida (2025-04-08)

- Los cinco revisores confirmaron la fin, purgaron las construcciones locaux y abandonaron la ventana de
  vista previa; Las revocaciones de acceso están registradas en `DOCS-SORA-Preview-W0`.
- Aucun incidente ni alerta colgante la vague; Los paneles de telemetría están en reposo verde.
  colgante toda la época.
- Las acciones de navegación + gravámenes croises (W0-A1/A2) son implementadas y reflejadas en
  les docs ci-dessus; La telemetría anterior (W0-A3) está adjunta al rastreador.
- Paquete de archivo anterior: capturas de pantalla de telemetría, acusaciones de invitación y este resumen
  Sont mentiras después de la emisión del rastreador.

## Etapas de prochaines- Implementador de las acciones W0 antes del trabajo W1.
- Obtener la aprobación legal y un espacio de puesta en escena para el poder, después de las etapas de
  verificación previa de los vagos participantes detallados en [vista previa del flujo de invitación] (../../preview-invite-flow.md).

_Este resumen se encuentra después del [rastreador de invitación de vista previa](../../preview-invite-tracker.md) para
guarde la hoja de ruta DOCS-SORA rastreable._