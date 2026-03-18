---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
título: Plano de admissão comunitário W2
sidebar_label: Plano W2
descrição: Admissão, aprovações e checklist de evidências para a coorte de visualização comunitária.
---

| Artigo | Detalhes |
| --- | --- |
| Onda | W2 - Revisores comunitários |
| Janela alvo | 3º trimestre de 2025, semana 1 (provisória) |
| Tag de arte (planejado) | `preview-2025-06-15` |
| Problema do rastreador | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Definir critérios de admissão comunitária e fluxo de trabalho de verificação.
2. Obter aprovação de governança para a lista proposta e o adendo de uso aceitavel.
3. Atualize os artefatos de visualização selecionados por checksum e o pacote de telemetria para uma nova janela.
4. Prepare o proxy Try it e os dashboards antes de enviar os convites.

## Desdobramento de tarefas

| ID | Tarefa | Responsável | Prazo | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Redigir critérios de admissão comunitários (eligibilidade, max slots, requisitos de CoC) e circular para governança | Líder do Documentos/DevRel | 15/05/2025 | Concluído | A política de admissão foi fundida em `DOCS-SORA-Preview-W2` e endossada na reunião do conselho 2025-05-20. |
| W2-P2 | Atualizar template de solicitação com perguntas comunitárias (motivação, disponibilidade, necessidades de localização) | Documentos-núcleo-01 | 18/05/2025 | Concluído | `docs/examples/docs_preview_request_template.md` agora inclui a seção Community, referenciada no formulário de admissão. |
| W2-P3 | Garantir aprovação de governança para o plano de captação (voto em reunião + atas registradas) | Ligação para governação | 22/05/2025 | Concluído | Voto aprovado por unanimidade em 2025-05-20; atas e lista de chamada vinculadas em `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programar staging do proxy Try it + captura de telemetria para a janela W2 (`preview-2025-06-15`) | Documentos/DevRel + Operações | 05/06/2025 | Concluído | Ticket de alteração `OPS-TRYIT-188` aprovado e concluído em 2025-06-09 02:00-04:00 UTC; screenshots Grafana arquivados com o ticket. |
| W2-P5 | Construir/verificar nova tag de arte de visualização (`preview-2025-06-15`) e arquivar descriptor/checksum/probe logs | PortalTL | 07/06/2025 | Concluído | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` rodado em 10/06/2025; saídas armazenadas em `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Montar lista de convites comunitários (<=25 revisores, lotes escalados) com contatos aprovados por governança | Gerente de comunidade | 10/06/2025 | Concluído | Primeira coorte de 8 revisores comunitários aprovados; IDs de requisição `DOCS-SORA-Preview-REQ-C01...C08` registrados no tracker. |

## Checklist de evidências

- [x] Registro de aprovação de governança (notas de reunião + link de voto) anexado a `DOCS-SORA-Preview-W2`.
- [x] Template de solicitação de atualização commited sob `docs/examples/`.
- [x] Descritor `preview-2025-06-15`, checksum log, probe output, link report e transcript do proxy Try it armazenado em `artifacts/docs_preview/W2/`.
- [x] Capturas de tela Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturadas para a janela preflight W2.
- [x] Tabela de lista de convites com IDs de revisores, tickets de solicitação e timestamps de aprovação preenchidos antes do envio (ver seção W2 no tracker).

Mantenha este plano atualizado; o tracker o referencia para que o roadmap DOCS-SORA veja exatamente o que falta antes de enviar convites W2.