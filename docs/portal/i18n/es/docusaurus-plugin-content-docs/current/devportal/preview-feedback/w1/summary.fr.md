---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-resumen
título: Reanudar comentarios y salida W1
sidebar_label: reanudar W1
descripción: Constantes, acciones y preferencias de salida para la vaga de vista previa partenaires/integrateurs Torii.
---

| Elemento | Detalles |
| --- | --- |
| Vago | W1 - Partenaires e integradores Torii |
| Ventana de invitación | 2025-04-12 -> 2025-04-26 |
| Etiqueta de artefacto | `preview-2025-04-12` |
| Rastreador de problemas | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Puntos navegantes

1. **Suma de verificación del flujo de trabajo**: todos los revisores pueden verificar el descriptor/archivo a través de `scripts/preview_verify.sh`; les logs ont ete stocks avec les accuses d'invitation.
2. **Telemetría** - Los tableros `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals` están en reposo en todo el mundo; aucun incidente ni pagina d'alerte.
3. **Documentos de comentarios (`docs-preview/w1`)** - Deux nits mineurs ont ete signales:
   - `docs-preview/w1 #1`: aclara la formulación de navegación en la sección Pruébalo (resolución).
   - `docs-preview/w1 #2`: mettre a jour la capture Pruébelo (resolución).
4. **Parite runbook** - Los operadores SoraFS confirman que los nuevos enlaces cruzados entre `orchestrator-ops` e `multi-source-rollout` se encuentran en los puntos W0.

## Acciones| identificación | Descripción | Responsable | Estatuto |
| --- | --- | --- | --- |
| W1-A1 | Mettre a jour la formulación de navegación Pruébelo según `docs-preview/w1 #1`. | Documentos-core-02 | Terminar (2025-04-18). |
| W1-A2 | Rafraichir la capture Pruébelo según `docs-preview/w1 #2`. | Documentos-core-03 | Terminar (19-04-2025). |
| W1-A3 | Reanudar las estadísticas de los socios y la telemetría previa en la hoja de ruta/estado. | Líder de Docs/DevRel | Terminar (voir tracker + status.md). |

## Reanudar la salida (2025-04-26)

- Los revisores huit confirman la fin del horario de oficina, purgan los artefactos locales y sus accesos a las revocaciones.
- La telemetrie est restee verte jusqu'a la sortie; instantáneas finalux adjunta un `DOCS-SORA-Preview-W1`.
- Le log d'invitations a ete mis a jour avec les accuses de sortie; El rastreador de la marca W1 comienza y agrega los puntos de control.
- Paquete anterior (descriptor, registro de suma de verificación, salida de sonda, transcripción del proxy Pruébelo, capturas de pantalla de telemetría, resumen de comentarios) archivado en `artifacts/docs_preview/W1/`.

## Etapas de prochaines

- Preparador del plan de admisión comunitario W2 (gobernanza de aprobación + ajustes del modelo de demanda).
- Rafraichir le tag d'artefact vista previa para la vaga W2 y relancer le script de preflight una vez les fechas finalizadas.
- Introduzca las estadísticas aplicables de W1 en la hoja de ruta/estado para que la vaga comunidad se ajuste a las últimas indicaciones.