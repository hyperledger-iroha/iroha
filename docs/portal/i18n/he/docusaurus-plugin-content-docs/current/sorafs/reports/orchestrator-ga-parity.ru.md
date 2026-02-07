---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о паритете GA ל-SoraFS Orchestrator

Детерминированный multi-fetch паритет теперь отслеживается по каждому SDK, чтобы
מהנדסי שחרור могли убедиться, что בתים מטען, קבלות נתח, ספק
לוח התוצאות של דיווחים ו результаты остаются согласованными между реализациями.
Каждый רתמה использует канонический חבילה מרובת ספקים из
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, который включает план SF1,
מטא-נתונים של ספק, תמונת מצב של טלמטריה и опции מתזמר.

## קו בסיס חלודה

- **פקודה:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **היקף:** Запускает план `MultiPeerFixture` дважды через מתזמר בתהליך,
  проверяя собранные בתים של מטען, קבלות של נתחים, דוחות ספקים ופריטים
  לוח התוצאות. Инструментация также отслеживает пиковую конкуренцию и эффективный
  סט עבודה размер (`max_parallel × max_chunk_length`).
- **משמר ביצועים:** Каждый прогон должен завершиться за 2 s על חומרת CI.
- **תקרת סט עבודה:** רתמת Для профиля SF1 применяет `max_parallel = 3`,
  давая окно ≤ 196608 בתים.

פלט יומן לדוגמה:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## רתמת JavaScript SDK

- **פקודה:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **היקף:** Проигрывает ту же מתקן через `iroha_js_host::sorafsMultiFetchLocal`,
  сравнивая מטענים, קבלות, דוחות ספקים ותמונות מצב של לוח התוצאות между
  последовательными запусками.
- **משמר ביצועים:** Каждый запуск должен завершиться за 2 שניות; לרתום печатает
  измеренную длительность и потолок בתים שמורים (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

שורת סיכום לדוגמה:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתמת Swift SDK

- **פקודה:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **היקף:** חבילת זוגיות Запускает из `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  מתקן проигрывая SF1 дважды через Norito גשר (`sorafsLocalFetch`). רתם проверяет
  בייטים של מטען, קבלות של נתחים, דוחות ספקים ולוח תוצאות, используя ту же
  детерминированную מטא-נתונים של ספק וצילומי טלמטריה, חלקים וסוויטות Rust/JS.
- **רצועת אתחול גשר:** רתום распаковывает `dist/NoritoBridge.xcframework.zip` по требованию и
  загружает macOS slice через `dlopen`. Когда xcframework отсутствует или не содержит SoraFS כריכות,
  выполняется fallback עבור `cargo build -p connect_norito_bridge --release` и линковка с
  `target/release/libconnect_norito_bridge.dylib`, ללא ручной настройки в CI.
- **משמר ביצועים:** Каждый запуск должен завершиться за 2 s на CI hardware; לרתום печатает
  измеренную длительность и потолок בתים שמורים (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

שורת סיכום לדוגמה:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתמת Python Bindings- **פקודה:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **היקף:** Проверяет עטיפה ברמה גבוהה `iroha_python.sorafs.multi_fetch_local` и его מודפס
  מחלקות נתונים, чтобы каноническая fixture проходила через ту же API, что вызывают
  גלגל הצרכנים. Тест пересобирает מטא נתונים של ספק из `providers.json`, инжектит
  תמונת מצב של טלמטריה и проверяет בתים של מטען, קבלות של נתחים, דוחות ספקים и
  содержимое לוח התוצאות так же, как suites Rust/JS/Swift.
- **דרישה מוקדמת:** Запустите `maturin develop --release` (אולי גלגל установите), чтобы `_crypto`
  открыл כריכת `sorafs_multi_fetch_local` перед pytest; לרתום авто-скипает,
  когда מחייב недоступен.
- **משמר ביצועים:** Тот же бюджет ≤ 2 שניות, что и у חלודה סוויטה; pytest логирует число
  собранных bytes и סיכום ספקי участия ל- release artefact.

שחרור שער должен захватывать פלט סיכום каждого רתמת (חלודה, Python, JS, Swift), чтобы
архивированный отчет мог сравнивать קבלות מטען и метрики единообразно перед продвижением
לבנות. Запустите `ci/sdk_sorafs_orchestrator.sh`, чтобы выполнить все parity suites
(חלודה, כריכות פייתון, JS, Swift) за один проход; חפצי CI должны приложить קטע יומן
из этого helper и сгенерированный `matrix.md` (таблица SDK/status/duration) к כרטיס שחרור,
чтобы סוקרים могли аудитить מטריצת זוגיות без повторного локального прогона.