---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59d36561f60e28f8d7d5794e953755d3221daf8f0627e0be6a540115b0ad1f05
source_last_modified: "2025-11-20T08:09:37.293192+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: direct-mode-pack
title: Пакет отката прямого режима SoraFS (SNNet-5a)
sidebar_label: Пакет прямого режима
description: Обязательная конфигурация, проверки соответствия и шаги развертывания при работе SoraFS в прямом режиме Torii/QUIC во время перехода SNNet-5a.
---

:::note Канонический источник
:::

Контуры SoraNet остаются транспортом по умолчанию для SoraFS, но пункт дорожной карты **SNNet-5a** требует регулируемого fallback, чтобы операторы могли сохранять детерминированный доступ на чтение, пока развертывание анонимности завершается. Этот пакет фиксирует параметры CLI/SDK, профили конфигурации, проверки соответствия и чек-лист развертывания, необходимые для работы SoraFS в прямом режиме Torii/QUIC без обращения к приватным транспортам.

Fallback применяется к staging и регулируемым прод-средам, пока SNNet-5–SNNet-9 не пройдут свои gates готовности. Храните артефакты ниже вместе с обычными материалами развертывания SoraFS, чтобы операторы могли переключаться между анонимным и прямым режимами по требованию.

## 1. Флаги CLI и SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` отключает планирование реле и принудительно использует транспорты Torii/QUIC. Справка CLI теперь включает `direct-only` как допустимое значение.
- SDK должны устанавливать `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` каждый раз, когда они предлагают переключатель «прямого режима». Сгенерированные биндинги в `iroha::ClientOptions` и `iroha_android` прокидывают тот же enum.
- Gateway-харнесс (`sorafs_fetch`, Python-биндинги) могут разбирать переключатель direct-only через общие Norito JSON-хелперы, чтобы автоматизация получала такое же поведение.

Задокументируйте флаг в runbook'ах для партнеров и подключайте переключатели через `iroha_config`, а не через переменные окружения.

## 2. Профили политики gateway

Используйте Norito JSON, чтобы сохранять детерминированную конфигурацию оркестратора. Пример профиля в `docs/examples/sorafs_direct_mode_policy.json` кодирует:

- `transport_policy: "direct_only"` — отклоняет провайдеров, которые рекламируют только транспорты реле SoraNet.
- `max_providers: 2` — ограничивает прямых пиров наиболее надежными Torii/QUIC эндпойнтами. Настраивайте с учетом региональных требований compliance.
- `telemetry_region: "regulated-eu"` — маркирует метрики, чтобы дашборды и аудиты различали fallback-запуски.
- Консервативные бюджеты повторов (`retry_budget: 2`, `provider_failure_threshold: 3`) для того, чтобы не маскировать неправильно настроенные gateway.

Загружайте JSON через `sorafs_cli fetch --config` (автоматизация) или SDK-биндинги (`config_from_json`) до публикации политики операторам. Сохраняйте вывод scoreboard (`persist_path`) для аудиторских следов.

Параметры принудительного режима на стороне gateway описаны в `docs/examples/sorafs_gateway_direct_mode.toml`. Шаблон отражает вывод `iroha app sorafs gateway direct-mode enable`, отключая проверки envelope/admission, подключая defaults для rate-limit и заполняя таблицу `direct_mode` хостами из плана и digest значениями manifest. Замените placeholder-значения на данные вашего плана rollout перед фиксацией фрагмента в системе управления конфигурациями.

## 3. Набор проверок соответствия

Готовность прямого режима теперь включает покрытие как в оркестраторе, так и в CLI-крейтах:

- `direct_only_policy_rejects_soranet_only_providers` гарантирует, что `TransportPolicy::DirectOnly` быстро падает, когда каждый кандидат advert поддерживает только реле SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` гарантирует использование Torii/QUIC, когда они доступны, и исключение реле SoraNet из сессии.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` парсит `docs/examples/sorafs_direct_mode_policy.json`, чтобы документация оставалась согласованной с утилитами-хелперами.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` запускает `sorafs_cli fetch --transport-policy=direct-only` против мокнутого Torii gateway, предоставляя smoke-тест для регулируемых сред, фиксирующих прямые транспорты.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` оборачивает ту же команду JSON-политикой и сохранением scoreboard для автоматизации rollout.

Запускайте сфокусированный набор тестов перед публикацией обновлений:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Если сборка workspace падает из-за upstream изменений, зафиксируйте блокирующую ошибку в `status.md` и повторите запуск после обновления зависимости.

## 4. Автоматизированные smoke-прогоны

Одна лишь проверка CLI не выявляет регрессии, специфичные для окружения (например, дрейф политик gateway или несоответствие manifests). Отдельный smoke-скрипт находится в `scripts/sorafs_direct_mode_smoke.sh` и оборачивает `sorafs_cli fetch` политикой оркестратора прямого режима, сохранением scoreboard и сбором summary.

Пример использования:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- Скрипт уважает и CLI-флаги, и key=value конфиг-файлы (см. `docs/examples/sorafs_direct_mode_smoke.conf`). Перед запуском заполните digest manifest и записи advert провайдера прод-значениями.
- `--policy` по умолчанию равен `docs/examples/sorafs_direct_mode_policy.json`, но можно передать любой JSON оркестратора, сгенерированный `sorafs_orchestrator::bindings::config_to_json`. CLI принимает политику через `--orchestrator-config=PATH`, обеспечивая воспроизводимые запуски без ручной подгонки флагов.
- Если `sorafs_cli` нет в `PATH`, помощник собирает его из крейта `sorafs_orchestrator` (release профиль), чтобы smoke-прогоны проверяли поставляемую схему прямого режима.
- Выходные данные:
  - Собранный payload (`--output`, по умолчанию `artifacts/sorafs_direct_mode/payload.bin`).
  - Summary fetch (`--summary`, по умолчанию рядом с payload), содержащий регион телеметрии и отчеты провайдеров для доказательной базы rollout.
  - Снимок scoreboard, сохраненный по пути из JSON политики (например, `fetch_state/direct_mode_scoreboard.json`). Архивируйте его вместе с summary в тикетах изменений.
- Автоматизация adoption gate: после завершения fetch скрипт вызывает `cargo xtask sorafs-adoption-check`, используя сохраненные пути scoreboard и summary. Требуемый кворум по умолчанию равен числу провайдеров в командной строке; переопределяйте `--min-providers=<n>` при необходимости большего сэмпла. Adoption-отчеты пишутся рядом с summary (`--adoption-report=<path>` позволяет задать место), а скрипт по умолчанию добавляет `--require-direct-only` (в соответствии с fallback) и `--require-telemetry`, если вы передаете соответствующий флаг. Используйте `XTASK_SORAFS_ADOPTION_FLAGS` для передачи дополнительных аргументов xtask (например, `--allow-single-source` во время одобренного downgrade, чтобы gate и терпел, и enforced fallback). Пропускайте adoption gate с `--skip-adoption-check` только при локальной диагностике; roadmap требует, чтобы каждый регулируемый запуск в прямом режиме включал bundle adoption-отчета.

## 5. Чек-лист развертывания

1. **Заморозка конфигурации:** сохраните JSON-профиль прямого режима в репозитории `iroha_config` и запишите хэш в тикет изменения.
2. **Аудит gateway:** убедитесь, что Torii эндпойнты обеспечивают TLS, capability TLV и аудиторский лог до переключения в прямой режим. Опубликуйте профиль политики gateway для операторов.
3. **Согласование compliance:** поделитесь обновленным playbook с compliance/регуляторными ревьюерами и зафиксируйте одобрения на работу вне анонимного оверлея.
4. **Dry run:** выполните набор compliance-тестов и staging fetch против доверенных Torii провайдеров. Архивируйте scoreboard outputs и CLI summary.
5. **Переключение в прод:** объявите окно изменений, переключите `transport_policy` на `direct_only` (если выбрали `soranet-first`) и следите за дашбордами прямого режима (latency `sorafs_fetch`, счетчики сбоев провайдеров). Документируйте план отката к SoraNet-first после того, как SNNet-4/5/5a/5b/6a/7/8/12/13 перейдут в `roadmap.md:532`.
6. **Пост-изменение:** приложите снимки scoreboard, summary fetch и результаты мониторинга к тикету изменений. Обновите `status.md` с датой вступления в силу и любыми аномалиями.

Держите этот чек-лист рядом с runbook `sorafs_node_ops`, чтобы операторы могли отрепетировать процесс до живого переключения. Когда SNNet-5 перейдет в GA, уберите fallback после подтверждения паритета в продовой телеметрии.

## 6. Требования к доказательствам и adoption gate

Захваты прямого режима по-прежнему должны проходить SF-6c adoption gate. Собирайте scoreboard, summary, manifest envelope и adoption-отчет для каждого запуска, чтобы `cargo xtask sorafs-adoption-check` мог проверить fallback-позицию. Отсутствующие поля приводят к провалу gate, поэтому фиксируйте ожидаемую метадату в тикетах изменений.

- **Метаданные транспорта:** `scoreboard.json` должен указывать `transport_policy="direct_only"` (и переключать `transport_policy_override=true`, когда вы форсируете downgrade). Держите поля анонимной политики заполненными даже при наследовании defaults, чтобы ревьюеры видели отклонения от поэтапного плана анонимности.
- **Счетчики провайдеров:** Сессии gateway-only должны сохранять `provider_count=0` и заполнять `gateway_provider_count=<n>` числом Torii провайдеров. Не правьте JSON вручную: CLI/SDK уже вычисляет счетчики, а adoption gate отклоняет захваты без раздельного учета.
- **Доказательства manifest:** Когда участвуют Torii gateways, передайте подписанный `--gateway-manifest-envelope <path>` (или SDK-эквивалент), чтобы `gateway_manifest_provided` и `gateway_manifest_id`/`gateway_manifest_cid` были записаны в `scoreboard.json`. Убедитесь, что `summary.json` содержит те же `manifest_id`/`manifest_cid`; проверка adoption провалится, если любой файл не содержит пару.
- **Ожидания телеметрии:** Когда телеметрия сопровождает захват, запускайте gate с `--require-telemetry`, чтобы adoption-отчет подтвердил отправку метрик. Air-gapped репетиции могут пропускать флаг, но CI и тикеты изменений должны документировать отсутствие.

Пример:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Приложите `adoption_report.json` вместе со scoreboard, summary, manifest envelope и bundle smoke-логов. Эти артефакты повторяют требования adoption job в CI (`ci/check_sorafs_orchestrator_adoption.sh`) и сохраняют аудитируемость прямых downgrade.
