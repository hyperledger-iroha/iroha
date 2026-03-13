---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-log
título: Journal feedback e telemetria W1
sidebar_label: Diário W1
Descrição: Lista agregada, pontos de verificação de telemetria e notas de revisores para a estreia de parceiros de visualização vaga.
---

Este jornal mantém a lista de convites, os pontos de verificação de telemetria e os revisores de feedback para
**partenaires de pré-visualização W1** que acompanham as tabelas de aceitação em
[`preview-feedback/w1/plan.md`](./plan.md) e a entrada do rastreador de vagas em
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Mettez-le a jour quando um convite está enviado,
quando um instantâneo de telemetria é registrado, ou quando um item de feedback é testado para que os revisores de governança possam
rejouer les preuves sans courir apres des tickets externos.

## Lista da coorte

| ID do parceiro | Ticket de demanda | NDA recebido | Convidar enviado (UTC) | Login confirmado/premier (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| parceiro-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 12/04/2025 15:00 | 12/04/2025 15:11 | Término 2025/04/26 | sorafs-op-01; concentre-se nas recomendações de parte dos documentos orquestradores. |
| parceiro-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 12/04/2025 15:03 | 12/04/2025 15:15 | Término 2025/04/26 | sorafs-op-02; valide as ligações cruzadas Norito/telemetria. |
| parceiro-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 12/04/2025 15:06 | 12/04/2025 15:18 | Término 2025/04/26 | sorafs-op-03; uma execução de exercícios de failover multi-fonte. |
| parceiro-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 12/04/2025 15:09 | 12/04/2025 15:21 | Término 2025/04/26 | torii-int-01; revista do livro de receitas Torii `/v2/pipeline` + Experimente. |
| parceiro-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 12/04/2025 15:12 | 12/04/2025 15:23 | Término 2025/04/26 | torii-int-02; a acompanha la mise a jour de capture Experimente (docs-preview/w1 #2). |
| parceiro-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 12/04/2025 15:15 | 12/04/2025 15:26 | Término 2025/04/26 | SDK-parceiro-01; livros de receitas de feedback JS/Swift + verificações de integridade ponte ISO. |
| parceiro-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 12/04/2025 15:18 | 12/04/2025 15:29 | Término 2025/04/26 | SDK-parceiro-02; conformidade válida 2025-04-11, focalise sur notes Connect/telemetrie. |
| parceiro-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 12/04/2025 15:21 | 12/04/2025 15:33 | Término 2025/04/26 | gateway-ops-01; audit du guide ops gateway + flux proxy Experimente anonimizar. |

Renseignez **Convidar enviado** e **Confirmar** que o e-mail enviado é enviado.
Ancrez as horas de planejamento UTC definidas no plano W1.

## Telemetria de pontos de verificação

| Horodatage (UTC) | Painéis/sondas | Responsável | Resultado | Artefato |
| --- | --- | --- | --- | --- |
| 06/04/2025 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operações | Tudo Verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06/04/2025 18:20 | Transcrição `npm run manage:tryit-proxy -- --stage preview-w1` | Operações | Encenado | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 12/04/2025 14:45 | Painéis ci-dessus + `probe:portal` | Documentos/DevRel + Operações | Pré-convite instantâneo, regressão aucune | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | Dashboards ci-dessus + proxy diff de latência Experimente | Líder do Documentos/DevRel | Checkpoint meio válido (0 alertas; latência Experimente p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Dashboards ci-dessus + sonda de surtida | Contato do Docs/DevRel + Governança | Instantâneo da surtida, zero alertas restantes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Os echantillons quotidiens d'office hours (2025-04-13 -> 2025-04-25) são reagrupados nas exportações NDJSON + PNG sous
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` com nomes de arquivo
`docs-preview-integrity-<date>.json` e as capturas correspondentes.

## Registrar comentários e problemas

Utilize este quadro para retomar as estatísticas dos revisores. Liez chaque entree au ticket GitHub/discuss
ainsi qu'au captura de estrutura de formulário via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referência | Severita | Responsável | Estatuto | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Baixo | Documentos-núcleo-02 | Resolução 2025-04-18 | Esclarecimento sobre o texto da navegação Experimente + outra barra lateral (`docs/source/sorafs/tryit.md` recentemente com o novo rótulo). |
| `docs-preview/w1 #2` | Baixo | Documentos-núcleo-03 | Resolução 2025-04-19 | Capture Try it + legende rafraichies selon la demande; artefato `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informações | Líder do Documentos/DevRel | Ferme | Os comentários restantes contêm apenas perguntas e respostas; captura em cada formulário partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Suivi verificação de conhecimento e pesquisas1. Registrar as pontuações do quiz (cível >=90%) para cada revisor; joindre le CSV exporte a cote des artefacts d'invitation.
2. Colete as respostas qualitativas das capturas de pesquisa por meio do modelo de feedback e copie-as
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planeje os apelos de remediação para todas as pessoas que estão no seuil e no remetente aqui.

Os revisores da casa obtiveram >=94% de verificação de conhecimento (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Aucun apelo de remediação
não é necessário; as exportações de pesquisa para cada partenaire são sous
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventário de artefatos

- Descritor/soma de verificação de visualização do pacote: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Retomar sondagem + verificação de link: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de alterações do proxy Experimente: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exporta telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
Pacote de horário comercial diário de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exporta feedback + pesquisa: placer des dossiers par reviewer sous
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Verificação de conhecimento de CSV e currículo: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Garanta a sincronização do inventário com o rastreador de problemas. Junte os hashes na cópia dos artefatos
a governança de tickets para que os auditores possam verificar os arquivos sem acesso ao shell.