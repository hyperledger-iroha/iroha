---
lang: fr
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/orchestrator-tuning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 066131598223253073cf980da8ae72b87b9ccc7bbeaa7e678d3c92819acc4e2f
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: orchestrator-tuning
lang: he
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
משקף את `docs/source/sorafs/developer/orchestrator_tuning.md`. שמרו על סנכרון שתי הגרסאות עד שהמסמכים המורשים יוחלפו לחלוטין.
:::

# מדריך רולאאוט וכיוונון לאורקסטרטור

מדריך זה נשען על [מדריך התצורה](orchestrator-config.md) ועל
[ראנבוק ה-rollout רב-המקורות](multi-source-rollout.md). הוא מסביר
כיצד לכוון את האורקסטרטור בכל שלב rollout, כיצד לפרש את ארטיפקטי ה-scoreboard,
ואילו אותות טלמטריה חייבים להיות זמינים לפני הרחבת תעבורה. יישמו את ההמלצות
באופן עקבי ב-CLI, ב-SDKs ובאוטומציה כדי שכל צומת תפעל לפי מדיניות fetch דטרמיניסטית אחת.

## 1. סטים בסיסיים של פרמטרים

התחילו מתבנית תצורה משותפת וכוונו מספר קטן של כפתורים ככל שה‑rollout מתקדם. הטבלה
מטה מרכזת ערכים מומלצים לשלבים הנפוצים; ערכים שלא מצוינים חוזרים לברירות המחדל של
`OrchestratorConfig::default()` ו-`FetchOptions::default()`.

| שלב | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | הערות |
|------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|-------|
| **Lab / CI** | `3` | `2` | `2` | `2500` | `300` | תקרת השהיה הדוקה וחלון חסד קצר חושפים טלמטריה רועשת מהר. שמרו על retries נמוכים כדי לחשוף מניפסטים לא תקינים מוקדם יותר. |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | משקף את ברירות המחדל של הייצור ומשאיר מרווח לנקודות בדיקה ניסיוניות. |
| **Canary** | `6` | `3` | `3` | `5000` | `900` | תואם לברירות המחדל; הגדירו `telemetry_region` כדי שהדשבורדים יוכלו לפצל תעבורת קנרי. |
| **זמינות כללית (GA)** | `None` (להשתמש בכל ה‑eligible) | `4` | `4` | `5000` | `900` | העלו את ספי ה‑retry והכשל כדי לספוג תקלות רגעיות בזמן שהביקורת ממשיכה לאכוף דטרמיניזם. |

- `scoreboard.weight_scale` נשאר כברירת המחדל `10_000` אלא אם מערכת downstream דורשת רזולוציית שלמים אחרת. הגדלת הסקייל לא משנה את סדר הספקים; היא רק מפיקה חלוקת קרדיטים צפופה יותר.
- בעת מעבר בין שלבים, שימרו את חבילת ה‑JSON והשתמשו ב‑`--scoreboard-out` כדי שמסלול הביקורת יתעד את סט הפרמטרים המדויק.

## 2. היגיינת scoreboard

ה‑scoreboard משלב דרישות מניפסט, פרסומי ספקים וטלמטריה. לפני שמתקדמים:

1. **אמתו רעננות טלמטריה.** ודאו שה‑snapshots שמופנים דרך `--telemetry-json` נאספו
   במסגרת חלון החסד. רשומות ישנות מ‑`telemetry_grace_secs` נכשלות עם
   `TelemetryStale { last_updated }`. התייחסו לכך כעצירה קשיחה ורעננו את יצוא
   הטלמטריה לפני המשך.
2. **בדקו סיבות זכאות.** שימרו ארטיפקטים באמצעות
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. כל רשומה כוללת
   בלוק `eligibility` עם סיבת הכשל המדויקת. אל תעקפו אי‑התאמות יכולת או פרסומים שפגו;
   תקנו את ה‑payload המקורי.
3. **בדקו שינויים במשקל.** השוו את `normalised_weight` לריליס הקודם. סטיות של יותר מ‑10% חייבות
   להתכתב עם שינויים מכוונים בפרסומים או טלמטריה ולהירשם ביומן ה‑rollout.
4. **ארכבו ארטיפקטים.** הגדירו `scoreboard.persist_path` כך שכל ריצה תפלוט את snapshot הסופי.
   צרפו את הארטיפקט לרשומת ה‑release יחד עם המניפסט וחבילת הטלמטריה.
5. **רשמו הוכחות למיקס ספקים.** המטא‑דאטה של `scoreboard.json` _וגם_ ה‑`summary.json`
   המתאים חייבים לחשוף `provider_count`, `gateway_provider_count` ואת התווית
   `provider_mix` כדי לאפשר אימות שהריצה הייתה `direct-only`, `gateway-only` או `mixed`.
   לכידות gateway מדווחות `provider_count=0` ו‑`provider_mix="gateway-only"`, בעוד שריצות
   משולבות דורשות ספירות לא‑אפסיות לשני המקורות. `cargo xtask sorafs-adoption-check`
   מאכף שדות אלה (ונכשל כאשר הספירות/תוויות לא תואמות), לכן הריצו אותו תמיד יחד עם
   `ci/check_sorafs_orchestrator_adoption.sh` או עם סקריפט הלכידה שלכם כדי להפיק
   `adoption_report.json`. כאשר מעורבים Torii gateways, שמרו `gateway_manifest_id`/
   `gateway_manifest_cid` במטא‑דאטה של ה‑scoreboard כדי ששער האימוץ יוכל לקשור את
   מעטפת המניפסט עם המיקס שתועד.

להגדרות שדות מפורטות, ראו
`crates/sorafs_car/src/scoreboard.rs` ואת מבנה תקציר ה‑CLI שמופיע דרך
`sorafs_cli fetch --json-out`.

## רפרנס דגלי CLI ו‑SDK

`sorafs_cli fetch` (ראו `crates/sorafs_car/src/bin/sorafs_cli.rs`) והעטיפה
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) משתמשים באותו
משטח הגדרה של האורקסטרטור. השתמשו בדגלים הבאים בעת לכידת evidence או שחזור
ה‑fixtures הקנוניים:

רפרנס דגלים משותף לרב‑מקורות (שמרו על סנכרון עזרת ה‑CLI והמסמכים בעריכת קובץ זה בלבד):

- `--max-peers=<count>` מגביל את מספר הספקים ה‑eligible שעוברים את מסנן ה‑scoreboard. השאירו ריק כדי להזרים מכל הספקים ה‑eligible, והגדירו `1` רק כאשר מבצעים בכוונה fallback חד‑מקור. משקף את `maxPeers` ב‑SDKs (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` מעביר את מגבלת retry לכל chunk שמיושמת על ידי `FetchOptions`. השתמשו בטבלת ה‑rollout במדריך הכיוונון לערכים מומלצים; ריצות CLI שאוספות evidence חייבות להתאים לברירות המחדל של ה‑SDK כדי לשמור על פריטי.
- `--telemetry-region=<label>` מתייג סדרות Prometheus `sorafs_orchestrator_*` (וריליי OTLP) בלייבל אזור/סביבה כדי שהדשבורדים יפרידו בין lab, staging, canary ו‑GA.
- `--telemetry-json=<path>` מזריק את ה‑snapshot שמופנה ב‑scoreboard. שמרו את ה‑JSON ליד ה‑scoreboard כדי שהמבקרים יוכלו לשחזר את הריצה (וכדי ש‑`cargo xtask sorafs-adoption-check --require-telemetry` יוכיח איזה זרם OTLP הזין את הלכידה).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) מפעיל hook‑ים של bridge observer. כאשר מוגדר, האורקסטרטור מזרים chunks דרך Norito/Kaigi proxy מקומי כדי שלקוחות דפדפן, guard caches וחדרי Kaigi יקבלו את אותם receipts שיוצאים מ‑Rust.
- `--scoreboard-out=<path>` (אופציונלי עם `--scoreboard-now=<unix_secs>`) שומר snapshot זכאות למבקרים. תמיד שייכו את ה‑JSON השמור לארטיפקטי הטלמטריה והמניפסט שמופיעים בטיקט ה‑release.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` מחילים התאמות דטרמיניסטיות מעל מטא‑דאטה של פרסומים. השתמשו בדגלים אלה רק לחזרות; downgrades בייצור חייבים לעבור דרך ארטיפקטי ממשל כדי שכל צומת תחיל את אותו bundle מדיניות.
- `--provider-metrics-out` / `--chunk-receipts-out` שומרים מדדי בריאות לפי ספק ו‑chunk receipts שמוזכרים ברשימת ה‑rollout; צרפו את שניהם בעת הגשת evidence.

דוגמה (עם ה‑fixture שפורסם):

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

ה‑SDKs צורכים את אותה תצורה דרך `SorafsGatewayFetchOptions` ב‑Rust client
(`crates/iroha/src/client.rs`), ב‑JS bindings
(`javascript/iroha_js/src/sorafs.js`) וב‑Swift SDK
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). שמרו על סנכרון העזרים האלה
עם ברירות המחדל של ה‑CLI כדי שהמפעילים יוכלו להעתיק מדיניות בין אוטומציה ללא שכבות
תרגום מותאמות.

## 3. כיוונון מדיניות fetch

`FetchOptions` שולט ב‑retry, במקביליות ובאימות. בעת כיוונון:

- **Retries:** העלאת `per_chunk_retry_limit` מעל `4` מגדילה את זמן השחזור אבל עלולה להסתיר תקלות ספק. העדיפו להשאיר את `4` כתקרה ולהסתמך על סבב ספקים כדי להבליט ביצועים חלשים.
- **סף כשל:** `provider_failure_threshold` קובע מתי ספק מנוטרל להמשך הסשן. התאימו את הערך למדיניות ה‑retry: סף נמוך מתקציב retry מאלץ את האורקסטרטור להוציא peer לפני שממצים את כל ה‑retries.
- **מקביליות:** השאירו `global_parallel_limit` לא מוגדר (`None`) אלא אם סביבה ספציפית לא מצליחה למצות את הטווחים המוצהרים. כאשר מוגדר, ודאו שהערך ≤ סכום תקציבי הזרמים של הספקים כדי להימנע מ‑starvation.
- **מתגי אימות:** `verify_lengths` ו‑`verify_digests` חייבים להישאר פעילים בייצור. הם מבטיחים דטרמיניזם כשיש צי ספקים מעורב; כבו אותם רק בסביבות fuzzing מבודדות.

## 4. שלבי תעבורה ואנונימיות

השתמשו בשדות `rollout_phase`, `anonymity_policy`, ו‑`transport_policy` כדי לייצג את מצב הפרטיות:

- העדיפו `rollout_phase="snnet-5"` ותנו למדיניות האנונימיות ברירת המחדל להתיישר עם אבני הדרך של SNNet-5. עקפו עם `anonymity_policy_override` רק כאשר ממשל מפרסם הנחיה חתומה.
- שמרו על `transport_policy="soranet-first"` כברירת מחדל בזמן ש‑SNNet-4/5/5a/5b/6a/7/8/12/13 הם 🈺
  (ראו `roadmap.md`). השתמשו ב‑`transport_policy="direct-only"` רק עבור downgrades מתועדים או תרגילי תאימות, והמתינו לבדיקת כיסוי PQ לפני קידום ל‑`transport_policy="soranet-strict"`—רמה זו תכשל מהר אם רק רילייז קלאסיים נשארו.
- `write_mode="pq-only"` צריך להיאכף רק כאשר כל מסלולי הכתיבה (SDK, אורקסטרטור, כלי ממשל) מסוגלים לעמוד בדרישות PQ. במהלך rollouts שמרו על `write_mode="allow-downgrade"` כדי שתגובות חירום יוכלו להסתמך על מסלולים ישירים בזמן שהטלמטריה מסמנת את ה‑downgrade.
- בחירת guards ו‑circuit staging נשענים על ספריית SoraNet. ספקו snapshot חתום של `relay_directory` ושמרו cache של `guard_set` כדי שתנודת guards תישאר בתוך חלון השימור שהוסכם. טביעת האצבע של cache שנרשמת ב‑`sorafs_cli fetch` היא חלק מראיות ה‑rollout.

## 5. Hooks לדאונגרייד ותאימות

שני תתי‑מערכות באורקסטרטור עוזרות לאכוף מדיניות ללא התערבות ידנית:

- **Remediation לדאונגרייד** (`downgrade_remediation`): מנטר אירועי `handshake_downgrade_total`, ולאחר חריגה של `threshold` בתוך `window_secs` כופה את ה‑proxy המקומי ל‑`target_mode` (ברירת מחדל metadata-only). שמרו על ברירות המחדל (`threshold=3`, `window=300`, `cooldown=900`) אלא אם תחקירי אירוע מצביעים על דפוס אחר. תעדו כל override ביומן ה‑rollout וודאו שהדשבורדים עוקבים אחרי `sorafs_proxy_downgrade_state`.
- **מדיניות תאימות** (`compliance`): חריגים לפי שיפוט או מניפסט עוברים דרך רשימות opt‑out שמנוהלות על ידי הממשל. אל תוסיפו overrides אד‑הוק לחבילת התצורה; במקום זאת בקשו עדכון חתום ל‑`governance/compliance/soranet_opt_outs.json` והפיצו מחדש את ה‑JSON שנוצר.

לשני המנגנונים, שמרו את חבילת התצורה שהתקבלה וצרפו אותה לראיות release כדי שמבקרים יוכלו לעקוב אחר איך הופעלו הורדות.

## 6. טלמטריה ודשבורדים

לפני הרחבת ה‑rollout, ודאו שהאותות הבאים פעילים בסביבה היעד:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  צריך להיות אפס אחרי השלמת canary.
- `sorafs_orchestrator_retries_total` ו‑
  `sorafs_orchestrator_retry_ratio` — צריכים להתייצב מתחת ל‑10% במהלך canary ולהישאר
  מתחת ל‑5% לאחר GA.
- `sorafs_orchestrator_policy_events_total` — מאמת שהשלב המצופה פעיל (label `stage`) ורושם brownouts דרך `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — עוקבים אחר היצע ריליי PQ מול ציפיות המדיניות.
- יעדי הלוג `telemetry::sorafs.fetch.*` — חייבים להישלח למאגד הלוגים המשותף עם חיפושים שמורים עבור `status=failed`.

טענו את דשבורד Grafana הקנוני מ‑
`dashboards/grafana/sorafs_fetch_observability.json` (מופיע בפורטל תחת **SoraFS → Fetch Observability**) כדי שמסנני אזור/מניפסט, heatmap retry לפי ספק, היסטוגרמות לטנטיות של chunks ומוני stall יתאימו למה ש‑SRE בודקים בזמן burn‑in. חברו את כללי Alertmanager ב‑`dashboards/alerts/sorafs_fetch_rules.yml` ואמתו את תחביר Prometheus באמצעות `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ה‑helper מריץ `promtool test rules` מקומית או בדוקר). העברות התראה דורשות את אותו בלוק ניתוב שהסקריפט מדפיס כדי שהמפעילים יוכלו לצרף את הראיות לטיקט ה‑rollout.

### זרימת burn-in לטלמטריה

פריט roadmap **SF-6e** מחייב burn‑in טלמטרי של 30 יום לפני החלפת האורקסטרטור רב‑המקורות לברירות המחדל של GA. השתמשו בסקריפטים של המאגר כדי ללכוד חבילת ארטיפקטים ניתנת לשחזור לכל יום בחלון:

1. הריצו `ci/check_sorafs_orchestrator_adoption.sh` עם משתני burn‑in מוגדרים. דוגמה:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   ה‑helper מריץ מחדש את `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   כותב `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` ו‑`adoption_report.json` תחת
   `artifacts/sorafs_orchestrator/<timestamp>/`, ומאכף מספר מינימלי של ספקים eligible
   באמצעות `cargo xtask sorafs-adoption-check`.
2. כשמשתני burn‑in קיימים, הסקריפט גם פולט `burn_in_note.json`, המתעד את התווית, אינדקס היום,
   מזהה המניפסט, מקור הטלמטריה וה‑digests של הארטיפקטים. צרפו את ה‑JSON ליומן ה‑rollout כדי
   שיהיה ברור איזה לכידה עמדה בכל יום בחלון 30 הימים.
3. יבאו את דשבורד Grafana המעודכן (`dashboards/grafana/sorafs_fetch_observability.json`)
   אל סביבת staging/production, תייגו אותו בתווית burn‑in, ואמתו שכל לוח מציג דגימות עבור
   המניפסט/אזור הנבדקים.
4. הריצו `scripts/telemetry/test_sorafs_fetch_alerts.sh` (או `promtool test rules …`)
   בכל שינוי של `dashboards/alerts/sorafs_fetch_rules.yml` כדי לתעד שהניתוב תואם את
   המדדים שיוצאו במהלך burn‑in.
5. ארכבו צילום דשבורד, פלט בדיקת התראות וזנב לוגים של חיפושי `telemetry::sorafs.fetch.*`
   יחד עם ארטיפקטי האורקסטרטור כדי שהממשל יוכל לשחזר את הראיות בלי למשוך נתונים
   ממערכות חיות.

## 7. צ׳ק‑ליסט rollout

1. בצעו רגנרציה ל‑scoreboards ב‑CI עם תצורת המועמד ושמרו את הארטיפקטים תחת בקרת גרסאות.
2. הריצו fetch דטרמיניסטי של fixtures בכל סביבה (lab, staging, canary, production) וצרפו את
   `--scoreboard-out` ו‑`--json-out` לרשומת ה‑rollout.
3. עברו על דשבורדי הטלמטריה יחד עם מהנדס ה‑on-call וודאו שכל המדדים לעיל כוללים דגימות חיות.
4. תעדו את נתיב התצורה הסופי (לרוב דרך `iroha_config`) ואת git commit של רג׳יסטר הממשל
   ששימש לפרסומים ולתאימות.
5. עדכנו את tracker ה‑rollout והודיעו לצוותי ה‑SDK על ברירות המחדל החדשות כדי לשמור על
   יישור קו באינטגרציות הלקוח.

פעולה לפי מדריך זה משאירה את פריסות האורקסטרטור דטרמיניסטיות וברות‑ביקורת, תוך מתן
לולאות משוב ברורות לכיול תקציבי retry, קיבולת ספקים ועמדת פרטיות.
