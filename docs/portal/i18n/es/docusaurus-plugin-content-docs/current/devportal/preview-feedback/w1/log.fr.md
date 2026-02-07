---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: vista previa-comentarios-w1-log
título: Comentarios del diario y telemetría W1
sidebar_label: Diario W1
Descripción: Lista agregada, puntos de control de telemetría y notas de revisores para la primera partenaires de vista previa vaga.
---

Este diario conserva la lista de invitaciones, los puntos de control de telemetría y los revisores de comentarios para
**vista previa de partenaires W1** que acompañan los tacos de aceptación en
[`preview-feedback/w1/plan.md`](./plan.md) y la entrada del rastreador de datos vagos
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Mettez-le a jour quand una invitación est enviada,
¿Qué instantánea de telemetría se registra, o qué elemento de retroalimentación se intenta para que los revisores gobiernen bien?
Rejouer les preuves sans courir después de los tickets externos.

## Lista de cohortes| ID de socio | Boleto de demanda | Recuperación de NDA | Invitar enviado (UTC) | Acuse de recibo/inicio de sesión principal (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| socio-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Terminar 2025-04-26 | sorafs-op-01; concentre sur les preuves de parite de docs Orchestrator. |
| socio-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | Terminar 2025-04-26 | sorafs-op-02; valide los enlaces cruzados Norito/telemetrie. |
| socio-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Terminar 2025-04-26 | sorafs-op-03; y ejecutar ejercicios de conmutación por error de múltiples fuentes. |
| socio-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Terminar 2025-04-26 | torii-int-01; revista del libro de cocina Torii `/v1/pipeline` + Pruébalo. |
| socio-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Terminar 2025-04-26 | torii-int-02; acompagne la mise a jour de capture Pruébelo (docs-preview/w1 #2). |
| socio-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Terminar 2025-04-26 | socio-sdk-01; Comentarios sobre libros de cocina JS/Swift + controles de cordura Puente ISO. || socio-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Terminar 2025-04-26 | socio-sdk-02; Cumplimiento valide 2025-04-11, focalise sur notes Connect/telemetrie. |
| socio-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Terminar 2025-04-26 | puerta de enlace-ops-01; auditoría de guía de operaciones puerta de enlace + proxy flux Pruébelo de forma anónima. |

Renseignez **Invitar enviado** y **Ack** des que l'email sortant est emis.
Ancrez les heures au Planning UTC define dans le plan W1.

## Telemetría de puntos de control

| Horodataje (UTC) | Cuadros de mandos / sondas | Responsable | Resultado | Artefacto |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operaciones | Todo verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcripción `npm run manage:tryit-proxy -- --stage preview-w1` | Operaciones | En escena | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Cuadros de mando ci-dessus + `probe:portal` | Documentos/DevRel + Operaciones | Instantánea previa a la invitación, regresión aucune | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Paneles ci-dessus + diferencial de latencia proxy Pruébalo | Líder de Docs/DevRel | Punto de control entorno válido (0 alertas; latencia Pruébelo p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Cuadros de instrumentos ci-dessus + sonda de salida | Docs/DevRel + enlace de gobernanza | Instantánea de salida, cero alertas restantes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |Les echantillons quotidiens d'office hours (2025-04-13 -> 2025-04-25) sont reagrupes en exports NDJSON + PNG sous
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` con los nombres de archivo
`docs-preview-integrity-<date>.json` et les captures correspondants.

## Registrar comentarios y problemas

Utilice esta tabla para reanudar las estadísticas de los revisores. Liez chaque entrada au ticket GitHub/discutir
captura de estructura de formulario ainsi qu'au a través de
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Severita | Responsable | Estatuto | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Bajo | Documentos-core-02 | Resolución 2025-04-18 | Aclaración sobre la redacción de nav Pruébalo + barra lateral adicional (`docs/source/sorafs/tryit.md` mis a jour avec le nouveau label). |
| `docs-preview/w1 #2` | Bajo | Documentos-core-03 | Resolución 2025-04-19 | Captura Pruébalo + legende rafraichies selon la demande; artefacto `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Información | Líder de Docs/DevRel | Ferme | Les commentaires restants etaient Uniquement Preguntas y respuestas; capturas en cada formulario partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Verificación de conocimientos y encuestas de Suivi

1. Registrar las puntuaciones del cuestionario (capaz >=90%) para cada revisor; Joindre le CSV exporte a cote des artefactos de invitación.
2. Recopila las respuestas cualitativas de la encuesta capturadas a través de la plantilla de comentarios y las fotocopiadoras.
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planifier des appels de remediation pour toute personne sous le seuil et les consigner ici.Los pocos revisores obtuvieron >=94% en la verificación de conocimientos (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Aucun apelación de remediación
n'a ete necesario; les exports de Survey pour cada partenaire sont sous
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefactos

- Descriptor de vista previa del paquete/suma de comprobación: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Reanudar sonda + verificación de enlace: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Registro de cambio de proxy Pruébelo: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Telemetría de exportación: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Paquete de telemetría en horario de oficina diario: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Comentarios sobre exportaciones + encuesta: placer des dossiers par reviewer sous
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Comprobación de conocimientos CSV y currículum: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Guarde el inventario sincronizado con el rastreador de problemas. Joindre des hashes lors de la copie d'artefacts vers
el gobierno del ticket afin que los auditores puedan verificar los archivos sin acceso al shell.