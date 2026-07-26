---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-resumen
título: Reanudar comentarios y estado W2
sidebar_label: reanudar W2
descripción: Resumen directo para la vaga de vista previa comunitaria (W2).
---

| Elemento | Detalles |
| --- | --- |
| Vago | W2 - Revisores comunitarios |
| Ventana de invitación | 2025-06-15 -> 2025-06-29 |
| Etiqueta de artefacto | `preview-2025-06-15` |
| Rastreador de problemas | `DOCS-SORA-Preview-W2` |
| Participantes | comunicación-vol-01... comunicación-vol-08 |

## Puntos navegantes

1. **Gobernanza y herramientas** - La política de admisión comunitaria aprobó por unanimidad el 2025-05-20; le template de demande mis a jour avec champs motive/fuseau horaire est dans `docs/examples/docs_preview_request_template.md`.
2. **Preflight et preuves** - El cambio de proxy Pruébelo `OPS-TRYIT-188`, ejecute el 2025-06-09, los paneles Grafana captura y las salidas descriptor/checksum/probe de `preview-2025-06-15` archiva en `artifacts/docs_preview/W2/`.
3. **Invitaciones vagas** - Huit reviewers communautaires invites le 2025-06-15, avec accuses enregistres dans la table d'invitation du tracker; Todos terminamos la suma de verificación de verificación antes de la navegación.
4. **Comentarios** - `docs-preview/w2 #1` (redacción de información sobre herramientas) e `#2` (orden de barra lateral de localización) ont ete saisis le 2025-06-18 et resolus d'ici 2025-06-21 (Docs-core-04/05); Aucun incidente colgante la vague.

## Acciones| identificación | Descripción | Responsable | Estatuto |
| --- | --- | --- | --- |
| W2-A1 | Traiter `docs-preview/w2 #1` (redacción de información sobre herramientas). | Documentos-core-04 | Terminar 2025-06-21 |
| W2-A2 | Traiter `docs-preview/w2 #2` (barra lateral de localización). | Documentos-core-05 | Terminar 2025-06-21 |
| W2-A3 | Archiver les preuves de sortie + mettre a jour hoja de ruta/estado. | Líder de Docs/DevRel | Terminar 2025-06-29 |

## Reanudar la salida (2025-06-29)

- Les huit reviewers communautaires ont confirme la fin et l'acces previa a ete revoque; acusa a los registradores en el registro de invitación del rastreador.
- Las instantáneas finales de telemetría (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) están en blanco; registros y transcripciones del proxy Pruébelo, adjunta un `DOCS-SORA-Preview-W2`.
- Paquete de archivos anteriores (descriptor, registro de suma de verificación, salida de sonda, informe de enlace, capturas de pantalla Grafana, acusaciones de invitación) archivados en `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Le log de checkpoints W2 du tracker a ete mis a jour justsqu'a la sortie, garantissant un enregistrement auditable avant le demarrage de la planification W3.