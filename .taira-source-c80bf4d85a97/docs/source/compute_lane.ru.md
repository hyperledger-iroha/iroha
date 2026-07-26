---
lang: ru
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2026-01-03T18:07:56.917770+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Вычислительный переулок (SSC-1)

Вычислительная линия принимает детерминированные вызовы в стиле HTTP и отображает их на Kotodama.
точки входа и записывает измерения/квитанции для выставления счетов и проверки управления.
Этот RFC замораживает схему манифеста, конверты вызовов/квитанций, ограждения песочницы,
и настройки по умолчанию для первого выпуска.

## Манифест

- Схема: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest`/
  `ComputeRoute`).
- `abi_version` закреплен за `1`; манифесты с другой версией отклоняются
  во время проверки.
- На каждом маршруте указано:
  - И18НИ00000014Х (И18НИ00000015Х, И18НИ00000016Х)
  - `entrypoint` (имя точки входа Kotodama)
  - список разрешенных кодеков (`codecs`)
  - Ограничения TTL/газ/запрос/ответ (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - класс детерминированности/исполнения (`determinism`, `execution_class`)
  - SoraFS входные дескрипторы/дескрипторы модели (`input_limits`, опционально `model`)
  - семейство цен (`price_family`) + профиль ресурса (`resource_profile`)
  - политика аутентификации (`auth`)
- Ограждения песочницы находятся в блоке манифеста `sandbox` и являются общими для всех.
  маршруты (режим/случайность/хранилище и недетерминированное отклонение системных вызовов).

Пример: `fixtures/compute/manifest_compute_payments.json`.

## Звонки, запросы и квитанции

- Схема: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` в
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` создает канонический хеш запроса (заголовки сохраняются).
  в детерминированном `BTreeMap`, а полезная нагрузка передается как `payload_hash`).
- `ComputeCall` фиксирует пространство имен/маршрут, кодек, TTL/газ/ограничение ответа,
  профиль ресурса + семейство цен, аутентификация (`Public` или с привязкой к UAID
  `ComputeAuthn`), детерминизм (`Strict` vs `BestEffort`), класс исполнения
  подсказки (CPU/GPU/TEE), объявленные SoraFS входные байты/куски, необязательный спонсор
  бюджет и канонический конверт запроса. Хэш запроса используется для
  защита от повторного воспроизведения и маршрутизация.
- Маршруты могут включать дополнительные ссылки на модели SoraFS и входные ограничения.
  (встроенные/заглавные блоки); манифест правил песочницы, шлюз, подсказки GPU/TEE.
- `ComputePriceWeights::charge_units` преобразует данные измерений в выставленные счета вычисления.
  единицы посредством деления ячеек на циклы и выходные байты.
- `ComputeOutcome` сообщает `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted` или `InternalError` и дополнительно включает хэши ответов/
  размеры/кодек для аудита.

Примеры:
- Звоните: `fixtures/compute/call_compute_payments.json`
- Квитанция: `fixtures/compute/receipt_compute_payments.json`

## Песочница и профили ресурсов- `ComputeSandboxRules` по умолчанию блокирует режим выполнения на `IvmOnly`,
  начальная детерминированная случайность из хеша запроса, позволяет только чтение SoraFS
  доступ и отклоняет недетерминированные системные вызовы. Подсказки GPU/TEE закрываются
  `allow_gpu_hints`/`allow_tee_hints` для обеспечения детерминированности выполнения.
- `ComputeResourceBudget` устанавливает ограничения на циклы, линейную память и стек для каждого профиля.
  размер, бюджет ввода-вывода и исходящий трафик, а также переключатели для подсказок графического процессора и помощников WASI-lite.
- По умолчанию поставляются два профиля (`cpu-small`, `cpu-balanced`) под
  `defaults::compute::resource_profiles` с детерминированными резервными вариантами.

## Единицы ценообразования и выставления счетов

- Семейства цен (`ComputePriceWeights`) отображают циклы и выходные байты в вычисления.
  единицы; по умолчанию заряжается `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` с
  `unit_label = "cu"`. Семейства имеют ключ `price_family` в манифестах и
  применяется при поступлении.
- Записи измерений содержат `charged_units` плюс необработанный цикл/вход/выход/длительность.
  итоги для сверки. Расходы увеличиваются в зависимости от класса исполнения и
  множители детерминизма (`ComputePriceAmplifiers`) и ограничены
  `compute.economics.max_cu_per_call`; выход зажат
  `compute.economics.max_amplification_ratio` для связанного усиления ответа.
- Бюджеты спонсоров (`ComputeCall::sponsor_budget_cu`) применяются к
  лимиты на звонок/день; единицы, которым выставлен счет, не должны превышать заявленный бюджет спонсора.
- В обновленных ценах управления используются границы классов риска в
  `compute.economics.price_bounds` и базовые семейства, записанные в
  `compute.economics.price_family_baseline`; использовать
  `ComputeEconomics::apply_price_update` для проверки изменений перед обновлением
  активная семейная карта. Обновления конфигурации Torii используют
  `ConfigUpdate::ComputePricing`, и кисо применяет его с теми же ограничениями к
  сохраняйте детерминированный характер изменений управления.

## Конфигурация

Новая вычислительная конфигурация находится в `crates/iroha_config/src/parameters`:

- Пользовательский вид: `Compute` (`user.rs`) с переопределениями окружения:
  - `COMPUTE_ENABLED` (по умолчанию `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Цена/экономика: `compute.economics` фиксирует
  `max_cu_per_call`/`max_amplification_ratio`, разделение гонорара, лимиты спонсоров
  (за вызов и ежедневные CU), базовые уровни цен + классы/границы риска для
  обновления управления и множители класса исполнения (GPU/TEE/best-effort).
- Фактические значения/по умолчанию: `actual.rs` / `defaults.rs::compute` подвергаются анализу.
  Настройки `Compute` (пространства имен, профили, семейства цен, песочница).
- Неверные конфигурации (пустые пространства имен, профиль/семейство по умолчанию отсутствует, ограничение TTL).
  инверсии) во время анализа отображаются как `InvalidComputeConfig`.

## Тесты и приспособления

- Детерминированные помощники (`request_hash`, цены) и фикстуры туда и обратно живут в
  `crates/iroha_data_model/src/compute/mod.rs` (см. `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- Фикстуры JSON живут в `fixtures/compute/` и выполняются моделью данных.
  тесты на покрытие регрессии.

## Ресурсы и бюджеты SLO- Конфигурация `compute.slo.*` предоставляет доступ к ручкам SLO шлюза (очередь в полете).
  глубина, ограничение числа запросов в секунду и целевые значения задержки) в
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. По умолчанию: 32
  в полете, 512 в очереди на маршрут, 200 RPS, p50 25 мс, p95 75 мс, p99 120 мс.
- Запустите облегченную тестовую программу для сбора сводок SLO и запросов/исходящих данных.
  снимок: `cargo run -p xtask --bin Compute_gateway -- Bench [manifest_path]
  [итерации] [параллелизм] [out_dir]` (defaults: `fixtures/compute/manifest_compute_pays.json`,
  128 итераций, 16 параллелизма, результаты под
  `artifacts/compute_gateway/bench_summary.{json,md}`). На скамейке используются
  детерминированные полезные нагрузки (`fixtures/compute/payload_compute_payments.json`) и
  заголовки для каждого запроса, чтобы избежать конфликтов при воспроизведении во время тренировки
  Точки входа `echo`/`uppercase`/`sha3`.

## Фиксаторы четности SDK/CLI

- Канонические фикстуры живут под `fixtures/compute/`: манифест, вызов, полезная нагрузка и
  макет ответа/получения в стиле шлюза. Хэши полезной нагрузки должны соответствовать вызову
  И18НИ00000111X; вспомогательная полезная нагрузка живет в
  `fixtures/compute/payload_compute_payments.json`.
- CLI поставляется с `iroha compute simulate` и `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` в прямом эфире
  `javascript/iroha_js/src/compute.js` с регрессионными тестами под
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` загружает одни и те же фикстуры, проверяет хэши полезной нагрузки,
  и моделирует точки входа с помощью тестов в
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- Все помощники CLI/JS/Swift используют одни и те же приспособления Norito, поэтому SDK могут
  проверять построение запроса и обработку хеша в автономном режиме, не затрагивая
  работающий шлюз.