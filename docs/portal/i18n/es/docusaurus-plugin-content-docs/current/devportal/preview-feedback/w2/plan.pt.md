---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w2-plan
title: Plano de intake comunitario W2
sidebar_label: Plano W2
description: Intake, aprovacoes e checklist de evidencia para a coorte de preview comunitaria.
---

| Item | Detalhes |
| --- | --- |
| Onda | W2 - Reviewers comunitarios |
| Janela alvo | Q3 2025 semana 1 (tentativa) |
| Tag de artefato (planejado) | `preview-2025-06-15` |
| Issue do tracker | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Definir criterios de intake comunitario e workflow de vetting.
2. Obter aprovacao de governanca para o roster proposto e o addendum de uso aceitavel.
3. Atualizar o artefato de preview verificado por checksum e o bundle de telemetria para a nova janela.
4. Preparar o proxy Try it e os dashboards antes do envio dos convites.

## Desdobramento de tarefas

| ID | Tarefa | Responsavel | Prazo | Status | Notas |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Redigir criterios de intake comunitario (eligibilidade, max slots, requisitos de CoC) e circular para governanca | Docs/DevRel lead | 2025-05-15 | Concluido | A politica de intake foi mergeada em `DOCS-SORA-Preview-W2` e endossada na reuniao do conselho 2025-05-20. |
| W2-P2 | Atualizar template de solicitacao com perguntas comunitarias (motivacao, disponibilidade, necessidades de localizacao) | Docs-core-01 | 2025-05-18 | Concluido | `docs/examples/docs_preview_request_template.md` agora inclui a secao Community, referenciada no formulario de intake. |
| W2-P3 | Garantir aprovacao de governanca para o plano de intake (voto em reuniao + atas registradas) | Governance liaison | 2025-05-22 | Concluido | Voto aprovado por unanimidade em 2025-05-20; atas e roll call linkados em `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programar staging do proxy Try it + captura de telemetria para a janela W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | Concluido | Ticket de change `OPS-TRYIT-188` aprovado e executado em 2025-06-09 02:00-04:00 UTC; screenshots Grafana arquivados com o ticket. |
| W2-P5 | Construir/verificar novo tag de artefato de preview (`preview-2025-06-15`) e arquivar descriptor/checksum/probe logs | Portal TL | 2025-06-07 | Concluido | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` rodou em 2025-06-10; outputs armazenados em `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Montar roster de convites comunitarios (<=25 reviewers, lotes escalonados) com contatos aprovados por governanca | Community manager | 2025-06-10 | Concluido | Primeiro cohorte de 8 reviewers comunitarios aprovado; IDs de requisicao `DOCS-SORA-Preview-REQ-C01...C08` registrados no tracker. |

## Checklist de evidencia

- [x] Registro de aprovacao de governanca (notas de reuniao + link de voto) anexado a `DOCS-SORA-Preview-W2`.
- [x] Template de solicitacao atualizado commited sob `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, checksum log, probe output, link report e transcript do proxy Try it armazenados em `artifacts/docs_preview/W2/`.
- [x] Screenshots Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturados para a janela preflight W2.
- [x] Tabela de roster de convites com IDs de reviewers, tickets de solicitacao e timestamps de aprovacao preenchidos antes do envio (ver secao W2 no tracker).

Mantenha este plano atualizado; o tracker o referencia para que o roadmap DOCS-SORA veja exatamente o que falta antes de enviar convites W2.
