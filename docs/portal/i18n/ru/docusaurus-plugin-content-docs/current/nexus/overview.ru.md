---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-overview
title: Обзор Sora Nexus
description: Высокоуровневый обзор архитектуры Iroha 3 (Sora Nexus) с указателями на канонические документы монорепозитория.
---

Nexus (Iroha 3) расширяет Iroha 2 за счет multi-lane исполнения, пространств данных под управлением governance и общих инструментов для каждого SDK. Эта страница отражает новый обзор `docs/source/nexus_overview.md` в монорепозитории, чтобы читатели портала быстро поняли, как элементы архитектуры сочетаются.

## Линейки релизов

- **Iroha 2** - самостоятельные развертывания для консорциумов или частных сетей.
- **Iroha 3 / Sora Nexus** - публичная multi-lane сеть, где операторы регистрируют пространства данных (DS) и получают общие инструменты governance, settlement и observability.
- Обе линейки собираются из одного workspace (IVM + toolchain Kotodama), поэтому исправления SDK, обновления ABI и фикстуры Norito остаются переносимыми. Операторы скачивают пакет `iroha3-<version>-<os>.tar.zst`, чтобы присоединиться к Nexus; обратитесь к `docs/source/sora_nexus_operator_onboarding.md` за полноэкранным чек-листом.

## Строительные блоки

| Компонент | Резюме | Ссылки портала |
|-----------|---------|--------------|
| Пространство данных (DS) | Определенная governance область исполнения/хранения, владеющая одним или несколькими lane, объявляющая наборы валидаторов, класс приватности и политику комиссий + DA. | См. [Nexus spec](./nexus-spec) для схемы манифеста. |
| Lane | Детерминированный шард исполнения; выпускает коммитменты, которые упорядочивает глобальное NPoS кольцо. Классы lane включают `default_public`, `public_custom`, `private_permissioned` и `hybrid_confidential`. | [Lane model](./nexus-lane-model) описывает геометрию, префиксы хранения и удержание. |
| План перехода | Placeholder-идентификаторы, фазы маршрутизации и упаковка двойного профиля отслеживают, как одно-лейновые развертывания эволюционируют в Nexus. | [Transition notes](./nexus-transition-notes) документируют каждую фазу миграции. |
| Space Directory | Реестровый контракт, который хранит манифесты + версии DS. Операторы сверяют записи каталога с этим каталогом перед вступлением. | Трекер diff-ов манифестов находится в `docs/source/project_tracker/nexus_config_deltas/`. |
| Каталог lane | Секция конфигурации `[nexus]` сопоставляет IDs lane с алиасами, политиками маршрутизации и порогами DA. `irohad --sora --config … --trace-config` печатает рассчитанный каталог для аудитов. | Используйте `docs/source/sora_nexus_operator_onboarding.md` для CLI walkthrough. |
| Роутер settlement | Оркестратор XOR переводов, соединяющий частные CBDC lane с публичными liquidity lane. | `docs/source/cbdc_lane_playbook.md` описывает политические ручки и телеметрические gate. |
| Телеметрия/SLOs | Дашборды + алерты в `dashboards/grafana/nexus_*.json` фиксируют высоту lane, backlog DA, задержку settlement и глубину governance очереди. | [Telemetry remediation plan](./nexus-telemetry-remediation) описывает дашборды, алерты и аудит-доказательства. |

## Снимок развертывания

| Фаза | Фокус | Критерии выхода |
|-------|-------|---------------|
| N0 - Закрытая бета | Registrar под управлением совета (`.sora`), ручной onboarding операторов, статический каталог lane. | Подписанные DS манифесты + отрепетированные передачи governance. |
| N1 - Публичный запуск | Добавляет суффиксы `.nexus`, аукционы, self-service registrar, проводку XOR settlement. | Тесты синхронизации resolver/gateway, дашборды сверки биллинга, настольные учения по спорам. |
| N2 - Расширение | Вводит `.dao`, reseller API, аналитику, портал споров, scorecards для stewards. | Версионированные compliance артефакты, online policy-jury toolkit, отчеты прозрачности казначейства. |
| Ворота NX-12/13/14 | Compliance engine, телеметрические дашборды и документация должны выйти вместе до партнерских пилотов. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) опубликованы, дашборды подключены, policy engine слит. |

## Ответственность операторов

1. **Гигиена конфигурации** - держите `config/config.toml` синхронизированным с опубликованным каталогом lane и dataspaces; архивируйте вывод `--trace-config` с каждым релизным тикетом.
2. **Отслеживание манифестов** - сверяйте записи каталога с последним пакетом Space Directory перед вступлением или обновлением узлов.
3. **Покрытие телеметрией** - публикуйте `nexus_lanes.json`, `nexus_settlement.json` и связанные SDK дашборды; подключайте алерты к PagerDuty и проводите квартальные обзоры по плану remediation.
4. **Отчетность об инцидентах** - следуйте матрице серьезности в [Nexus operations](./nexus-operations) и подавайте RCA в течение пяти рабочих дней.
5. **Готовность к governance** - участвуйте в голосованиях совета Nexus, влияющих на ваши lane, и раз в квартал репетируйте инструкции rollback (отслеживается через `docs/source/project_tracker/nexus_config_deltas/`).

## См. также

- Канонический обзор: `docs/source/nexus_overview.md`
- Подробная спецификация: [./nexus-spec](./nexus-spec)
- Геометрия lane: [./nexus-lane-model](./nexus-lane-model)
- План перехода: [./nexus-transition-notes](./nexus-transition-notes)
- План remediation телеметрии: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook операций: [./nexus-operations](./nexus-operations)
- Гайд по onboarding операторов: `docs/source/sora_nexus_operator_onboarding.md`
