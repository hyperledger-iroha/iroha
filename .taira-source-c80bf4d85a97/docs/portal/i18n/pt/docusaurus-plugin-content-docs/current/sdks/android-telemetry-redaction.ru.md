---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f22e82241a07052e62531058ef612cbd99bfe52a0bb0cf53ec6d2e28bd6bf389
source_last_modified: "2025-11-18T05:53:43.294424+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: План редактирования телеметрии Android
sidebar_label: Телеметрия Android
slug: /sdks/android-telemetry
---

:::note Канонический источник
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# План редактирования телеметрии Android (AND7)

## Область применения

Этот документ фиксирует предлагаемую политику редактирования телеметрии и артефакты включения для Android SDK в соответствии с пунктом дорожной карты **AND7**. Он выравнивает мобильную инструментализацию с базовой линией узлов Rust, учитывая гарантии приватности конкретных устройств. Итог служит pre-read для SRE‑ревью по управлению в феврале 2026.

Цели:

- Каталогизировать каждый Android‑сигнал, который доходит до общих наблюдательных backend‑ов (трейсы OpenTelemetry, Norito‑кодированные логи, экспорты метрик).
- Классифицировать поля, отличающиеся от базовой линии Rust, и описать правила редактирования или удержания.
- Описать работу по включению и тестированию, чтобы команды поддержки отвечали детерминированно на алерты по редактированию.

## Инвентарь сигналов (черновик)

Планируемая инструментализация сгруппирована по каналам. Все имена полей следуют схеме телеметрии Android SDK (`org.hyperledger.iroha.android.telemetry.*`). Опциональные поля помечены `?`.

| ID сигнала | Канал | Ключевые поля | Классификация PII/PHI | Редактирование / Ретеншн | Примечания |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Trace span | `authority_hash`, `route`, `status_code`, `latency_ms` | Authority публична; route не содержит секретов | Хэшировать authority (`blake2b_256`) перед экспортом; хранить 7 дней | Отражает Rust `torii.http.request`; хэширование защищает мобильный alias. |
| `android.torii.http.retry` | Событие | `route`, `retry_count`, `error_code`, `backoff_ms` | Нет | Без редактирования; хранить 30 дней | Используется для детерминированных аудит‑retry; поля идентичны Rust. |
| `android.pending_queue.depth` | Gauge метрика | `queue_type`, `depth` | Нет | Без редактирования; хранить 90 дней | Соответствует Rust `pipeline.pending_queue_depth`. |
| `android.keystore.attestation.result` | Событие | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (производный), метаданные устройства | Заменить alias на детерминированную метку, редактировать бренд до enum bucket | Требуется для AND2; узлы Rust не эмитят метаданные устройства. |
| `android.keystore.attestation.failure` | Счётчик | `alias_label`, `failure_reason` | Нет после редактирования alias | Без редактирования; хранить 90 дней | Поддерживает chaos‑drills; `alias_label` derived от хэшированного alias. |
| `android.telemetry.redaction.override` | Событие | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Роль актёра — операционная PII | Экспортировать маскированную категорию роли; хранить 365 дней с аудит‑логом | В Rust отсутствует; операторы оформляют override через поддержку. |
| `android.telemetry.export.status` | Счётчик | `backend`, `status` | Нет | Без редактирования; хранить 30 дней | Паритет с Rust экспортёр‑счётчиками статуса. |
| `android.telemetry.redaction.failure` | Счётчик | `signal_id`, `reason` | Нет | Без редактирования; хранить 30 дней | Нужно для `streaming_privacy_redaction_fail_total` в Rust. |
| `android.telemetry.device_profile` | Gauge метрика | `profile_id`, `sdk_level`, `hardware_tier` | Метаданные устройства | Эмитить грубые buckets (SDK major, hardware tier); хранить 30 дней | Даёт паритетные дашборды без раскрытия OEM‑деталей. |
| `android.telemetry.network_context` | Событие | `network_type`, `roaming` | Carrier может быть PII | Полностью убрать `carrier_name`; прочие поля хранить 7 дней | `ClientConfig.networkContextProvider` даёт sanitised snapshot, чтобы приложения эмитили тип сети + roaming без утечки данных подписчика; дашборды трактуют сигнал как мобильный аналог `peer_host` Rust. |
| `android.telemetry.config.reload` | Событие | `source`, `result`, `duration_ms` | Нет | Без редактирования; хранить 30 дней | Отражает Rust config reload spans. |
| `android.telemetry.chaos.scenario` | Событие | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Профиль устройства bucketised | Как `device_profile`; хранить 30 дней | Логируется во время chaos‑rehearsals для AND7. |
| `android.telemetry.redaction.salt_version` | Gauge метрика | `salt_epoch`, `rotation_id` | Нет | Без редактирования; хранить 365 дней | Отслеживает ротацию Blake2b salt; alert при расхождении epoch с Rust. |
| `android.crash.report.capture` | Событие | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Отпечаток crash + метаданные процесса | Хэшировать `crash_id` общим salt, bucketize watchdog state, убрать stack frames до экспорта; хранить 30 дней | Автоматически включается через `ClientConfig.Builder.enableCrashTelemetryHandler()`; питает дашборды без утечки device‑идентификаторов. |
| `android.crash.report.upload` | Счётчик | `crash_id`, `backend`, `status`, `retry_count` | Отпечаток crash | Использовать хэшированный `crash_id`, эмитить только статус; хранить 30 дней | Эмитить через `ClientConfig.crashTelemetryReporter()` или `CrashTelemetryHandler.recordUpload`, чтобы uploads наследовали Sigstore/OLTP гарантии. |

### Точки интеграции

- `ClientConfig` теперь протаскивает данные телеметрии из manifest через `setTelemetryOptions(...)`/`setTelemetrySink(...)`, автоматически регистрируя `TelemetryObserver`, чтобы хэшированные authority и метрики salt шли без специальных observers. См. `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` и классы под `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- Приложения могут вызвать `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` для регистрации `AndroidNetworkContextProvider`, который запрашивает `ConnectivityManager` во время выполнения и эмитит событие `android.telemetry.network_context` без зависимостей Android на этапе компиляции.
- Юнит‑тесты `TelemetryOptionsTests` и `TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) защищают helpers хэширования и интеграционный hook ClientConfig, чтобы регрессии manifest всплывали сразу.
- Enablement kit/labs теперь ссылаются на конкретные API вместо псевдокода, удерживая документ и runbook в синхроне с поставляемым SDK.

> **Операционная заметка:** лист owner/status находится в `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` и должен обновляться вместе с таблицей на каждом AND7 checkpoint.

## Allowlist паритета и workflow schema diff

Governance требует двойной allowlist, чтобы Android‑экспорты никогда не утекали идентификаторы, которые сервисы Rust намеренно раскрывают. Этот раздел зеркалит runbook (`docs/source/android_runbook.md` §2.3), но держит план AND7 автономным.

| Категория | Android экспортеры | Rust сервисы | Валидатор |
|----------|-------------------|---------------|-----------------|
| Контекст authority/route | Хэшировать `authority`/`alias` через Blake2b-256 и удалять сырые Torii hostnames перед экспортом; эмитить `android.telemetry.redaction.salt_version` для доказательства ротации salt. | Эмитить полные Torii hostnames и peer IDs для корреляции. | Сравнить `android.torii.http.request` и `torii.http.request` в последнем diff под `docs/source/sdk/android/readiness/schema_diffs/`, затем запустить `scripts/telemetry/check_redaction_status.py` для проверки epoch salt. |
| Идентичность устройства/подписанта | Bucketize `hardware_tier`/`device_profile`, хэшировать controller alias и никогда не экспортировать серийники. | Эмитить `peer_id` валидатора, `public_key` контроллера и queue hashes как есть. | Согласовать с `docs/source/sdk/mobile_device_profile_alignment.md`, прогнать alias‑hashing tests в `java/iroha_android/run_tests.sh` и архивировать queue‑inspector outputs во время labs. |
| Метаданные сети | Экспортировать только `network_type` + `roaming`; удалить `carrier_name`. | Сохранять peer hostname/TLS метаданные. | Хранить каждый diff в `readiness/schema_diffs/` и алертить, если виджет Grafana “Network Context” показывает строки carrier. |
| Override/chaos evidence | Эмитить `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` с маскированными ролями. | Эмитить override approvals без маски; без chaos‑span. | Сверить `docs/source/sdk/android/readiness/and7_operator_enablement.md` после drills, чтобы override tokens и chaos artefacts стояли рядом с незамаскированными Rust событиями. |

Workflow:

1. После каждого изменения manifest/exporter выполнить `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` и сохранить JSON в `docs/source/sdk/android/readiness/schema_diffs/`.
2. Проверить diff по таблице выше. Если Android эмитит Rust‑only поле (или наоборот), завести AND7 readiness bug и обновить этот план и runbook.
3. Во время еженедельных ops‑review выполнить `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` и записать epoch salt + timestamp diff в readiness worksheet.
4. Зафиксировать отклонения в `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`, чтобы governance packets отражали решения по паритету.

> **Ссылка на схему:** канонические идентификаторы полей исходят из `android_telemetry_redaction.proto` (материализуется при сборке Android SDK вместе с Norito descriptors). Схема экспонирует `authority_hash`, `alias_label`, `attestation_digest`, `device_brand_bucket` и `actor_role_masked`, используемые SDK и экспортёрами телеметрии.

`authority_hash` — фиксированный 32‑байтовый digest authority Torii из proto. `attestation_digest` фиксирует отпечаток канонического attestation statement, а `device_brand_bucket` маппит сырую Android‑строку бренда в утверждённый enum (`generic`, `oem`, `enterprise`). `actor_role_masked` переносит категорию роли (`support`, `sre`, `audit`) вместо сырого идентификатора пользователя.

### Выравнивание экспорта crash‑телеметрии

Crash‑телеметрия теперь использует те же OpenTelemetry экспортеры и provenance pipeline, что и Torii‑сигналы, закрывая governance follow‑up о дублирующих экспортёрах. Crash handler формирует событие `android.crash.report.capture` с хэшированным `crash_id` (Blake2b-256 с salt редактирования, отслеживаемым через `android.telemetry.redaction.salt_version`), buckets состояния процесса и sanitised ANR watchdog metadata. Stack traces остаются на устройстве и лишь суммируются в `has_native_trace` и `anr_watchdog_bucket` перед экспортом, поэтому PII или OEM‑строки не покидают устройство.

Загрузка crash создаёт счётчик `android.crash.report.upload`, позволяя SRE проверять надёжность backend без раскрытия пользователя или стека. Поскольку оба сигнала используют экспортёр Torii, они наследуют Sigstore signing, политику ретенции и alerting hooks, уже определённые для AND7. Runbook поддержки может сопоставлять хэшированный crash id между Android и Rust evidence bundles без отдельного crash‑pipeline.

Включайте handler через `ClientConfig.Builder.enableCrashTelemetryHandler()` после настройки telemetry options/sinks; crash upload bridges могут использовать `ClientConfig.crashTelemetryReporter()` (или `CrashTelemetryHandler.recordUpload`), чтобы эмитить backend outcomes в том же подписанном pipeline.

## Политические отличия от базовой линии Rust

Различия политик телеметрии Android и Rust с мерами смягчения.

| Категория | Базовая линия Rust | Политика Android | Смягчение / Валидация |
|----------|---------------|----------------|-------------------------|
| Authority/peer identifiers | Authority строки в открытом виде | `authority_hash` (Blake2b-256, ротируемый salt) | Общий salt публикуется через `iroha_config.telemetry.redaction_salt`; parity test обеспечивает обратимое сопоставление для поддержки. |
| Host/Network metadata | Hostnames/IPs узлов экспортируются | Только тип сети + roaming | Dashboards здоровья сети обновлены на категории доступности вместо hostnames. |
| Device characteristics | N/A (server‑side) | Bucket‑профиль (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | Chaos rehearsals проверяют mapping; runbook поддержки описывает эскалацию при необходимости деталей. |
| Redaction overrides | Не поддерживаются | Ручной override token хранится в Norito ledger (`actor_role_masked`, `reason`) | Overrides требуют подписанного запроса; audit log хранится 1 год. |
| Attestation traces | Серверная attestation через SRE | SDK эмитит sanitized attestation summary | Сверять attestation hashes с Rust‑валидатором; хэшированный alias предотвращает утечки. |

Проверочный список:

- Юнит‑тесты редактирования для каждого сигнала проверяют хэширование/маскирование до экспорта.
- Инструмент schema diff (общий с Rust) запускается ночью для подтверждения паритета полей.
- Chaos rehearsal script проверяет workflow override и подтверждает аудит‑лог.

## Задачи реализации (до SRE governance)

1. **Подтверждение инвентаря** — Сверить таблицу с фактическими hook‑ами Android SDK и Norito schema definitions. Owners: Android Observability TL, LLM.
2. **Telemetry schema diff** — Запустить общий diff‑инструмент против Rust метрик для артефактов паритета к SRE review. Owner: SRE privacy lead.
3. **Runbook draft (выполнено 2026-02-03)** — `docs/source/android_runbook.md` теперь описывает end‑to‑end override workflow (Section 3) и расширенную матрицу эскалации/ролей (Section 3.1), связывая CLI helpers, incident evidence и chaos scripts с политикой управления. Owners: LLM + Docs/Support.
4. **Enablement content** — Подготовить briefing‑слайды, lab‑инструкции и вопросы проверки знаний для сессии февраля 2026. Owners: Docs/Support Manager, SRE enablement team.

## Enablement workflow и runbook hooks

### 1. Локальное smoke + CI покрытие

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` поднимает Torii sandbox, воспроизводит канонический multi‑source SoraFS fixture (через `ci/check_sorafs_orchestrator_adoption.sh`) и генерирует synthetic Android telemetry.
  - Генерация трафика выполняется `scripts/telemetry/generate_android_load.py`, который сохраняет request/response transcript в `artifacts/android/telemetry/load-generator.log` и учитывает headers, path overrides или dry‑run.
  - Helper копирует SoraFS scoreboard/summary в `${WORKDIR}/sorafs/`, чтобы AND7 rehearsals доказывали multi‑source паритет до подключения мобильных клиентов.
- CI использует те же инструменты: `ci/check_android_dashboard_parity.sh` запускает `scripts/telemetry/compare_dashboards.py` против `dashboards/grafana/android_telemetry_overview.json`, Rust‑референсного dashboard и allowlist‑файла `dashboards/data/android_rust_dashboard_allowances.json`, формируя подписанный snapshot `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- Chaos rehearsals следуют `docs/source/sdk/android/telemetry_chaos_checklist.md`; sample‑env script плюс dashboard parity check формируют evidence‑bundle “ready” для AND7 burn‑in audit.

### 2. Выдача override и audit trail

- `scripts/android_override_tool.py` — каноническая CLI для выдачи/отзыва override. `apply` принимает подписанный запрос, эмитит manifest bundle (`telemetry_redaction_override.to` по умолчанию) и добавляет строку хэшированного токена в `docs/source/sdk/android/telemetry_override_log.md`. `revoke` проставляет timestamp отзыва в этой строке, а `digest` пишет sanitised JSON snapshot для governance.
- CLI отказывается менять audit log без заголовка Markdown‑таблицы, соответствуя требованию compliance из `docs/source/android_support_playbook.md`. Юнит‑покрытие в `scripts/tests/test_android_override_tool_cli.py` защищает парсер таблицы, manifest emitters и обработку ошибок.
- Операторы прикладывают сгенерированный manifest, обновлённый log excerpt **и** digest JSON в `docs/source/sdk/android/readiness/override_logs/` при каждом override; лог хранит 365 дней истории согласно этому плану.

### 3. Захват evidence и ретеншн

- Каждый rehearsal или инцидент создаёт структурированный bundle в `artifacts/android/telemetry/`:
  - Transcript генератора нагрузки и агрегированные счётчики из `generate_android_load.py`.
  - Dashboard diff (`android_vs_rust-<stamp>.json`) и allowlist hash, эмитированные `ci/check_android_dashboard_parity.sh`.
  - Override log delta (если override был), соответствующий manifest и обновлённый digest JSON.
- SRE burn‑in report ссылается на эти артефакты плюс SoraFS scoreboard, скопированный `android_sample_env.sh`, формируя детерминированную цепочку hashes → dashboards → override status.

## Выравнивание device profile между SDK

Dashboards переводят Android `hardware_tier` в канонический `mobile_profile_class`, определённый в `docs/source/sdk/mobile_device_profile_alignment.md`, чтобы AND7 и IOS7 сравнивали одни и те же когорты:

- `lab` — эмитится как `hardware_tier = emulator`, совпадает с `device_profile_bucket = simulator` в Swift.
- `consumer` — эмитится как `hardware_tier = consumer` (с суффиксом SDK major) и группируется с `iphone_small`/`iphone_large`/`ipad` у Swift.
- `enterprise` — эмитится как `hardware_tier = enterprise`, выравнивается с bucket `mac_catalyst` в Swift и будущими managed iOS desktop runtimes.

Любой новый tier должен быть добавлен в документ выравнивания и в schema diff артефакты до использования в dashboards.

## Governance и распространение

- **Pre-read пакет** — Этот документ и appendices (schema diff, runbook diff, outline readiness deck) будут отправлены в SRE governance mailing list не позже **2026-02-05**.
- **Feedback loop** — Комментарии из governance попадут в JIRA epic `AND7`; блокеры отражаются в `status.md` и еженедельных Android stand‑up notes.
- **Публикация** — После утверждения краткая политика будет связана из `docs/source/android_support_playbook.md` и упомянута в общей FAQ `docs/source/telemetry.md`.

## Замечания по аудиту и соответствию

- Политика соблюдает GDPR/CCPA, удаляя данные мобильного абонента до экспорта; salt authority hash ротируется ежеквартально и хранится в общем secrets vault.
- Enablement artefacts и обновления runbook фиксируются в compliance registry.
- Квартальные ревью подтверждают, что overrides остаются closed‑loop (без устаревшего доступа).

## Итог governance (2026-02-12)

SRE governance‑сессия **2026-02-12** утвердила политику редактирования Android без изменений. Ключевые решения (см. `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **Принятие политики.** Хэширование authority, bucket‑профиль и исключение имён carriers были ратифицированы. Отслеживание salt rotation через `android.telemetry.redaction.salt_version` становится квартальным аудит‑пунктом.
- **План валидации.** Одобрены unit/integration coverage, ночные schema diffs и квартальные chaos rehearsals. Действие: публиковать dashboard parity report после каждого rehearsal.
- **Override governance.** Norito‑записанные override tokens одобрены с окном ретенции 365 дней. Support engineering отвечает за review digest override log на ежемесячных ops sync.

## Статус follow‑up

1. **Выравнивание device‑profile (срок 2026-03-01).** ✅ Выполнено — общая карта в `docs/source/sdk/mobile_device_profile_alignment.md` определяет, как значения `hardware_tier` Android маппятся в канонический `mobile_profile_class` для dashboards и schema diff tooling.

## Предстоящий SRE governance brief (Q2 2026)

Пункт **AND7** требует, чтобы следующая SRE‑сессия получила краткий pre‑read по Android redaction. Используйте этот раздел как живой brief и обновляйте перед каждым council‑митингом.

### Checklist подготовки

1. **Evidence bundle** — экспортируйте последний schema diff, скриншоты dashboards и digest override log (см. матрицу ниже) и положите в датированную папку (например `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) до отправки invite.
2. **Drill summary** — приложите самый свежий chaos rehearsal log и snapshot метрики `android.telemetry.redaction.failure`; убедитесь, что Alertmanager annotations ссылаются на тот же timestamp.
3. **Override audit** — подтвердите, что все активные overrides записаны в Norito registry и отражены в meeting deck. Укажите сроки истечения и соответствующие incident IDs.
4. **Agenda note** — предупредите SRE chair за 48 часов до встречи ссылкой на brief и выделите решения, которые нужны (новые сигналы, изменения ретенции или политика overrides).

### Матрица evidence

| Артефакт | Локация | Owner | Примечания |
|----------|----------|-------|-------|
| Schema diff vs Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | Должен быть создан <72 ч до встречи. |
| Скриншоты dashboard diff | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | Включить `sorafs.fetch.*`, `android.telemetry.*` и snapshots Alertmanager. |
| Override digest | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | Запустить `scripts/android_override_tool.sh digest` (см. README директории) для последнего `telemetry_override_log.md`; токены остаются хэшированными. |
| Chaos rehearsal log | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | Приложить KPI summary (stall count, retry ratio, override usage). |

### Открытые вопросы для совета

- Нужно ли сокращать окно ретенции override с 365 дней теперь, когда digest автоматизирован?
- Должен ли `android.telemetry.device_profile` принять новые общие labels `mobile_profile_class` в следующем релизе или подождать Swift/JS SDK?
- Требуется ли дополнительное руководство по региональной резидентности данных после появления Torii Norito-RPC событий на Android (follow‑up NRPC-3)?

### Процедура Telemetry Schema Diff

Запускайте schema diff минимум раз на release candidate (и при любых изменениях Android instrumentation), чтобы SRE council получал свежие parity artefacts вместе с dashboard diff:

1. Экспортируйте Android и Rust telemetry schemas для сравнения. Для CI конфиги находятся в `configs/android_telemetry.json` и `configs/rust_telemetry.json`.
2. Выполните `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`.
   - Альтернатива: передайте commits (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) для загрузки configs из git; скрипт фиксирует hashes внутри artefact.
3. Приложите JSON к readiness bundle и сослайтесь на него из `status.md` + `docs/source/telemetry.md`. Diff подчёркивает добавленные/удалённые поля и retention deltas для аудита без повторного запуска инструмента.
4. Когда diff показывает допустимое расхождение (например Android‑only override signals), обновите allowlist‑файл, используемый `ci/check_android_dashboard_parity.sh`, и добавьте объяснение в README директории diff.

> **Правила архива:** держите последние пять diff‑ов в `docs/source/sdk/android/readiness/schema_diffs/`, а более старые снимки переносите в `artifacts/android/telemetry/schema_diffs/`, чтобы governance reviewers всегда видели актуальные данные.
