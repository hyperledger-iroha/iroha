---
lang: ru
direction: ltr
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 42124694cdd79f301ac27514be46b5ade190b03407337ab5c1bf76510d4b26e5
source_last_modified: "2025-12-19T22:34:16.947579+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/governance_playbook.md` и теперь
служит канонической копией портала. Исходный файл остается для PR переводов.
:::

# Плейбук управления Sora Name Service (SN-6)

**Статус:** Подготовлен 2026-03-24 — живой справочник для готовности SN-1/SN-6  
**Ссылки roadmap:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", политика адресов ADDR-1/ADDR-5  
**Предпосылки:** Схема реестра в [`registry-schema.md`](./registry-schema.md), контракт registrar API в [`registrar-api.md`](./registrar-api.md), UX-руководство адресов в [`address-display-guidelines.md`](./address-display-guidelines.md), и правила структуры аккаунтов в [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Этот плейбук описывает, как органы управления Sora Name Service (SNS) принимают
чартеры, утверждают регистрации, эскалируют споры и подтверждают синхронность
состояний resolver и gateway. Он выполняет требование roadmap о том, что CLI
`sns governance ...`, манифесты Norito и аудиторские артефакты используют единый
операторский источник до N1 (публичного запуска).

## 1. Область и аудитория

Документ предназначен для:

- Членов Governance Council, голосующих по чартеру, политикам суффиксов и
  результатам споров.
- Членов guardian board, которые вводят экстренные заморозки и пересматривают
  откаты.
- Steward по суффиксам, ведущих очереди registrar, утверждающих аукционы и
  управляющих распределением дохода.
- Операторов resolver/gateway, отвечающих за распространение SoraDNS, обновления
  GAR и телеметрические guardrails.
- Команд комплаенса, казначейства и поддержки, которые должны доказать, что
  каждое действие управления оставило аудируемые артефакты Norito.

Он охватывает фазы закрытой беты (N0), публичного запуска (N1) и расширения (N2),
перечисленные в `roadmap.md`, связывая каждый workflow с необходимыми доказательствами,
дашбордами и путями эскалации.

## 2. Роли и карта контактов

| Роль | Основные обязанности | Основные артефакты и телеметрия | Эскалация |
|------|----------------------|----------------------------------|-----------|
| Governance Council | Разрабатывает и утверждает чартеры, политики суффиксов, вердикты по спорам и ротации steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, бюллетени совета, сохраненные через `sns governance charter submit`. | Председатель совета + трекер повестки управления. |
| Guardian Board | Выпускает soft/hard заморозки, экстренные каноны и 72 h обзоры. | Тикеты guardian, создаваемые `sns governance freeze`, override-манифесты в `artifacts/sns/guardian/*`. | Ротация guardian on-call (<=15 min ACK). |
| Suffix Stewards | Ведут очереди registrar, аукционы, ценовые уровни и коммуникации с клиентами; подтверждают комплаенс. | Политики steward в `SuffixPolicyV1`, ценовые листы, acknowledgements steward рядом с регуляторными мемо. | Лид программы steward + PagerDuty по суффиксу. |
| Registrar & Billing Ops | Обслуживают `/v2/sns/*` эндпоинты, сверяют платежи, публикуют телеметрию и сохраняют снимки CLI. | Registrar API ([`registrar-api.md`](./registrar-api.md)), метрики `sns_registrar_status_total`, доказательства платежей в `artifacts/sns/payments/*`. | Duty manager registrar и liaison казначейства. |
| Resolver & Gateway Operators | Поддерживают SoraDNS, GAR и состояние gateway в синхронизации с событиями registrar; стримят метрики прозрачности. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE on-call + ops мост gateway. |
| Treasury & Finance | Применяют распределение 70/30, referral carve-outs, налоговые/казначейские отчеты и SLA аттестации. | Манифесты начислений дохода, выгрузки Stripe/казначейства, квартальные KPI приложения в `docs/source/sns/regulatory/`. | Финансовый контролер + комплаенс-офицер. |
| Compliance & Regulatory Liaison | Отслеживает глобальные обязательства (EU DSA и др.), обновляет KPI covenants и подает раскрытия. | Регуляторные мемо в `docs/source/sns/regulatory/`, reference decks, записи `ops/drill-log.md` о tabletop-репетициях. | Лид программы комплаенса. |
| Support / SRE On-call | Обрабатывает инциденты (коллизии, дрейф биллинга, простои resolver), координирует сообщения клиентам и владеет runbook. | Шаблоны инцидентов, `ops/drill-log.md`, лабораторные доказательства, транскрипты Slack/war-room в `incident/`. | Ротация SNS on-call + SRE менеджмент. |

## 3. Канонические артефакты и источники данных

| Артефакт | Расположение | Назначение |
|----------|-------------|-----------|
| Чартер + KPI приложения | `docs/source/sns/governance_addenda/` | Подписанные чартеры с контролем версий, KPI covenants и решения управления, на которые ссылаются CLI-голоса. |
| Схема реестра | [`registry-schema.md`](./registry-schema.md) | Канонические структуры Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Контракт registrar | [`registrar-api.md`](./registrar-api.md) | REST/gRPC payloads, метрики `sns_registrar_status_total` и ожидания governance hook. |
| UX-гайд адресов | [`address-display-guidelines.md`](./address-display-guidelines.md) | Канонические отображения I105 (предпочтительно) и сжатые (второй выбор), используемые кошельками/эксплорерами. |
| Документы SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Детерминированное вычисление host, поток работы transparency tailer и правила алертов. |
| Регуляторные мемо | `docs/source/sns/regulatory/` | Заметки приема по юрисдикциям (например, EU DSA), acknowledgements steward, шаблонные приложения. |
| Drill log | `ops/drill-log.md` | Записи хаос- и IR-репетиций перед выходом из фаз. |
| Хранилище артефактов | `artifacts/sns/` | Доказательства платежей, тикеты guardian, diff resolver, KPI выгрузки и подписанный CLI вывод от `sns governance ...`. |

Все действия управления должны ссылаться минимум на один артефакт из таблицы
выше, чтобы аудиторы могли восстановить цепочку решений в течение 24 часов.

## 4. Плейбуки жизненного цикла

### 4.1 Чартерные и steward-моушены

| Шаг | Владелец | CLI / Доказательства | Примечания |
|-----|----------|----------------------|-----------|
| Черновик приложения и KPI delta | Докладчик совета + лидер steward | Markdown шаблон в `docs/source/sns/governance_addenda/YY/` | Включить ID KPI covenant, телеметрические hooks и условия активации. |
| Подача предложения | Председатель совета | `sns governance charter submit --input SN-CH-YYYY-NN.md` (создает `CharterMotionV1`) | CLI выпускает манифест Norito в `artifacts/sns/governance/<id>/charter_motion.json`. |
| Голосование и guardian acknowledgement | Совет + guardians | `sns governance ballot cast --proposal <id>` и `sns governance guardian-ack --proposal <id>` | Приложить хешированные протоколы и доказательства кворума. |
| Принятие steward | Программа steward | `sns governance steward-ack --proposal <id> --signature <file>` | Требуется до смены политик суффикса; сохранить конверт в `artifacts/sns/governance/<id>/steward_ack.json`. |
| Активация | Registrar ops | Обновить `SuffixPolicyV1`, сбросить кэши registrar, опубликовать заметку в `status.md`. | Таймстамп активации записывается в `sns_governance_activation_total`. |
| Audit log | Комплаенс | Добавить запись в `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` и в drill log, если проводился tabletop. | Добавить ссылки на телеметрические дашборды и policy diff. |

### 4.2 Одобрение регистрации, аукциона и цен

1. **Preflight:** registrar запрашивает `SuffixPolicyV1`, чтобы подтвердить ценовой
   уровень, доступные сроки и окна grace/redemption. Поддерживайте ценовые листы
   синхронизированными с таблицей уровней 3/4/5/6-9/10+ (базовый уровень +
   коэффициенты суффикса), описанной в roadmap.
2. **Sealed-bid аукционы:** Для premium пулов выполните цикл 72 h commit / 24 h
   reveal через `sns governance auction commit` / `... reveal`. Опубликуйте список
   commit (только хеши) в `artifacts/sns/auctions/<name>/commit.json`, чтобы
   аудиторы могли проверить случайность.
3. **Проверка платежа:** registrar валидируют `PaymentProofV1` против распределения
   казначейства (70% treasury / 30% steward с referral carve-out <=10%). Сохраните
   Norito JSON в `artifacts/sns/payments/<tx>.json` и привяжите его к ответу
   registrar (`RevenueAccrualEventV1`).
4. **Governance hook:** Добавьте `GovernanceHookV1` для premium/guarded имен с
   ссылкой на IDs предложений совета и подписи steward. Отсутствие hook приводит к
   `sns_err_governance_missing`.
5. **Активация + sync resolver:** После того как Torii отправит событие реестра,
   запустите transparency tailer resolver, чтобы подтвердить распространение нового
   состояния GAR/zone (см. 4.5).
6. **Клиентское раскрытие:** Обновите клиентский ledger (wallet/explorer) через
   общие fixtures в [`address-display-guidelines.md`](./address-display-guidelines.md),
   убедившись, что I105 и сжатые отображения совпадают с copy/QR гайдами.

### 4.3 Продления, биллинг и сверка казначейства

- **Workflow продления:** registrar применяют окно grace 30 дней + окно redemption
  60 дней, указанное в `SuffixPolicyV1`. Через 60 дней автоматически запускается
  голландская последовательность reopen (7 дней, комиссия 10x с уменьшением 15%/день)
  через `sns governance reopen`.
- **Распределение доходов:** Каждое продление или трансфер создает
  `RevenueAccrualEventV1`. Выгрузки казначейства (CSV/Parquet) должны
  сопоставляться с этими событиями ежедневно; прикладывайте доказательства в
  `artifacts/sns/treasury/<date>.json`.
- **Referral carve-outs:** Необязательные проценты referral отслеживаются по
  суффиксу через `referral_share` в политике steward. registrar публикуют итоговое
  распределение и хранят referral манифесты рядом с доказательством оплаты.
- **Каденс отчетности:** Финансы публикуют ежемесячные KPI приложения
  (регистрации, продления, ARPU, использование споров/bond) в
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Дашборды должны опираться на
  те же выгруженные таблицы, чтобы числа Grafana совпадали с доказательствами ledger.
- **Ежемесячный KPI обзор:** Чекпоинт первого вторника объединяет финансового лида,
  steward on duty и program PM. Откройте [SNS KPI dashboard](./kpi-dashboard.md)
  (портальный embed `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  экспортируйте таблицы throughput + revenue registrar, зафиксируйте дельты в
  приложении и приложите артефакты к мемо. Запускайте инцидент при обнаружении
  SLA нарушений (окна freeze >72 h, всплески ошибок registrar, дрейф ARPU).

### 4.4 Заморозки, споры и апелляции

| Фаза | Владелец | Действие и доказательства | SLA |
|------|----------|----------------------------|-----|
| Запрос soft freeze | Steward / поддержка | Создать тикет `SNS-DF-<id>` с доказательствами платежа, ссылкой на bond спора и затронутыми селекторами. | <=4 h от поступления. |
| Guardian тикет | Guardian board | `sns governance freeze --selector <I105> --reason <text> --until <ts>` создает `GuardianFreezeTicketV1`. Сохранить JSON в `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h выполнение. |
| Ратификация совета | Governance council | Утвердить или отклонить заморозки, задокументировать решение со ссылкой на guardian тикет и digest bond спора. | Следующее заседание совета или асинхронное голосование. |
| Арбитражная панель | Комплаенс + steward | Созвать панель из 7 присяжных (согласно roadmap) с хешированными бюллетенями через `sns governance dispute ballot`. Приложить анонимные квитанции голосов к пакету инцидента. | Вердикт <=7 дней после внесения bond. |
| Апелляция | Guardian + совет | Апелляции удваивают bond и повторяют процесс присяжных; записать манифест Norito `DisputeAppealV1` и сослаться на первичный тикет. | <=10 дней. |
| Разморозка и ремедиация | Registrar + resolver ops | Выполнить `sns governance unfreeze --selector <I105> --ticket <id>`, обновить статус registrar и распространить diff GAR/resolver. | Сразу после вердикта. |

Экстренные каноны (заморозки, инициированные guardian <=72 h) следуют тому же
потоку, но требуют ретроспективного обзора совета и заметки о прозрачности в
`docs/source/sns/regulatory/`.

### 4.5 Распространение resolver и gateway

1. **Event hook:** каждое событие реестра отправляется в поток событий resolver
   (`tools/soradns-resolver` SSE). Resolver ops подписываются и записывают diff через
   transparency tailer (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Обновление GAR шаблона:** gateways должны обновить GAR шаблоны, на которые
   ссылается `canonical_gateway_suffix()`, и переподписать список `host_pattern`.
   Сохранить diff в `artifacts/sns/gar/<date>.patch`.
3. **Публикация zonefile:** Используйте skeleton zonefile, описанный в `roadmap.md`
   (name, ttl, cid, proof), и отправьте его в Torii/SoraFS. Архивируйте Norito JSON
   в `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Проверка прозрачности:** Запустите `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   чтобы убедиться, что алерты зеленые. Приложите текстовый вывод Prometheus к
   еженедельному отчету о прозрачности.
5. **Аудит gateway:** Запишите образцы заголовков `Sora-*` (cache policy, CSP, GAR digest)
   и приложите их к журналу управления, чтобы операторы могли доказать, что gateway
   обслужил новое имя с нужными guardrails.

## 5. Телеметрия и отчетность

| Сигнал | Источник | Описание / Действие |
|--------|----------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Обработчики registrar Torii | Счетчик успех/ошибка для регистраций, продлений, заморозок, трансферов; алерт при росте `result="error"` по суффиксу. |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Метрики Torii | SLO по латентности для API обработчиков; используется в дашбордах из `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` и `soradns_bundle_cid_drift_total` | Resolver transparency tailer | Выявляют устаревшие доказательства или дрейф GAR; guardrails определены в `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Governance CLI | Счетчик, увеличивающийся при активации чартеров/приложений; используется для сверки решений совета и опубликованных addenda. |
| `guardian_freeze_active` gauge | Guardian CLI | Отслеживает окна soft/hard freeze по селектору; пейджить SRE, если значение `1` держится дольше SLA. |
| KPI приложения дашборды | Финансы / Документация | Ежемесячные rollup публикуются вместе с регуляторными мемо; портал встраивает их через [SNS KPI dashboard](./kpi-dashboard.md), чтобы stewards и регуляторы видели одинаковый Grafana вид. |

## 6. Требования к доказательствам и аудиту

| Действие | Доказательства для архива | Хранилище |
|----------|----------------------------|-----------|
| Изменение чартера / политики | Подписанный Norito манифест, CLI транскрипт, KPI diff, stewardship acknowledgement. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Регистрация / продление | `RegisterNameRequestV1` payload, `RevenueAccrualEventV1`, доказательство платежа. | `artifacts/sns/payments/<tx>.json`, логи registrar API. |
| Аукцион | Commit/reveal манифесты, seed случайности, таблица расчета победителя. | `artifacts/sns/auctions/<name>/`. |
| Заморозка / разморозка | Guardian ticket, хеш голосования совета, URL инцидент лога, шаблон коммуникации с клиентом. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Распространение resolver | Zonefile/GAR diff, JSONL выписка tailer, снимок Prometheus. | `artifacts/sns/resolver/<date>/` + отчеты о прозрачности. |
| Регуляторный intake | Intake memo, трекер дедлайнов, acknowledgement steward, сводка KPI изменений. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Чеклист фазовых ворот

| Фаза | Критерии выхода | Пакет доказательств |
|------|------------------|--------------------|
| N0 — Закрытая бета | Схема реестра SN-1/SN-2, ручной registrar CLI, завершенный guardian drill. | Charte motion + steward ACK, dry-run логи registrar, отчет о прозрачности resolver, запись в `ops/drill-log.md`. |
| N1 — Публичный запуск | Аукционы + фиксированные ценовые уровни для `.sora`/`.nexus`, self-service registrar, auto-sync resolver, биллинг дашборды. | Diff ценовых листов, результаты CI registrar, платежные/KPI приложения, вывод transparency tailer, заметки по репетициям инцидентов. |
| N2 — Расширение | `.dao`, reseller API, портал споров, steward scorecards, аналитические дашборды. | Скриншоты портала, SLA метрики споров, выгрузки steward scorecards, обновленный чартер управления с политиками reseller. |

Выход из фаз требует записанных tabletop drills (счастливый путь регистрации,
заморозка, outage resolver) с артефактами в `ops/drill-log.md`.

## 8. Реагирование на инциденты и эскалация

| Триггер | Уровень | Немедленный владелец | Обязательные действия |
|---------|---------|----------------------|-----------------------|
| Дрейф resolver/GAR или устаревшие доказательства | Sev 1 | Resolver SRE + guardian board | Пейджить resolver on-call, собрать вывод tailer, решить вопрос о заморозке затронутых имен, публиковать статус каждые 30 min. |
| Авария registrar, сбой биллинга или массовые API ошибки | Sev 1 | Registrar duty manager | Остановить новые аукционы, перейти на ручной CLI, уведомить stewards/казначейство, приложить логи Torii к инциденту. |
| Спор по одному имени, несоответствие платежа или эскалация клиента | Sev 2 | Steward + lead support | Собрать доказательства платежа, определить необходимость soft freeze, ответить заявителю в SLA, записать результат в трекере спора. |
| Замечание комплаенс-аудита | Sev 2 | Compliance liaison | Подготовить план ремедиации, разместить мемо в `docs/source/sns/regulatory/`, запланировать последующую сессию совета. |
| Drill или репетиция | Sev 3 | Program PM | Выполнить сценарий из `ops/drill-log.md`, архивировать артефакты, отметить пробелы как задачи roadmap. |

Все инциденты должны создавать `incident/YYYY-MM-DD-sns-<slug>.md` с таблицами
владения, журналами команд и ссылками на доказательства, собранные по этому
плейбуку.

## 9. Ссылки

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR разделы)

Держите этот плейбук актуальным при изменении текста чартеров, CLI поверхностей
или контрактов телеметрии; элементы roadmap, ссылающиеся на
`docs/source/sns/governance_playbook.md`, должны всегда соответствовать
последней редакции.
