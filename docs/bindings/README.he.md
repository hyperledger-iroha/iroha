---
lang: he
direction: rtl
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# ממשל לקישורי SDK ול-fixtures

<div dir="rtl">

WP1-E במפת הדרכים מציין את “docs/bindings” כמקום הקנוני לשמירת מצב קישורי
הבינדינג בין שפות. מסמך זה מתעד את מלאי הבינדינגים, פקודות הרגנרציה, מגיני
הסטייה ומיקומי הראיות כדי ששערי פריטיות GPU (WP1-E/F/G) ומועצת הקצב הבין־SDK
יקבלו נקודת ייחוס אחת.

## מגינים משותפים
- **פלייבוק קנוני:** `docs/source/norito_binding_regen_playbook.md` מתאר את מדיניות
  הרוטציה, הראיות הצפויות ותהליך ההסלמה עבור Android, Swift, Python ובינדינגים
  עתידיים.
- **פריטיות סכמת Norito:** `scripts/check_norito_bindings_sync.py` (מורץ דרך
  `scripts/check_norito_bindings_sync.sh` ומגודר ב-CI באמצעות
  `ci/check_norito_bindings_sync.sh`) חוסם בניות כאשר ארטיפקטי הסכמה של Rust,
  Java או Python סוטים.
- **שומר קצב:** `scripts/check_fixture_cadence.py` קורא את
  `artifacts/*_fixture_regen_state.json` ומאכף חלונות ג'
  (Android, Python) ו-ד' (Swift) כך שלשערי ה-roadmap יהיו חותמות זמן שניתנות לביקורת.

## מטריצת בינדינגים

| בינדינג | נקודות כניסה | פקודת fixtures/רגנרציה | מגיני סטייה | ראיות |
|---------|--------------|------------------------|-------------|-------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (אופציונלי `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## פרטי בינדינגים

### Android (Java)
ה-SDK של Android נמצא ב-`java/iroha_android/` ומשתמש ב-fixtures Norito קנוניים
שנוצרים על ידי `scripts/android_fixture_regen.sh`. העזר הזה מייצא blobs `.norito`
טריים מה-toolchain של Rust, מעדכן את `artifacts/android_fixture_regen_state.json`
ומקליט מטא-דטה של קצב ש-`scripts/check_fixture_cadence.py` ולוחות הממשל צורכים.
סטייה מזוהה על ידי `scripts/check_android_fixtures.py` (מחובר גם ל-
`ci/check_android_fixtures.sh`) ועל ידי `java/iroha_android/run_tests.sh`, שבודק
בינדינגי JNI, שחזור תור WorkManager ו-fallbacks של StrongBox. הראיות לרוטציה,
הערות כשל ותמלילי ריצות חוזרות נמצאים ב-`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` משקף את אותם payloads של Norito דרך `scripts/swift_fixture_regen.sh`.
הסקריפט מתעד בעלות רוטציה, תווית קצב ומקור (`live` מול `archive`) בתוך
`artifacts/swift_fixture_regen_state.json` ומזין את בודק הקצב. `scripts/swift_fixture_archive.py`
מאפשר למתחזקים לייבא ארכיונים שנוצרו ב-Rust; `scripts/check_swift_fixtures.py` ו-
`ci/check_swift_fixtures.sh` אוכפים פריטיות ברמת בייט ומגבלות גיל SLA, בעוד
`scripts/swift_fixture_regen.sh` תומך ב-`SWIFT_FIXTURE_EVENT_TRIGGER` לרוטציות
ידניות. תהליך ההסלמה, KPI ולוחות בקרה מתועדים ב-
`docs/source/swift_parity_triage.md` ובמסמכי הקצב תחת `docs/source/sdk/swift/`.

### Python
לקוח Python (`python/iroha_python/`) חולק את fixtures של Android. הרצה של
`scripts/python_fixture_regen.sh` מושכת את ה-payloads האחרונים של `.norito`,
מרעננת את `python/iroha_python/tests/fixtures/`, ותפלוט מטא-דטה של קצב אל
`artifacts/python_fixture_regen_state.json` לאחר הרוטציה הראשונה שלאחר ה-roadmap.
`scripts/check_python_fixtures.py` ו-`python/iroha_python/scripts/run_checks.sh`
חוסמים pytest, mypy, ruff ופריטיות fixtures מקומית וב-CI. מסמכי ה-end-to-end
(`docs/source/sdk/python/…`) וה-playbook לרגנרציה מתארים כיצד לתאם רוטציות עם
בעלי Android.

### JavaScript
`javascript/iroha_js/` אינו תלוי בקבצי `.norito` מקומיים, אך WP1-E עוקב אחר ראיות
ה-release כדי שלנתיבי CI של GPU תהיה provenance מלאה. כל release מתעד provenance
באמצעות `npm run release:provenance` (מופעל על ידי
`javascript/iroha_js/scripts/record-release-provenance.mjs`), מייצר וחותם חבילות
SBOM עם `scripts/js_sbom_provenance.sh`, מריץ staging חתום (`scripts/js_signed_staging.sh`),
ומאמת את ארטיפקט הרישום עם
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. המטא-דטה המתקבלת נשמרת
ב-`artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/sbom/`
ו-`artifacts/js/verification/`, ומספקת ראיות דטרמיניסטיות להרצות roadmap JS5/JS6
ו-WP1-F. ה-playbook לפרסום ב-`docs/source/sdk/js/` קושר את כל האוטומציה יחד.

</div>
