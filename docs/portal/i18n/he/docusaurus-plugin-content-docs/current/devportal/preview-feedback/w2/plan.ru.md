---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w2-plan
title: План community intake W2
sidebar_label: План W2
description: Intake, одобрения и чек-лист доказательств для community preview когорты.
---

| Пункт | Детали |
| --- | --- |
| Волна | W2 - community reviewers |
| Целевое окно | Q3 2025 неделя 1 (предварительно) |
| Тег артефакта (план) | `preview-2025-06-15` |
| Трекер | `DOCS-SORA-Preview-W2` |

## Цели

1. Определить критерии community intake и workflow vetting.
2. Получить governance-одобрение для предложенного roster и acceptable-use addendum.
3. Обновить checksum-верифицированный preview-артефакт и телеметрический bundle под новое окно.
4. Подготовить Try it proxy и dashboards до отправки приглашений.

## Разбивка задач

| ID | Задача | Владелец | Срок | Статус | Примечания |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Подготовить критерии community intake (eligibility, max slots, требования CoC) и разослать в governance | Docs/DevRel lead | 2025-05-15 | ✅ Завершено | Политика intake смержена в `DOCS-SORA-Preview-W2` и одобрена на совете 2025-05-20. |
| W2-P2 | Обновить request template вопросами для community (motivation, availability, localization needs) | Docs-core-01 | 2025-05-18 | ✅ Завершено | `docs/examples/docs_preview_request_template.md` теперь включает раздел Community и указан в intake форме. |
| W2-P3 | Получить governance-одобрение плана intake (голосование + протокол) | Governance liaison | 2025-05-22 | ✅ Завершено | Голосование прошло единогласно 2025-05-20; протокол и roll call связаны в `DOCS-SORA-Preview-W2`. |
| W2-P4 | Запланировать staging Try it proxy + телеметрию для окна W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ Завершено | Change ticket `OPS-TRYIT-188` одобрен и выполнен 2025-06-09 02:00-04:00 UTC; Grafana скриншоты сохранены с тикетом. |
| W2-P5 | Собрать/проверить новый preview artefact tag (`preview-2025-06-15`) и архивировать descriptor/checksum/probe logs | Portal TL | 2025-06-07 | ✅ Завершено | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` выполнен 2025-06-10; outputs сохранены в `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Сформировать roster community приглашений (<=25 reviewers, staged batches) с контактами, одобренными governance | Community manager | 2025-06-10 | ✅ Завершено | Первая когорта из 8 community reviewers одобрена; IDs `DOCS-SORA-Preview-REQ-C01...C08` записаны в трекере. |

## Чек-лист доказательств

- [x] Запись governance approval (заметки встречи + ссылка на голосование) приложена к `DOCS-SORA-Preview-W2`.
- [x] Обновленный request template закоммичен под `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, checksum log, probe output, link report и Try it proxy transcript сохранены в `artifacts/docs_preview/W2/`.
- [x] Grafana screenshots (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) сняты для окна preflight W2.
- [x] Таблица roster приглашений с reviewer IDs, request tickets и timestamps одобрений заполнена до отправки (см. секцию W2 в трекере).

Держите этот план актуальным; трекер ссылается на него, чтобы roadmap DOCS-SORA видел, что осталось до рассылки W2 приглашений.
