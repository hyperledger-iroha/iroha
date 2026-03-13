---
lang: he
direction: rtl
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-11-17T18:34:13.669847+00:00"
translation_last_reviewed: 2026-01-01
---

# לוח אימוץ Norito-RPC

> רשומות התכנון הקנוניות נמצאות ב-`docs/source/torii/norito_rpc_adoption_schedule.md`.  
> עותק הפורטל הזה מזקק את ציפיות ההשקה עבור כותבי SDK, מפעילים וסוקרים.

## יעדים

- ליישר כל SDK (Rust CLI, Python, JavaScript, Swift, Android) על גבי תעבורת Norito-RPC הבינארית לפני מתג הייצור AND4.
- לשמור על שערי שלב, חבילות ראיות ו-hooks של טלמטריה דטרמיניסטיים כדי שהממשל יוכל לאמת את הפריסה.
- להקל על לכידת ראיות fixtures ו-canary עם העוזרים המשותפים שה-roadmap NRPC-4 מציין.

## ציר זמן של שלבים

| שלב | חלון | היקף | קריטריוני יציאה |
|-----|------|------|------------------|
| **P0 - פריטריות מעבדה** | Q2 2025 | חבילות smoke של Rust CLI + Python מריצות `/v2/norito-rpc` ב-CI, העוזר JS עובר בדיקות יחידה, ורתמת Android mock מתרגלת שני טרנספורטים. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` ו-`javascript/iroha_js/test/noritoRpcClient.test.js` ירוקים ב-CI; רתמת Android מחוברת ל-`./gradlew test`. |
| **P1 - תצוגה מקדימה של SDK** | Q3 2025 | חבילת fixtures משותפת נבדקת לקוד, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` מקליטה לוגים + JSON תחת `artifacts/norito_rpc/`, ודגלי תעבורת Norito אופציונליים נחשפים בדוגמאות SDK. | מניפסט fixtures חתום, עדכוני README מציגים שימוש opt-in, API תצוגה מקדימה של Swift זמין מאחורי הדגל IOS2. |
| **P2 - Staging / תצוגה מקדימה AND4** | Q1 2026 | מאגרי Torii ב-staging מעדיפים Norito, לקוחות AND4 preview ב-Android וסדרות פריטריות IOS2 ב-Swift משתמשים כברירת מחדל בתעבורה הבינארית, ודשבורד הטלמטריה `dashboards/grafana/torii_norito_rpc_observability.json` מאוכלס. | `docs/source/torii/norito_rpc_stage_reports.md` לוכד את ה-canary, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` עובר, וריפליי של רתמת Android mock לוכד מקרי הצלחה/שגיאה. |
| **P3 - GA בייצור** | Q4 2026 | Norito הופך לתעבורה ברירת מחדל לכל ה-SDKs; JSON נשאר fallback brownout. עבודות release מאחסנות ארטיפקטי פריטריות עם כל תג. | Checklist ה-release אוסף את פלט ה-smoke של Norito עבור Rust/JS/Python/Swift/Android; ספי התראה עבור SLOs של שיעור שגיאות Norito מול JSON נאכפים; `status.md` והערות release מצטטים ראיות GA. |

## תוצרי SDK ו-hooks של CI

- **Rust CLI ורתמת אינטגרציה** - להרחיב את בדיקות ה-smoke של `iroha_cli pipeline` כדי לאלץ את תעבורת Norito ברגע ש-`cargo xtask norito-rpc-verify` זמין. לשמור על הגנות עם `cargo test -p integration_tests -- norito_streaming` (lab) ו-`cargo xtask norito-rpc-verify` (staging/GA), תוך שמירת ארטיפקטים תחת `artifacts/norito_rpc/`.
- **SDK Python** - להגדיר את smoke ה-release (`python/iroha_python/scripts/release_smoke.sh`) כברירת מחדל על Norito RPC, להשאיר את `run_norito_rpc_smoke.sh` ככניסת CI, ולתעד פריטריות ב-`python/iroha_python/README.md`. יעד CI: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **SDK JavaScript** - לייצב את `NoritoRpcClient`, לאפשר לעוזרי governance/query לעבור לברירת מחדל של Norito כאשר `toriiClientConfig.transport.preferred === "norito_rpc"`, ולתעד דוגמאות end-to-end ב-`javascript/iroha_js/recipes/`. CI חייב להריץ `npm test` ואת משימת `npm run test:norito-rpc` המרושתת לפני פרסום; provenance מעלה לוגים של Norito smoke ל-`javascript/iroha_js/artifacts/`.
- **SDK Swift** - לחבר את תעבורת Norito bridge מאחורי דגל IOS2, לשקף את קצב ה-fixtures, ולהבטיח שסוויטת הפריטריות Connect/Norito רצה בליינס Buildkite המוזכרים ב-`docs/source/sdk/swift/index.md`.
- **SDK Android** - לקוחות AND4 preview ורתמת Torii mock מאמצים Norito, עם טלמטריה של retry/backoff מתועדת ב-`docs/source/sdk/android/networking.md`. הרתמה משתפת fixtures עם SDKs אחרים דרך `scripts/run_norito_rpc_fixtures.sh --sdk android`.

## ראיות ואוטומציה

- `scripts/run_norito_rpc_fixtures.sh` עוטף את `cargo xtask norito-rpc-verify`, אוסף stdout/stderr, ומפיק `fixtures.<sdk>.summary.json` כדי שלבעלי SDK יהיה ארטיפקט דטרמיניסטי לצירוף אל `status.md`. השתמשו ב-`--sdk <label>` ו-`--out artifacts/norito_rpc/<stamp>/` כדי לשמור על חבילות CI מסודרות.
- `cargo xtask norito-rpc-verify` אוכף פריטריות hash של סכמה (`fixtures/norito_rpc/schema_hashes.json`) ונכשל אם Torii מחזיר `X-Iroha-Error-Code: schema_mismatch`. צמדו כל כשל בלכידת fallback JSON לצורכי דיבוג.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` ו-`dashboards/grafana/torii_norito_rpc_observability.json` מגדירים את חוזי ההתראה עבור NRPC-2. הריצו את הסקריפט אחרי כל עריכת דשבורד ושמרו את פלט `promtool` בחבילת ה-canary.
- `docs/source/runbooks/torii_norito_rpc_canary.md` מתאר את תרגילי staging וייצור; עדכנו אותו כאשר hashes של fixtures או שערי התראה משתנים.

## צ'ק ליסט לסוקרים

לפני סימון אבן דרך NRPC-4, אשרו:

1. ה-hashes של חבילת ה-fixtures האחרונה תואמים ל-`fixtures/norito_rpc/schema_hashes.json` והארטיפקט של CI תועד תחת `artifacts/norito_rpc/<stamp>/`.
2. README של SDK ודוקס הפורטל מסבירים כיצד לאלץ fallback ל-JSON ומציינים את תעבורת Norito כברירת מחדל.
3. דשבורדי הטלמטריה מציגים פאנלים של שיעור שגיאות dual-stack עם קישורי התראה, ו-dry run של Alertmanager (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) מצורף למעקב.
4. לוח האימוץ כאן תואם את ערך המעקב (`docs/source/torii/norito_rpc_tracker.md`) וה-roadmap (NRPC-4) מציין את אותה חבילת ראיות.

משמעת בלוח הזמנים שומרת על התנהגות cross-SDK צפויה ומאפשרת לממשל לאמת את אימוץ Norito-RPC ללא בקשות מותאמות.
