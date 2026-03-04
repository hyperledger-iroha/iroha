---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير تكافؤ GA لمنسق SoraFS

Вы можете использовать функцию множественной выборки в SDK, используя функцию множественной выборки, которую вы хотите использовать.
Загрузка полезной нагрузки, фрагмент, поставщик услуг, табло, отображение результатов.
تطبيقات. Воспользуйтесь ремнями безопасности, чтобы их можно было использовать.
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, приложение для SF1 SF1
поставщик услуг телеметрии и оркестратор.

## Обновление для Rust

- **الأمر:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** يشغّل خطة `MultiPeerFixture` مرتين عبر Orchestrator داخل العملية، مع التحقق
  Табло для полезной нагрузки и фрагмента провайдера. Хана
  Создан рабочий набор для рабочего набора (`max_parallel x max_chunk_length`).
- **Обратное сообщение:** Он был в Уилсоне в течение 2 секунд в режиме CI.
- **Зарегистрировано:** в комплектации SF1 для жгута `max_parallel = 3`, в наличии.
  Число <= 196608 дней.

Сообщение от автора:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Использование JavaScript SDK

- **الأمر:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Установлено:** установлен светильник عبر `iroha_js_host::sorafsMultiFetchLocal`, ويقارن.
  полезная нагрузка и квитанции провайдер ولقطات табло عبر تشغيلين متتاليين.
- **Обратный ход:** Уинстон и Нэнси в течение 2 с; يطبع ремни безопасности المقاسة وسقف
  Установите флажок (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Сообщение от автора:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Использование Swift SDK

- **الأمر:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Уведомление:** يشغّل مجموعة التكافؤ المعرّفة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  Для установки приспособления SF1 используется Norito (`sorafsLocalFetch`). ремни безопасности
  Для полезной нагрузки, фрагмента, провайдера, табло, для просмотра результатов.
  Поставщик услуг по удалению телеметрии и использованию Rust/JS.
- **Подробнее:** Ремень безопасности `dist/NoritoBridge.xcframework.zip` находится в центре внимания.
  Версия macOS `dlopen`. Установите xcframework и установите флажок SoraFS, проверьте файл.
  `cargo build -p connect_norito_bridge --release` ويربط مع
  `target/release/libconnect_norito_bridge.dylib`, телефонный звонок для CI.
- **Обратное сообщение:** Он был в Нью-Йорке в 2 секунды в CI; ويطبع ремни безопасности
  Установите флажок (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

Сообщение от автора:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Использование Python

- **الأمر:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Обращение:** Закрыть оболочку для `iroha_python.sorafs.multi_fetch_local` и dataclasses.
  Завершение матча с Сан-Франциско состоится в матче с Манчестер Юнайтед в Нью-Йорке. مستهلكو
  колесо. Поставщик услуг связи с поставщиком услуг `providers.json`, ويحقن
  Телеметрия, поиск полезной нагрузки и фрагмент провайдера.
  табло на платформе Rust/JS/Swift.
- **Отображение:** شغّل `maturin develop --release` (с колесом) на `_crypto` ربط
  `sorafs_multi_fetch_local` для pytest; يتخطى упряжь تلقائيا عندما لا يكون
  الربط متاحا.
- **Обновление:** Время ожидания <= 2 с при запуске Rust; Используйте pytest для проверки
  Обратитесь к поставщикам услуг.Создан для создания программного обеспечения для использования с ремнями безопасности (Rust, Python, JS и Swift)
Он был создан в 2017 году в Колумбии, где полезная нагрузка была передана Бэнкс-Луи. ترقية
строить. Создан `ci/sdk_sorafs_orchestrator.sh` для работы с приложениями (Rust и Python
привязки (JS и Swift) в режиме реального времени Он был отправлен в CI в штате Калифорния.
Загрузите файл `matrix.md` (установка SDK/поддержки/разрешения) إصدار
Он сыграл в фильме "Тренер" в "Старом городе" в Вашингтоне. محليا.