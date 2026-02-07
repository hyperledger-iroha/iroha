---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
título: Plano de admissão comunitário W2
sidebar_label: Plano W2
descrição: Ingestão, aprovações e lista de verificação de evidências para a coorte de pré-visualização comunitária.
---

| Artigo | Detalhes |
| --- | --- |
| Olá | W2 - Revisores comunitários |
| Ventana objetivo | 3º trimestre de 2025, semana 1 (provisória) |
| Etiqueta de artefato (planeado) | `preview-2025-06-15` |
| Emissão do rastreador | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Definir critérios de admissão comunitária e fluxo de verificação.
2. Obter aprovação de governo para a lista proposta e o adendo de uso aceitável.
3. Atualize o artefato de visualização selecionado por checksum e o pacote de telemetria para a nova janela.
4. Prepare o proxy Try it e os painéis antes de enviar os convites.

## Desglose de tareias

| ID | Tara | Responsável | Data limite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Reditar critérios de admissão comunitários (elegibilidad, max slots, requisitos de CoC) e circular a gobernanza | Líder do Documentos/DevRel | 15/05/2025 | Concluído | A política de admissão foi fundida em `DOCS-SORA-Preview-W2` e respalda na reunião do conselho 20/05/2025. |
| W2-P2 | Atualizar modelo de solicitação com perguntas específicas de comunidade (motivação, disponibilidade, necessidades de localização) | Documentos-núcleo-01 | 18/05/2025 | Concluído | `docs/examples/docs_preview_request_template.md` agora inclui a seção Comunidade, referenciada no formulário de ingestão. |
| W2-P3 | Garantir aprovação de governo para o plano de admissão (voto em reunião + atos registrados) | Ligação para governação | 22/05/2025 | Concluído | Voto aprovado por unanimidade em 2025-05-20; atos e chamada enlazados em `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programar staging del proxy Try it + captura de telemetria para a janela W2 (`preview-2025-06-15`) | Documentos/DevRel + Operações | 05/06/2025 | Concluído | Ticket de mudança `OPS-TRYIT-188` aprovado e executado 09/06/2025 02:00-04:00 UTC; capturas de tela de Grafana arquivadas com o ticket. |
| W2-P5 | Construir/verificar nova tag de artefato de visualização (`preview-2025-06-15`) e arquivar descritor/checksum/probe logs | PortalTL | 07/06/2025 | Concluído | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` foi executado em 10/06/2025; gera arquivos salvos abaixo de `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Armar lista de convites comunitários (<=25 revisores, muitos escalados) com informações de contato aprovadas pelo governo | Gerente de comunidade | 10/06/2025 | Concluído | Primeira coorte de 8 revisores comunitários aprovados; IDs de solicitação `DOCS-SORA-Preview-REQ-C01...C08` registrados no rastreador. |

## Checklist de evidências

- [x] Registro de aprovação de governo (notas de reunião + link de voto) adjunto a `DOCS-SORA-Preview-W2`.
- [x] Modelo de solicitação atualizado comprometido abaixo `docs/examples/`.
- [x] Descritor `preview-2025-06-15`, log de checksum, saída da sonda, relatório de link e transcrição do proxy Try it salvos abaixo de `artifacts/docs_preview/W2/`.
- [x] Capturas de tela de Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturadas para a janela de comprovação W2.
- [x] Tabela de lista de convites com IDs de revisores, tickets de solicitação e carimbos de data e hora de aprovação completados antes do envio (ver seção W2 do tracker).

Manter este plano atualizado; O rastreador é referência para que o roteiro DOCS-SORA seja exatamente o que resta antes de enviar os convites W2.