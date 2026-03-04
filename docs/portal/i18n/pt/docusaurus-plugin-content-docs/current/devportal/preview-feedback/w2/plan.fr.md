---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
título: Plano de captação comunitária W2
sidebar_label: Plano W2
descrição: Ingestão, aprovações e lista de verificação de pré-visualização para a coorte de visualização comunitária.
---

| Elemento | Detalhes |
| --- | --- |
| Vago | W2 - Revisores comunitários |
| Fenetre cible | Semana 1 do terceiro trimestre de 2025 (tentativa) |
| Tag d'artefact (planificação) | `preview-2025-06-15` |
| Rastreador de problemas | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Defina os critérios de admissão comunitários e o fluxo de trabalho de verificação.
2. Obter a governança de aprovação para a lista proposta e o adendo de uso aceitável.
3. Rafraichir a visualização do artefato verificada por soma de verificação e o pacote de telemetria para a nova janela.
4. Prepare o proxy Experimente e os painéis antes de enviar os convites.

## Decoupage des taches

| ID | Tache | Responsável | Echeance | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Rediger os critérios de admissão comunitária (elegibilidade, slots máximos, exigências CoC) e difusores de governança | Líder do Documentos/DevRel | 15/05/2025 | Término | A política de admissão foi fundida em `DOCS-SORA-Preview-W2` e endossada durante a reunião do Conselho 2025-05-20. |
| W2-P2 | Encontre hoje o modelo de demanda com perguntas comunitárias (motivação, disponibilidade, necessidades de localização) | Documentos-núcleo-01 | 18/05/2025 | Término | `docs/examples/docs_preview_request_template.md` inclui a manutenção da seção Community, referenciada no formulário de ingestão. |
| W2-P3 | Obtenir a governança de aprovação para a entrada do plano (votação em reunião + atas registradas) | Ligação para governação | 22/05/2025 | Término | Vote adotado por unanimite em 2025-05-20; minutos + lista de chamada estão em `DOCS-SORA-Preview-W2`. |
| W2-P4 | Planeje o teste do proxy Try it + capture telemetrie para a janela W2 (`preview-2025-06-15`) | Documentos/DevRel + Operações | 05/06/2025 | Término | Alteração do ticket `OPS-TRYIT-188` aprovada e executada em 09/06/2025 02:00-04:00 UTC; capturas de tela Grafana arquiva com o ticket. |
| W2-P5 | Construa/verifique a nova tag de visualização do artefato (`preview-2025-06-15`) e arquive os logs do descritor/checksum/probe | PortalTL | 07/06/2025 | Término | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` executado 10/06/2025; produz estoques sous `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Montar a lista de convites comunitários (<=25 revisores, muitos escalões) com contatos aprovados pela governança | Gerente de comunidade | 10/06/2025 | Término | Estreia coorte de 8 revisores comunitários aprovados; Os IDs da mensagem `DOCS-SORA-Preview-REQ-C01...C08` são registrados no rastreador. |

## Checklist prévio

- [x] Registro de governança de aprovação (notas de reunião + garantia de voto) anexado a `DOCS-SORA-Preview-W2`.
- [x] Modelo de demanda para o dia seguinte sob `docs/examples/`.
- [x] Descritor `preview-2025-06-15`, soma de verificação de log, saída de teste, relatório de link e proxy de transcrição Experimente usar o `artifacts/docs_preview/W2/`.
- [x] Capturas de tela Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturas para a janela pré-voo W2.
- [x] Lista de convites do Tableau com revisores de IDs, tickets solicitados e carimbos de data e hora de aprovação enviados antes do envio (veja a seção W2 do rastreador).

Garder ce planeje um dia; le tracker le reference pour que le roadmap DOCS-SORA voie exatamente o que resta antes do envio dos convites W2.