---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w0-resumen
título: Resumen de feedback do meio do W0
sidebar_label: Comentarios W0 (meio)
descripción: Puntos de control, achados y acoes de meio de onda para a onda de vista previa de mantenedores core.
---

| Artículo | Detalles |
| --- | --- |
| Onda | W0 - Núcleo de mantenedores |
| Datos del currículum | 2025-03-27 |
| Janela de revisao | 2025-03-25 -> 2025-04-08 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidad-01 |
| Etiqueta de artefato | `preview-2025-03-24` |

## Destaques

1. **Fluxo de checksum** - Todos los revisores confirman que `scripts/preview_verify.sh`
   teve sucesso contra o par descriptor/archive compartilhado. Nenhum anular manual foi
   necesario.
2. **Comentarios de navegación** - Dois problemas menores de ordenacao do sidebar foram
   registrados (`docs-preview/w0 #1-#2`). Ambos foros encaminados para Docs/DevRel y nao
   bloqueiam una onda.
3. **Paridade de runbooks SoraFS** - sorafs-ops-01 pediu links cruzados mais claros entre
   `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. Edición de acompañamiento abierto;
   tratar antes de W1.
4. **Revisao de telemetria** - observability-01 confirmou que `docs.preview.integrity`,
   `TryItProxyErrors` y los registros del sistema operativo hacen proxy Pruébelo ficaram verdes; nenhum alerta disparou.

## Artículos de acao| identificación | Descripción | Responsavel | Estado |
| --- | --- | --- | --- |
| W0-A1 | Reordenar las entradas de la barra lateral del portal de desarrollo para destacar los documentos enfocados en los revisores (`preview-invite-*` agrupados). | Documentos-core-01 | Concluido - la barra lateral ahora lista los documentos de revisores de forma continua (`docs/portal/sidebars.js`). |
| W0-A2 | Enlace adicional cruzado explícito entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Concluido: cada runbook ahora está disponible para el final para que los operadores vean ambas guías durante los lanzamientos. |
| W0-A3 | Compartilhar instantáneas de telemetría + paquete de consultas con o rastreador de gobierno. | Observabilidad-01 | Concluido - paquete anexo a `DOCS-SORA-Preview-W0`. |

## Resumen de encerramento (2025-04-08)

- Todos los cinco revisores confirmaron una conclusión, limpiaron builds locais y sairam da janela
  vista previa; as revogacoes de acesso ficaram registradas em `DOCS-SORA-Preview-W0`.
- Nenhum incidente ou alerta ocorreu durante una onda; os paneles de telemetría ficaram
  verdes durante todo el periodo.
- As acoes de navegacao + links cruzados (W0-A1/A2) estaso implementadas e refletidas nos docs
  ácima; a evidencia de telemetria (W0-A3) esta anexada al tracker.
- Paquete de evidencia archivado: capturas de pantalla de telemetría, confirmacoes de convite e este
  resumen estos vinculados no hay problema con el rastreador.

## Próximos pasos- Implementar los elementos de acao do W0 antes de abrir W1.
- Obtenga la aprobación legal y un espacio de preparación para el proxy, después de seguir los pasos de verificación previa
  da onda de parceiros descritos no [vista previa del flujo de invitación](../../preview-invite-flow.md).

_Este resumen está vinculado desde [preview invite tracker](../../preview-invite-tracker.md) para
manter o hoja de ruta DOCS-SORA rastreavel._