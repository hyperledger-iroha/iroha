---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-plan
título: Plano de toma comunitaria W2
etiqueta_barra lateral: Plano W2
descripción: Ingesta, aprovacoes y checklist de evidencia para una corte de vista previa comunitaria.
---

| Artículo | Detalles |
| --- | --- |
| Onda | W2 - Revisores comunitarios |
| Janela alvo | T3 2025 semana 1 (tentativa) |
| Etiqueta de artefato (planejado) | `preview-2025-06-15` |
| Problema con el rastreador | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Definir criterios de ingesta comunitaria y flujo de trabajo de vetting.
2. Obtener aprobación de gobierno para la lista propuesta y el addendum de uso aceitavel.
3. Actualizar el artefato de vista previa verificado por suma de comprobación y el paquete de telemetría para una nueva janela.
4. Preparar el proxy Pruébalo en los paneles de control antes de enviar dos invitaciones.

##Desdobramento de tarefas| identificación | Tarefa | Responsavel | Prazo | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| G2-P1 | Redigir criterios de admisión comunitaria (eligibilidade, max slots, requisitos de CoC) y circular para gobernanza | Líder de Docs/DevRel | 2025-05-15 | Concluido | Una política de admisión se fusionó en `DOCS-SORA-Preview-W2` y se endossada na reuniao do conselho 2025-05-20. |
| W2-P2 | Actualizar plantilla de solicitud con perguntas comunitarias (motivación, disponibilidad, necesidades de localización) | Documentos-core-01 | 2025-05-18 | Concluido | `docs/examples/docs_preview_request_template.md` agora inclui a secao Community, referenciada no formulario de ingesta. |
| W2-P3 | Garantía de aprobación de gobierno para el plano de admisión (voto en reunión + atas registradas) | Enlace de gobernanza | 2025-05-22 | Concluido | Voto aprobado por unanimidad en 2025-05-20; atas e roll call vinculados en `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programar staging do proxy Try it + captura de telemetría para a janela W2 (`preview-2025-06-15`) | Documentos/DevRel + Operaciones | 2025-06-05 | Concluido | Boleto de cambio `OPS-TRYIT-188` aprobado y ejecutado el 2025-06-09 02:00-04:00 UTC; capturas de pantalla Grafana arquivados con ticket. |
| W2-P5 | Construir/verificar nueva etiqueta de artefato de vista previa (`preview-2025-06-15`) y archivar descriptor/checksum/probe logs | Portal TL | 2025-06-07 | Concluido | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` rodou em 2025-06-10; salidas armadas en `artifacts/docs_preview/W2/preview-2025-06-15/`. || W2-P6 | Montar roster de convites comunitarios (<=25 críticos, lotes escalonados) con contactos aprobados por gobernador | Responsable de la comunidad | 2025-06-10 | Concluido | Primeiro cohorte de 8 revisores comunitarios aprobado; ID de solicitud `DOCS-SORA-Preview-REQ-C01...C08` registrados sin rastreador. |

## Lista de verificación de evidencia

- [x] Registro de aprovacao de gobernadora (notas de reuniao + link de voto) anexado a `DOCS-SORA-Preview-W2`.
- [x] Template de solicitacao atualizado commited sob `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, registro de suma de comprobación, salida de sonda, informe de enlace y transcripción del proxy Pruébelo armado en `artifacts/docs_preview/W2/`.
- [x] Capturas de pantalla Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturadas para a janela preflight W2.
- [x] Tabela de roster de convites com IDs de reviewers, tickets de solicitacao e timestamps de aprovacao preenchidos antes de enviar (ver secoo W2 no tracker).

Mantenha este plano actualizado; o tracker o referencia para que o roadmap DOCS-SORA veja exactamente o que falta antes de enviar convites W2.