---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: предварительный просмотр-обратная связь-w1-log
Название: Журнал обратной связи и телеметрии W1
Sidebar_label: Журнал W1
описание: Совокупный список, контрольные точки телеметрии и заметки рецензентов для премьеры расплывчатого предварительного просмотра партнеров.
---

В журнале сохраняется список приглашенных, контрольные точки телеметрии и отзывы рецензентов для
**предварительный просмотр для партнеров W1**, который сопровождает прием в
[`preview-feedback/w1/plan.md`](./plan.md) и вход в трекер туманных данных
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Mettez-le a jour quand une приглашение - посланник,
Если зарегистрирован снимок телеметрии или элемент обратной связи, который может быть использован рецензентами для управления
возобновите работу без курьера после внешних билетов.

## Состав когорты

| Идентификатор партнера | Билет по требованию | Соглашение о неразглашении | Пригласить посланника (UTC) | Подтвержденный/премьер-вход (UTC) | Статут | Заметки |
| --- | --- | --- | --- | --- | --- | --- |
| партнер-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ОК 03.04.2025 | 2025-04-12 15:00 | 2025-04-12 15:11 | Конец 2025-04-26 | сорафс-оп-01; сконцентрироваться на превентивных мерах по оркестрации документов. |
| партнер-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ОК 03.04.2025 | 2025-04-12 15:03 | 2025-04-12 15:15 | Конец 2025-04-26 | сорафс-оп-02; действительные перекрестные ссылки Norito/telemetrie. |
| партнер-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ОК 04.04.2025 | 2025-04-12 15:06 | 2025-04-12 15:18 | Конец 2025-04-26 | сорафс-оп-03; выполнение упражнений по аварийному переключению из нескольких источников. |
| партнер-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ОК 04.04.2025 | 2025-04-12 15:09 | 2025-04-12 15:21 | Конец 2025-04-26 | тории-int-01; ревю кулинарной книги Torii `/v2/pipeline` + Попробуйте. |
| партнер-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ОК 05.04.2025 | 2025-04-12 15:12 | 2025-04-12 15:23 | Конец 2025-04-26 | тории-int-02; a compagne la mise a jour de capture Попробуйте (docs-preview/w1 #2). |
| партнер-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ОК 05.04.2025 | 2025-04-12 15:15 | 2025-04-12 15:26 | Конец 2025-04-26 | SDK-партнер-01; кулинарные книги обратной связи JS/Swift + проверка работоспособности моста ISO. |
| партнер-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ОК 11.04.2025 | 2025-04-12 15:18 | 2025-04-12 15:29 | Конец 2025-04-26 | SDK-партнер-02; соответствие действительно на 11 апреля 2025 г., в фокусе примечаний Connect/telemetrie. |
| партнер-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ОК 11.04.2025 | 2025-04-12 15:21 | 12.04.2025 15:33 | Конец 2025-04-26 | шлюз-опс-01; Audit du Guide Ops Gateway + Flux Proxy Попробуйте анонимизировать. |

Renseignez **Пригласите посланника** и **Подтвердите** сортировку электронной почты по электронной почте.
Еще несколько часов при планировании UTC, определенных в плане W1.

## Телеметрия контрольно-пропускных пунктов| Городатаге (UTC) | Панели мониторинга/зонды | Ответственный | Результат | Артефакт |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Документы/DevRel + Ops | Все верт | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Расшифровка `npm run manage:tryit-proxy -- --stage preview-w1` | Операции | Постановочный | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Панели мониторинга ci-dessus + `probe:portal` | Документы/DevRel + Ops | Предварительное приглашение моментального снимка, регрессия aucune | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Dashboards ci-dessus + прокси-сервер diff delatence Попробуйте | Руководитель отдела документации и разработки | Действующая среда контрольной точки (0 предупреждений; задержка Попробуйте p95=410 мс) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Приборные панели ci-dessus + зонд вылета | Документы/DevRel + связь с управлением | Снимок вылета, ноль оповещений о остатках | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Les echantillons quotidiens d'office Часы (13 апреля 2025 г. -> 25 апреля 2025 г.) с перегруппировкой и экспортом NDJSON + PNG sous
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` с указанными именами
`docs-preview-integrity-<date>.json` и др. захватывают корреспондентов.

## Записывать отзывы и проблемы

Используйте эту таблицу для резюме рецензентов. Liez chaque entree au Ticket GitHub/обсудить
захват структуры ainsi qu'au Formulaire через
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Ссылка | Серьезный | Ответственный | Статут | Заметки |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Низкий | Документы-core-02 | Резолю 18 апреля 2025 г. | Разъяснение формулировки навигации. Попробуйте + дополнительная боковая панель (`docs/source/sorafs/tryit.md` не соответствует новому ярлыку). |
| `docs-preview/w1 #2` | Низкий | Документы-core-03 | Резолю 19 апреля 2025 г. | Захват Попробуйте + легенда rafraichies selon la requiree; артефакт `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Информация | Руководитель отдела документации и разработки | Ферме | Les commentaires restants etaient uniquement Вопросы и ответы; захватывает dans chaque formaire partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Проверка знаний Suivi и опросы

1. Зарегистрируйте результаты викторины (cible >=90%) для каждого рецензента; Объедините CSV-файлы с приглашениями.
2. Сбор качественных ответов, полученных в результате опроса, с помощью шаблона обратной связи и копировального аппарата.
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planifier des Appels de Remediation for toute personne sous le seuil et les consigner ici.

Самые важные рецензенты получают >=94 % при проверке знаний (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Аукун апелляционная жалоба на исправление ситуации
n'a ete necessaire; Обзор экспорта для Chaque Partenaire Sont Sous
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Изобретение артефактов

- Дескриптор/контрольная сумма предварительного просмотра пакета: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`.
- Проверка возобновления + проверка связи: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Журнал изменений прокси. Попробуйте: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`.
- Экспорт телеметрии: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Пакетная телеметрия в рабочие часы ежедневно: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Обратная связь по экспорту + опрос: размещение досье по запросу рецензента
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Проверка знаний CSV и резюме: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Garder l'inventaire синхронизируется с трекером проблем. Соединение хэшей с копиями артефактов
управление билетами зависит от того, какие аудиторы могут проверить фичи без доступа к оболочке.