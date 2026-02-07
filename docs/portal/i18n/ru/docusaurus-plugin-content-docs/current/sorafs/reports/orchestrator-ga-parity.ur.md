---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Оркестратор GA برابری رپورٹ

Детерминированная множественная выборка Использование SDK в различных средах или приложениях تصدیق کر سکیں کہ
байты полезной нагрузки, квитанции о чанках, отчеты поставщика и табло, а также различные реализации и другие варианты реализации.
Использование `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` для канонического пакета с несколькими поставщиками услуг.
План SF1, метаданные поставщика, снимок телеметрии и параметры оркестратора.

## Rust بیس لائن

- **کمانڈ:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **اسکوپ:** `MultiPeerFixture` plan کو in-process оркестратор, کے ذریعے دو بار چلاتا ہے, собранные байты полезной нагрузки,
  квитанции о чанках, отчеты провайдера и табло, а также информация о них Пик одновременной обработки данных
  Рабочий набор سائز (`max_parallel x max_chunk_length`)
- **Защитник производительности:** ہر رن ہارڈویئر پر 2 s کے اندر مکمل ہونا چاہیے۔
- **Потолок рабочего набора:** SF1 имеет встроенный жгут `max_parallel = 3`, размер файла <= 196608 байт. ہے۔

Вот что написано:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Использование JavaScript SDK

- **کمانڈ:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Свет:** Крепление `iroha_js_host::sorafsMultiFetchLocal` может быть использовано в качестве источника питания. درمیان
  полезные нагрузки, квитанции, отчеты поставщика и снимки табло, а также дополнительные сведения.
- **Охранник производительности:** 2 секунды в течение 2 с. жгут ماپی گئی مدت اور потолок зарезервированных байтов
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`)

Ответ на вопрос:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Обвязка Swift SDK

- **کمانڈ:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **اسکوپ:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` — это пакет паритета, который можно использовать.
  Мост Norito (`sorafsLocalFetch`) для крепления приспособления SF1, которое можно использовать для установки. использовать байты полезной нагрузки, квитанции фрагментов,
  отчеты провайдера записи на табло детерминированные метаданные провайдера снимки телеметрии
  Использование пакетов Rust/JS и приложений
- **Bridge bootstrap:** используйте `dist/NoritoBridge.xcframework.zip` для распаковки `dlopen` в `dlopen`.
  Часть macOS В xcframework используются привязки SoraFS, например, привязки SoraFS.
  `cargo build -p connect_norito_bridge --release` — резервный вариант резервного копирования
  `target/release/libconnect_norito_bridge.dylib` کے ساتھ کرتا ہے، اس لئے CI میں دستی سیٹ اپ درکار نہیں۔
- **Защитник производительности:** ہر اجرا CI ہارڈویئر پر 2 s میں مکمل ہونا چاہیے؛ жгут ماپی گئی مدت اور потолок зарезервированных байтов
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`)

Ответ на вопрос:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Привязка Python- **کمانڈ:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **اسکوپ:** Оболочка `iroha_python.sorafs.multi_fetch_local` позволяет использовать типизированные классы данных, которые можно использовать.
  каноническое приспособление اسی API سے گزرے جسے колесо صارفین استعمال کرتے ہیں۔ `providers.json` — метаданные поставщика.
  Внедрение снимков телеметрии, байты полезной нагрузки, квитанции о кусках, отчеты поставщика, табло результатов и пакеты Rust/JS/Swift.
  کی طرح ویریفائی کرتا ہے۔
- **Предварительный запрос:** `maturin develop --release` چلائیں (یا Wheel انسٹال کریں) или `_crypto` `sorafs_multi_fetch_local` привязка ظاہر کرے؛
  Обвязка دستیاب نہ ہو تو خودکار طور پر Skip ہو جاتا ہے۔
- **Защита производительности:** Rust Suite جیسا <= 2 с بجٹ؛ Количество собранных байтов pytest Сводка об участии поставщика и артефакт выпуска
  کے لئے لاگ کرتا ہے۔

Использование средств обвязки (Rust, Python, JS, Swift) и сводных выводов, необходимых для выполнения строить и продвигать
Получите информацию о поступлениях полезной нагрузки и метриках, а также о том, как сравнить данные Пакеты контроля четности (Rust, привязки Python, JS, Swift)
Если вы хотите использовать `ci/sdk_sorafs_orchestrator.sh` چلائیں؛ Артефакты CI могут быть помощником и доступным инструментом.
`matrix.md` (SDK/статус/длительность) Для получения дополнительной информации о рецензентах Матрица паритета для аудита
کر سکیں۔