---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-tuning
title: Rollout и настройка оркестратора
sidebar_label: Настройка оркестратора
description: Практичные значения по умолчанию, советы по настройке и аудит‑чекпоинты для вывода multi‑source оркестратора в GA.
---

:::note Канонический источник
Отражает `docs/source/sorafs/developer/orchestrator_tuning.md`. Держите обе копии синхронизированными, пока набор наследной документации не будет выведен из эксплуатации.
:::

# Руководство по rollout и настройке оркестратора

Это руководство опирается на [справочник конфигурации](orchestrator-config.md) и
[runbook multi-source rollout](multi-source-rollout.md). Оно объясняет,
как настраивать оркестратор для каждой фазы rollout, как интерпретировать
артефакты scoreboard и какие сигналы телеметрии должны быть на месте перед
расширением трафика. Применяйте рекомендации последовательно в CLI, SDK и
автоматизации, чтобы каждый узел следовал одной и той же детерминированной
политике fetch.

## 1. Базовые наборы параметров

Начните с общего шаблона конфигурации и корректируйте небольшой набор ручек по мере
прогресса rollout. Таблица ниже фиксирует рекомендованные значения для наиболее
распространённых фаз; параметры, не указанные в таблице, берутся из
`OrchestratorConfig::default()` и `FetchOptions::default()`.

| Фаза | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Примечания |
|------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|------------|
| **Lab / CI** | `3` | `2` | `2` | `2500` | `300` | Жёсткий лимит задержки и короткая grace‑окно быстро выявляют шумную телеметрию. Держите retries низкими, чтобы раньше выявлять некорректные манифесты. |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | Отражает продакшн‑параметры, оставляя запас для экспериментальных peers. |
| **Canary** | `6` | `3` | `3` | `5000` | `900` | Соответствует значениям по умолчанию; установите `telemetry_region`, чтобы дашборды могли сегментировать canary‑трафик. |
| **General Availability** | `None` (использовать всех eligible) | `4` | `4` | `5000` | `900` | Увеличьте пороги retry/ошибок, чтобы поглощать транзиентные сбои, при этом аудит продолжает обеспечивать детерминизм. |

- `scoreboard.weight_scale` остаётся на дефолтном `10_000`, если только downstream‑система не требует другой целочисленной точности. Увеличение масштаба не меняет порядок провайдеров; оно лишь создаёт более плотное распределение кредитов.
- При переходе между фазами сохраняйте JSON‑bundle и используйте `--scoreboard-out`, чтобы аудит‑трейл фиксировал точный набор параметров.

## 2. Гигиена scoreboard

Scoreboard объединяет требования манифеста, объявления провайдеров и телеметрию.
Перед продвижением:

1. **Проверьте свежесть телеметрии.** Убедитесь, что snapshot’ы, указанные в
   `--telemetry-json`, были сняты в пределах заданного grace‑окна. Записи старше
   `telemetry_grace_secs` завершатся ошибкой `TelemetryStale { last_updated }`.
   Рассматривайте это как жёсткую блокировку и обновите экспорт телеметрии прежде чем продолжать.
2. **Проверьте причины eligibility.** Сохраняйте артефакты через
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Каждая запись
   содержит блок `eligibility` с точной причиной отказа. Не обходите несоответствия
   возможностей или истекшие объявления; исправьте исходный payload.
3. **Проверьте дельты веса.** Сравните поле `normalised_weight` с предыдущим релизом.
   Сдвиги >10 % должны соответствовать намеренным изменениям объявлений или телеметрии
   и должны быть отражены в журнале rollout.
4. **Архивируйте артефакты.** Настройте `scoreboard.persist_path`, чтобы каждый запуск
   публиковал финальный snapshot scoreboard. Приложите артефакт к релизному досье
   вместе с манифестом и телеметрическим bundle.
5. **Фиксируйте доказательства микса провайдеров.** Метаданные `scoreboard.json` _и_
   соответствующий `summary.json` должны содержать `provider_count`,
   `gateway_provider_count` и вычисленный label `provider_mix`, чтобы ревьюеры могли
   доказать, что запуск был `direct-only`, `gateway-only` или `mixed`. Gateway‑снимки
   должны показывать `provider_count=0` и `provider_mix="gateway-only"`, а смешанные
   прогоны — ненулевые значения для обеих источников. `cargo xtask sorafs-adoption-check`
   валидирует эти поля (и падает при несоответствии счётчиков/лейблов), поэтому запускайте
   его вместе с `ci/check_sorafs_orchestrator_adoption.sh` или своим скриптом захвата,
   чтобы получить bundle `adoption_report.json`. Когда задействованы Torii gateways,
   сохраняйте `gateway_manifest_id`/`gateway_manifest_cid` в метаданных scoreboard, чтобы
   adoption‑gate мог сопоставить envelope манифеста с зафиксированным миксом провайдеров.

Подробные определения полей см. в
`crates/sorafs_car/src/scoreboard.rs` и структуре CLI summary, выдаваемой
`sorafs_cli fetch --json-out`.

## Справочник флагов CLI и SDK

`sorafs_cli fetch` (см. `crates/sorafs_car/src/bin/sorafs_cli.rs`) и обёртка
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) используют
одну и ту же поверхность конфигурации оркестратора. Используйте следующие флаги
при сборе evidence или воспроизведении канонических fixtures:

Общая справка по флагам multi-source (держите CLI help и документацию синхронной,
правя только этот файл):

- `--max-peers=<count>` ограничивает число eligible‑провайдеров, проходящих фильтр scoreboard. Оставьте пустым, чтобы стримить со всех eligible‑провайдеров, и ставьте `1` только при осознанном упражнении single‑source fallback. Отражает ручку `maxPeers` в SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` прокидывается в лимит retry на chunk, применяемый `FetchOptions`. Используйте таблицу rollout из руководства для рекомендуемых значений; CLI‑прогоны, собирающие evidence, должны соответствовать дефолтам SDK для сохранения паритета.
- `--telemetry-region=<label>` помечает серии Prometheus `sorafs_orchestrator_*` (и OTLP‑реле) лейблом региона/окружения, чтобы дашборды разделяли lab, staging, canary и GA‑трафик.
- `--telemetry-json=<path>` инжектит snapshot, указанный в scoreboard. Сохраняйте JSON рядом со scoreboard, чтобы аудиторы могли воспроизвести запуск (и чтобы `cargo xtask sorafs-adoption-check --require-telemetry` доказал, какой OTLP‑стрим питал capture).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) включают hook’и bridge‑наблюдателя. При включении оркестратор стримит chunks через локальный Norito/Kaigi proxy, чтобы браузерные клиенты, guard caches и Kaigi‑rooms получали те же receipts, что и Rust.
- `--scoreboard-out=<path>` (опционально с `--scoreboard-now=<unix_secs>`) сохраняет snapshot eligibility для аудиторов. Всегда связывайте сохранённый JSON с артефактами телеметрии и манифеста, указанными в релизном тикете.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` применяют детерминированные корректировки поверх metadata объявлений. Используйте эти флаги только для репетиций; продакшн‑downgrade должен проходить через governance‑артефакты, чтобы каждый узел применял один и тот же policy bundle.
- `--provider-metrics-out` / `--chunk-receipts-out` сохраняют метрики здоровья по провайдерам и receipts по chunks из rollout‑checklist; приложите оба артефакта при подаче evidence по adoption.

Пример (с использованием опубликованного fixture):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK используют ту же конфигурацию через `SorafsGatewayFetchOptions` в Rust‑клиенте
(`crates/iroha/src/client.rs`), JS‑bindings
(`javascript/iroha_js/src/sorafs.js`) и Swift SDK
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Держите эти хелперы
синхронно с дефолтами CLI, чтобы операторы могли копировать политики в автоматизацию
без специальных слоёв трансляции.

## 3. Настройка политики fetch

`FetchOptions` управляет retry, параллелизмом и верификацией. При настройке:

- **Retries:** повышение `per_chunk_retry_limit` выше `4` увеличивает время восстановления,
  но может маскировать проблемы провайдеров. Предпочитайте держать `4` как потолок и
  полагаться на ротацию провайдеров для выявления слабых.
- **Порог отказов:** `provider_failure_threshold` определяет, когда провайдер отключается
  на остаток сессии. Согласуйте это значение с retry‑политикой: порог ниже бюджета
  retry заставляет оркестратор выгнать peer ещё до исчерпания всех retries.
- **Параллелизм:** оставляйте `global_parallel_limit` unset (`None`), если только конкретная
  среда не может насытить объявленные диапазоны. Если задано, убедитесь, что значение
  ≤ сумме потоковых бюджетов провайдеров, чтобы избежать starvation.
- **Тогглы верификации:** `verify_lengths` и `verify_digests` должны оставаться включёнными
  в продакшене. Они гарантируют детерминизм при смешанном составе провайдеров; отключайте
  их только в изолированных fuzzing‑средах.

## 4. Стадирование транспорта и анонимности

Используйте поля `rollout_phase`, `anonymity_policy` и `transport_policy`, чтобы
описать приватностную позу:

- Предпочитайте `rollout_phase="snnet-5"` и позволяйте дефолтной политике анонимности
  следовать этапам SNNet-5. Переопределяйте через `anonymity_policy_override` только
  когда governance выпускает подписанную директиву.
- Держите `transport_policy="soranet-first"` как базу, пока SNNet-4/5/5a/5b/6a/7/8/12/13 находятся в 🈺
  (см. `roadmap.md`). Используйте `transport_policy="direct-only"` только для документированных
  downgrade/комплаенс‑учений и дождитесь ревью PQ‑покрытия перед повышением до
  `transport_policy="soranet-strict"` — этот режим быстро упадёт, если останутся лишь
  классические реле.
- `write_mode="pq-only"` следует применять только когда все write‑пути (SDK, оркестратор,
  governance‑tooling) способны удовлетворить PQ‑требования. Во время rollout держите
  `write_mode="allow-downgrade"`, чтобы аварийные реакции могли использовать прямые
  маршруты, пока телеметрия отмечает downgrade.
- Выбор guards и staging circuits зависит от каталога SoraNet. Передавайте подписанный
  snapshot `relay_directory` и сохраняйте `guard_set` cache, чтобы churn guards оставался
  в согласованном retention‑окне. Отпечаток cache, зафиксированный `sorafs_cli fetch`, входит
  в evidence rollout.

## 5. Хуки деградации и комплаенса

Два подсистемы оркестратора помогают обеспечить политику без ручного вмешательства:

- **Remediation деградаций** (`downgrade_remediation`): отслеживает события
  `handshake_downgrade_total` и, после превышения `threshold` в пределах `window_secs`,
  переводит локальный proxy в `target_mode` (по умолчанию metadata-only). Сохраняйте
  дефолты (`threshold=3`, `window=300`, `cooldown=900`), если только постмортемы не
  показывают другой паттерн. Документируйте любые override в журнале rollout и
  убедитесь, что дашборды отслеживают `sorafs_proxy_downgrade_state`.
- **Политика комплаенса** (`compliance`): carve‑outs по юрисдикциям и манифестам
  проходят через списки opt‑out, управляемые governance. Никогда не встраивайте
  ad‑hoc override в bundle конфигурации; вместо этого запросите подписанное обновление
  `governance/compliance/soranet_opt_outs.json` и разверните сгенерированный JSON.

Для обеих систем сохраняйте итоговый bundle конфигурации и включайте его в evidence
релиза, чтобы аудиторы могли проследить причины дауншифтов.

## 6. Телеметрия и дашборды

Перед расширением rollout убедитесь, что следующие сигналы активны в целевой среде:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  должен быть нулевым после завершения canary.
- `sorafs_orchestrator_retries_total` и
  `sorafs_orchestrator_retry_ratio` — должны стабилизироваться ниже 10 % во время canary
  и оставаться ниже 5 % после GA.
- `sorafs_orchestrator_policy_events_total` — подтверждает активность ожидаемой стадии
  rollout (лейбл `stage`) и фиксирует brownouts через `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — отслеживают доступность PQ‑реле относительно
  ожиданий политики.
- Лог‑таргеты `telemetry::sorafs.fetch.*` — должны поступать в общий лог‑агрегатор с
  сохранёнными запросами для `status=failed`.

Загрузите канонический Grafana‑дашборд из
`dashboards/grafana/sorafs_fetch_observability.json` (экспортируется в портале под
**SoraFS → Fetch Observability**), чтобы селекторы регион/manifest, heatmap retries
по провайдерам, гистограммы задержек chunks и счётчики стагнации соответствовали
тому, что SRE проверяют во время burn‑in. Подключите правила Alertmanager в
`dashboards/alerts/sorafs_fetch_rules.yml` и проверьте синтаксис Prometheus через
`scripts/telemetry/test_sorafs_fetch_alerts.sh` (helper автоматически запускает
`promtool test rules` локально или в Docker). Для alert hand‑off требуется тот же
routing‑block, который печатает скрипт, чтобы операторы могли приложить evidence
к rollout‑тикету.

### Процесс burn-in телеметрии

Пункт roadmap **SF-6e** требует 30‑дневного burn‑in телеметрии перед переключением
multi-source оркестратора на GA‑дефолты. Используйте скрипты репозитория, чтобы
ежедневно собирать воспроизводимый bundle артефактов на протяжении окна:

1. Запустите `ci/check_sorafs_orchestrator_adoption.sh` с установленными переменными
   burn‑in. Пример:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   Helper проигрывает `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   записывает `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` и `adoption_report.json` в
   `artifacts/sorafs_orchestrator/<timestamp>/`, и проверяет минимальное число
   eligible‑провайдеров через `cargo xtask sorafs-adoption-check`.
2. Когда переменные burn‑in присутствуют, скрипт также выпускает `burn_in_note.json`,
   фиксируя label, индекс дня, id манифеста, источник телеметрии и дайджесты артефактов.
   Приложите этот JSON к журналу rollout, чтобы было ясно, какая запись закрыла каждый
   день 30‑дневного окна.
3. Импортируйте обновлённый Grafana‑дашборд (`dashboards/grafana/sorafs_fetch_observability.json`)
   в workspace staging/production, пометьте его burn‑in label и убедитесь, что каждый
   панель отображает выборки для тестируемых manifest/region.
4. Запускайте `scripts/telemetry/test_sorafs_fetch_alerts.sh` (или `promtool test rules …`)
   всякий раз при изменении `dashboards/alerts/sorafs_fetch_rules.yml`, чтобы задокументировать
   соответствие routing alert‑ов метрикам, экспортированным в burn‑in.
5. Архивируйте снимок дашборда, вывод теста alert‑ов и хвост логов по запросам
   `telemetry::sorafs.fetch.*` вместе с артефактами оркестратора, чтобы governance могло
   воспроизвести evidence без обращения к live‑системам.

## 7. Чек‑лист rollout

1. Пересоберите scoreboards в CI с кандидатной конфигурацией и сохраните артефакты под VCS.
2. Запустите детерминированный fetch fixtures в каждом окружении (lab, staging, canary, production)
   и приложите артефакты `--scoreboard-out` и `--json-out` к записи rollout.
3. Проверьте телеметрические дашборды совместно с on‑call инженером, убедившись, что все
   вышеуказанные метрики имеют живые выборки.
4. Зафиксируйте финальный путь конфигурации (обычно через `iroha_config`) и git‑commit
   governance‑реестра, использованного для объявлений и комплаенса.
5. Обновите rollout‑трекер и уведомите команды SDK о новых дефолтах, чтобы клиентские
   интеграции оставались синхронизированными.

Следование этому руководству сохраняет развёртывания оркестратора детерминированными и
аудируемыми, одновременно предоставляя ясные контуры обратной связи для настройки
retry‑бюджетов, ёмкости провайдеров и приватностной позы.
