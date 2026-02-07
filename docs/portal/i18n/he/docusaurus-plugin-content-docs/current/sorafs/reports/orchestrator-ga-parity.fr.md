---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de parité GA SoraFS תזמורת

La parité déterministe multi-fetch est désormais suivie par SDK afin que les
מהנדסי שחרור מנסים לאשר את המטען, קבלות של נתחים,
דוחות ספקים ותוצאות של לוח התוצאות מיושרים
יישום. צ'אק רתום consomme le bundle multi-provider canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, זה ארגון מחדש של תוכנית SF1,
ספק המטא נתונים, תמונת מצב של טלמטריה ואפשרויות מתזמר.

## קו בסיס חלודה

- **פקודה:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **היקף:** בצע את התוכנית `MultiPeerFixture` deux fois באמצעות התזמורת בתהליך,
  en vérifiant les bytes de payload assemblés, קבלות נתחים, דוחות ספק וכו'
  תוצאות לוח התוצאות. חליפת כלי נגינה aussi la concurrence de pointe
  et la taille effective du working-set (`max_parallel × max_chunk_length`).
- **משמר ביצועים:** ביצוע צ'אק דויט פיניר en 2 s sur le hardware CI.
- **תקרת סט עבודה:** Avec le profil SF1, le harness impose `max_parallel = 3`,
  donnant une fenêtre ≤ 196608 בתים.

פלט יומן לדוגמה:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## רתמת JavaScript SDK

- **פקודה:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **היקף:** מתקן Rejoue la même דרך `iroha_js_host::sorafsMultiFetchLocal`,
  מטענים דומים, קבלות, דוחות ספקים ותמונות מצב של לוח התוצאות
  הוצאה להורג ברצף.
- **משמר ביצועים:** צ'אק ביצוע doit finir en 2 s ; le harness imprime la
  durée mesurée et le plafond de bytes réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

שורת סיכום לדוגמה:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתמת Swift SDK

- **פקודה:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **היקף:** ביצוע la suite de parité définie dans `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  rejouant la fixture SF1 deux fois via le bridge Norito (`sorafsLocalFetch`). Le רתמה vérifie
  מטען בתים, קבלות של נתחים, דוחות ספקים וכניסות ללוח התוצאות בשימוש
  ספק מטא-נתונים מפותחים ותמונות מצב של טלמטריה que les suites Rust/JS.
- **רצועת אתחול של גשר:** Le harness décompresse `dist/NoritoBridge.xcframework.zip` à la demande et charge
  le slice macOS דרך `dlopen`. Lorsque l'xcframework est manquant ou n'a pas les bindings SoraFS, il
  bascule sur `cargo build -p connect_norito_bridge --release` et se lie à
  `target/release/libconnect_norito_bridge.dylib`, ללא מדריך להתקנה ב-CI.
- **משמר ביצועים:** ביצוע צ'אקה doit finir en 2 s sur le hardware CI ; le harness imprime la
  durée mesurée et le plafond de bytes réservés (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

שורת סיכום לדוגמה:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתמת Python Bindings- **פקודה:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **היקף:** תרגול למעטפת רמה גבוהה `iroha_python.sorafs.multi_fetch_local` et ses dataclasses typeés
  afin que la fixture canonique passe par la même API que les consommateurs de wheel. למבחן מחדש
  לספק המטא נתונים depuis `providers.json`, הזרקת תמונת מצב של טלמטריה ובדוק את הבתים של המטען,
  קבלות חתיכות, דוחות ספקים ותוכן לוח התוצאות לסדרות Rust/JS/Swift.
- ** דרישה מוקדמת:** Exécutez `maturin develop --release` (ou installez la wheel) pour que `_crypto` expose le binding
  `sorafs_multi_fetch_local` avant d'invoquer pytest ; הרתמה אוטומטית דלג lorsque הכריכה היא בלתי נתפסת.
- **משמר ביצועים:** Même budget ≤ 2 s que la suite Rust ; pytest log le nombre de bytes assemblés
  et le résumé de participation des providers pour l'artefact de release.

Le release gating doit capturer le תקציר פלט de chaque רתמת (Rust, Python, JS, Swift) afin que
le rapport archivé puisse comparer receipts de payload et métriques de manière uniforme avant de
promouvoir un build. Exécutez `ci/sdk_sorafs_orchestrator.sh` pour lancer chaque suite de parité
(Rust, Python bindings, JS, Swift) en une seule passe ; les artefacts CI doivent joindre l'extrait
log de ce helper plus le `matrix.md` généré (טבלה SDK/status/durée) au ticket de release afin que
les reviewers puissent auditer la matrice de parité sans relancer la suite localement.