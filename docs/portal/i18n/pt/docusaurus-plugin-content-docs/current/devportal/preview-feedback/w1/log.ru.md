---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-log
título: Лог отзывов и телеметрии W1
sidebar_label: Logo W1
descrição: Lista sueca, pontos de verificação de telemetria e revisores de nomes para pré-visualização de todos os parceiros.
---

Este é um registro de lista de verificação, pontos de verificação de telemetria e revisores contratados para
**visualizar parceiros W1**, сопровождающей задачи приема в
[`preview-feedback/w1/plan.md`](./plan.md) e verifique os valores em
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Обновляйте его, когда отправлено приглашение,
O instantâneo telemétrico ou a triagem são usados ​​no ponto de contato, os revisores de governança podem ser
воспроизвести доказательства без поиска внешних тикетов.

## Рoster когорты

| ID do parceiro | Passagem de ingresso | Declaração de NDA | Transferência aberta (UTC) | Confirmar/permitir login (UTC) | Status | Nomeação |
| --- | --- | --- | --- | --- | --- | --- |
| parceiro-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 03/04/2025 | 12/04/2025 15:00 | 12/04/2025 15:11 | ✅ Finalizado 2025/04/26 | sorafs-op-01; сфокусирован на доказательствах paridade para orquestrador docs. |
| parceiro-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 03/04/2025 | 12/04/2025 15:03 | 12/04/2025 15:15 | ✅ Finalizado 2025/04/26 | sorafs-op-02; проверил ligações cruzadas Norito/telemetria. |
| parceiro-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 04/04/2025 | 12/04/2025 15:06 | 12/04/2025 15:18 | ✅ Finalizado 2025/04/26 | sorafs-op-03; executou exercícios de failover de múltiplas fontes. |
| parceiro-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 04/04/2025 | 12/04/2025 15:09 | 12/04/2025 15:21 | ✅ Finalizado 2025/04/26 | torii-int-01; ревью livro de receitas Torii `/v1/pipeline` + Experimente. |
| parceiro-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 05/04/2025 | 12/04/2025 15:12 | 12/04/2025 15:23 | ✅ Finalizado 2025/04/26 | torii-int-02; участвовал в обновлении скриншота Experimente (docs-preview/w1 #2). |
| parceiro-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 05/04/2025 | 12/04/2025 15:15 | 12/04/2025 15:26 | ✅ Finalizado 2025/04/26 | SDK-parceiro-01; feedback sobre o livro de receitas JS/Swift + verificações de sanidade para ponte ISO. |
| parceiro-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 11/04/2025 | 12/04/2025 15:18 | 12/04/2025 15:29 | ✅ Finalizado 2025/04/26 | SDK-parceiro-02; conformidade закрыт 2025-04-11, фокус на заметках Connect/telemetry. |
| parceiro-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 11/04/2025 | 12/04/2025 15:21 | 12/04/2025 15:33 | ✅ Finalizado 2025/04/26 | gateway-ops-01; аудит ops гайда gateway + анонимизированный поток Experimente proxy. |

Clique em **Приглашение отправлено** e **Ack** сразу отправки письма.
Привяжите время к UTC расписанию, заданному в плане W1.

## Pontos de controle telemétricos

| Roma (UTC) | Painéis/sondas | Владелец | Resultado | Artefato |
| --- | --- | --- | --- | --- |
| 06/04/2025 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operações | ✅ O que você precisa | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06/04/2025 18:20 | Transcrito `npm run manage:tryit-proxy -- --stage preview-w1` | Operações | ✅ Modificação | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 12/04/2025 14:45 | Você precisa de + `probe:portal` | Documentos/DevRel + Operações | ✅ Instantâneo do pré-convite, sem registro | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | Дашборды выше + diff по латентности Experimente proxy | Líder do Documentos/DevRel | ✅ Verificação do ponto médio прошел (0 алертов; латентность Experimente p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Дашборды выше + sonda de saída | Contato do Docs/DevRel + Governança | ✅ Sair do instantâneo, não ativar alertas | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Ежедневные выборки horário comercial (2025-04-13 -> 2025-04-25) упакованы как NDJSON + PNG экспорты под
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` com ícones de imagem
`docs-preview-integrity-<date>.json` e telas seguras.

## Лог отзывов e problemas

Use esta tabela para revisores de resultados de resumo. Selecione o elemento de segurança no GitHub/discuss
bilhete e formato de estrutura, definição de valor
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referência | Gravidade | Proprietário | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Baixo | Documentos-núcleo-02 | ✅ Resolvido 18/04/2025 | Уточнены формулировка навигации Experimente + якорь barra lateral (`docs/source/sorafs/tryit.md` обновлен новым etiqueta). |
| `docs-preview/w1 #2` | Baixo | Documentos-núcleo-03 | ✅ Resolvido 19/04/2025 | Tela aberta Experimente e verifique; artefato `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informações | Líder do Documentos/DevRel | 🟢 Fechado | Comentários gerais sobre perguntas e respostas; зафиксированы в форме каждого партнера pod `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Verificação de conhecimento e pesquisas1. Запишите результаты quiz (цель >=90%) para o revisor; Crie um arquivo CSV de exportação com um arquivo de arte.
2. Registre-se em uma pesquisa aberta, registre o feedback do formulário e solicite-o
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Limpe as soluções de remediação para você, que não são úteis, e remova-as neste caso.

Todos os revisores obtiveram >=94% na verificação de conhecimento (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). remediação звонки не потребовались;
pesquisa de exportações para o parceiro de negócios
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventários de artefactos

- Descritor/soma de verificação de visualização do pacote: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Sonda de resumo + verificação de link: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Лог изменений Experimente proxy: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportações de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
Pacote diário de telemetria no horário comercial: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Feedback + exportações de pesquisa: размещать папки por revisor под
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV de verificação de conhecimento e resumo: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Deixe a sincronização sincronizada com o rastreamento de problemas. Por cópias de artefatos de governança de ingressos
прикладывайте хэши, чтобы аудиторы podem fornecer arquivos sem shell-доступа.