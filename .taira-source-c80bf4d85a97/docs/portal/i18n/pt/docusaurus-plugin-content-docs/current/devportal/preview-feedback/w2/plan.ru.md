---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
título: Plano de admissão da comunidade W2
sidebar_label: Plano W2
description: Intake, одобрения и чек-лист доказательств для community preview когорты.
---

| Ponto | Detalhes |
| --- | --- |
| Volna | W2 - revisores da comunidade |
| Céu aberto | 3º trimestre de 2025, 1º trimestre (previsto) |
| Тег артефакта (plан) | `preview-2025-06-15` |
| Trecker | `DOCS-SORA-Preview-W2` |

##Céli

1. Critérios específicos de admissão da comunidade e verificação do fluxo de trabalho.
2. Definindo governança corporativa para lista de escalação e adendo de uso aceitável.
3. Abra o checksum-верифицированный arte-visualização e pacote telemétrico em um novo dia.
4. Tente experimentar proxy e painéis para usar a opção.

## Разбивка задач

| ID | Bem | Владелец | Croco | Status | Nomeação |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Critérios de admissão da comunidade (elegibilidade, slots máximos, número de CoC) e разослать в governança | Líder do Documentos/DevRel | 15/05/2025 | ✅ Melhor | A política de ingestão foi ajustada em `DOCS-SORA-Preview-W2` e aprovada em 20/05/2025. |
| W2-P2 | Обновить modelo de solicitação вопросами для comunidade (motivação, disponibilidade, necessidades de localização) | Documentos-núcleo-01 | 18/05/2025 | ✅ Melhor | `docs/examples/docs_preview_request_template.md` теперь включает раздел Comunidade e указан в entrada forma. |
| W2-P3 | Получить governança-одобрение planejamento ingestão (голосование + протокол) | Ligação para governação | 22/05/2025 | ✅ Melhor | Голосование прошло единогласно 2025/05/20; protocolo e lista de chamada são enviados para `DOCS-SORA-Preview-W2`. |
| W2-P4 | Запланировать staging Experimente proxy + телеметрию para окна W2 (`preview-2025-06-15`) | Documentos/DevRel + Operações | 05/06/2025 | ✅ Melhor | Alterar ticket `OPS-TRYIT-188` одобрен и выполнен 2025-06-09 02:00-04:00 UTC; Grafana telas seguras com bilhete. |
| W2-P5 | Obtenha/exiba uma nova tag de artefato de visualização (`preview-2025-06-15`) e архивировать descritor/checksum/logs de sonda | PortalTL | 07/06/2025 | ✅ Melhor | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` lançado em 10/06/2025; saídas definidas em `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Сформировать roster community приглашений (<=25 revisores, lotes escalonados) с контактами, одобренными governança | Gerente de comunidade | 10/06/2025 | ✅ Melhor | Первая когорта из 8 revisores da comunidade одобрена; IDs `DOCS-SORA-Preview-REQ-C01...C08` são registrados no rastreamento. |

## Чек-list доказательств

- [x] Aprovação de governança Запись (заметки встречи + ссылка на голосование) приложена к `DOCS-SORA-Preview-W2`.
- [x] Modelo de solicitação Обновленный obtido por `docs/examples/`.
- [x] Descritor `preview-2025-06-15`, log de soma de verificação, saída de sonda, relatório de link e Experimente a transcrição de proxy сохранены в `artifacts/docs_preview/W2/`.
- [x] Capturas de tela Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) сняты для окна comprovação W2.
- [x] Lista de listas de usuários com IDs de revisores, tickets de solicitação e carimbos de data e hora são adicionados à opção (como na seção W2 no rastreamento).

Verifique este plano atual; трекер ссылается на него, чтобы roadmap DOCS-SORA видел, что осталось до рассылки W2 приглашений.