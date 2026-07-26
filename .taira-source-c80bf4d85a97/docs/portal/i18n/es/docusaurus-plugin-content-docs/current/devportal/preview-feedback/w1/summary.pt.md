---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-resumen
título: Resumen de retroalimentación y encerramento W1
sidebar_label: Resumen W1
descripción: Achados, acoes e evidencia de encerramento para a onda de vista previa de parceiros/integradores Torii.
---

| Artículo | Detalles |
| --- | --- |
| Onda | W1 - Parceiros e integradores Torii |
| Janela de convite | 2025-04-12 -> 2025-04-26 |
| Etiqueta de artefato | `preview-2025-04-12` |
| Problema con el rastreador | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destaques

1. **Flujo de suma de comprobación** - Todos los revisores validan el descriptor/archivo a través de `scripts/preview_verify.sh`; logs armazenados junto aos agradecimientos de convite.
2. **Telemetria** - Dashboards `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` ficaram verdes por toda una onda; nenhum incidente ou página de alerta.
3. **Documentos de comentarios (`docs-preview/w1`)** - Dois nits menores registrados:
   - `docs-preview/w1 #1`: esclarecer wording de navegacao na secao Pruébalo (resolvido).
   - `docs-preview/w1 #2`: actualizó captura de pantalla de Try it (resolvido).
4. **Paridade de runbooks** - Los operadores de SoraFS confirman que los nuevos enlaces cruzados entre `orchestrator-ops` e `multi-source-rollout` resuelven las preocupaciones de W0.

## Artículos de acao| identificación | Descripción | Responsavel | Estado |
| --- | --- | --- | --- |
| W1-A1 | Actualizar el texto de navegación do Pruébalo conforme `docs-preview/w1 #1`. | Documentos-core-02 | Concluido (2025-04-18). |
| W1-A2 | Actualizar captura de pantalla de Pruébalo conforme `docs-preview/w1 #2`. | Documentos-core-03 | Concluido (2025-04-19). |
| W1-A3 | Resumir achados de parceiros e evidencia de telemetría en roadmap/status. | Líder de Docs/DevRel | Concluido (ver tracker + status.md). |

## Resumen de encerramento (2025-04-26)

- Todos los oito revisores confirmarán la conclusión durante el horario de oficina final, limpiarán los artefactos locales y retirarán el acceso.
- Una telemetria ficou verde se comió una saya; Instantáneas finais anexadas a `DOCS-SORA-Preview-W1`.
- O log de convites foi atualizado com acknowledgements de saya; El tracker marcou W1 como concluido y agregado a los puntos de control.
- Paquete de evidencia (descriptor, registro de suma de verificación, salida de sonda, transcripción del proxy Pruébelo, capturas de pantalla de telemetría, resumen de comentarios) adquirido en `artifacts/docs_preview/W1/`.

## Próximos pasos

- Preparar o plano de ingesta comunitario W2 (aprovacao degobernanza + ajustes no template de solicitacao).
- Actualizar la etiqueta de artefato de vista previa para una onda W2 y volver a ejecutar el script de verificación previa cuando los datos estiverem finalizados.
- Levar achados aplicaveis de W1 para roadmap/status para que una onda comunitaria tenga una orientación más reciente.