---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70a4c349a726ba56dd222c5b410b1ac0a6cdcec880118ce9fa4a44d1c6851148
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# דוח תאימות GA של Orchestrator SoraFS

מעקב אחר תאימות multi-fetch דטרמיניסטית מתבצע כעת לפי SDK כדי שמהנדסי ה-release יוכלו
לוודא ש-bytes של payload, chunk receipts, provider reports ותוצאות scoreboard נשארים
מיושרים בין המימושים. כל harness צורך את ה-bundle הקנוני מרובה ה-providers תחת
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, שמאגד את תוכנית SF1, provider
metadata, telemetry snapshot ואפשרויות orchestrator.

## קו בסיס Rust

- **פקודה:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **היקף:** מריץ את תוכנית `MultiPeerFixture` פעמיים דרך ה-orchestrator בתוך התהליך,
  ומוודא assembled payload bytes, chunk receipts, provider reports ותוצאות scoreboard.
  האינסטרומנטציה גם עוקבת אחרי שיא מקביליות וגודל working-set אפקטיבי
  (`max_parallel x max_chunk_length`).
- **סף ביצועים:** כל ריצה חייבת להסתיים בתוך 2 s על חומרת CI.
- **תקרת working set:** עם פרופיל SF1 ה-harness כופה `max_parallel = 3`, מה שנותן
  חלון <= 196608 bytes.

דוגמת פלט לוג:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## Harness ל-SDK של JavaScript

- **פקודה:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **היקף:** משחזר את אותו fixture דרך `iroha_js_host::sorafsMultiFetchLocal`, ומשווה
  payloads, receipts, provider reports ו-snapshots של scoreboard בין ריצות עוקבות.
- **סף ביצועים:** כל הרצה חייבת להסתיים בתוך 2 s; ה-harness מדפיס את משך הזמן הנמדד
  ואת תקרת הבייטים השמורים (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

שורת סיכום לדוגמה:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harness ל-SDK של Swift

- **פקודה:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **היקף:** מריץ את חבילת הפאריטי שמוגדרת ב-`IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  ומשחזר את fixture SF1 פעמיים דרך Norito bridge (`sorafsLocalFetch`). ה-harness מאמת
  payload bytes, chunk receipts, provider reports ורשומות scoreboard תוך שימוש באותן
  provider metadata דטרמיניסטיות ו-telemetry snapshots כמו חבילות Rust/JS.
- **אתחול הגשר:** ה-harness פורק את `dist/NoritoBridge.xcframework.zip` לפי צורך וטוען
  את פרוסת macOS באמצעות `dlopen`. כאשר ה-xcframework חסר או אינו כולל bindings של
  SoraFS, הוא עובר ל-`cargo build -p connect_norito_bridge --release` ומקשר מול
  `target/release/libconnect_norito_bridge.dylib`, כך שאין צורך בהגדרה ידנית ב-CI.
- **סף ביצועים:** כל הרצה חייבת להסתיים בתוך 2 s על חומרת CI; ה-harness מדפיס את משך
  הזמן הנמדד ואת תקרת הבייטים השמורים (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

שורת סיכום לדוגמה:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Harness ל-Bindings של Python

- **פקודה:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **היקף:** בוחן את ה-wrapper ברמה גבוהה `iroha_python.sorafs.multi_fetch_local` ואת
  ה-dataclasses הטיפוסיות שלו כדי שה-fixture הקנוני יעבור דרך אותה API שמשתמשים ב-wheel
  צורכים. הבדיקה בונה מחדש provider metadata מתוך `providers.json`, מזריקה את
  telemetry snapshot ומאמתת payload bytes, chunk receipts, provider reports ותוכן
  scoreboard כמו חבילות Rust/JS/Swift.
- **קדם-דרישה:** הרץ `maturin develop --release` (או התקן את ה-wheel) כדי ש-`_crypto`
  יחשוף את binding `sorafs_multi_fetch_local` לפני pytest; ה-harness מדלג אוטומטית
  כשה-binding לא זמין.
- **סף ביצועים:** אותו תקציב <= 2 s כמו חבילת Rust; pytest מתעד את ספירת הבייטים
  המורכבים ואת תקציר השתתפות ה-providers עבור ארטיפקט ה-release.

שער השחרור צריך ללכוד את פלט הסיכום מכל harness (Rust, Python, JS, Swift) כדי שהדוח
המאוחסן יוכל להשוות payload receipts ומדדים בצורה אחידה לפני קידום build. הרץ
`ci/sdk_sorafs_orchestrator.sh` כדי לבצע את כל חבילות ה-parity (Rust, Python bindings,
JS, Swift) במעבר אחד; ארטיפקטים של CI צריכים לצרף את קטע הלוג מה-helper הזה יחד עם
`matrix.md` שנוצר (טבלת SDK/status/duration) לכרטיס השחרור כדי שהבודקים יוכלו לאמת
את מטריצת ה-parity בלי להריץ מחדש מקומית.
