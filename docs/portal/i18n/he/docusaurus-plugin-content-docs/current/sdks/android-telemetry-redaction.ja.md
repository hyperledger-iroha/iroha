---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 869de542ae9fc479145f99410123a25e36d3ef54b14dd76e2ed3866efcf3034d
source_last_modified: "2026-01-30T16:38:43+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /sdks/android-telemetry
lang: he
direction: rtl
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: תוכנית רידאקציה לטלמטריית Android
sidebar_label: טלמטריית Android
slug: /sdks/android-telemetry
---

:::note מקור קנוני
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# תוכנית רידאקציה לטלמטריית Android (AND7)

## היקף

מסמך זה מתעד את מדיניות הרידאקציה המוצעת לטלמטריה ואת ארטיפקטי ההפעלה עבור Android SDK, כפי שנדרש בפריט המפת דרכים **AND7**. הוא מיישר את המדידה במובייל עם קו הבסיס של צמתי Rust תוך התחשבות בערבויות פרטיות ספציפיות למכשיר. התוצר משמש כ‑pre‑read לסקירת הממשל של SRE בפברואר 2026.

יעדים:

- למפות כל סיגנל שמונפק מ‑Android ומגיע ל‑backends משותפים של תצפיתיות (עקבות OpenTelemetry, לוגים מקודדי Norito, יצוא מדדים).
- לסווג שדות השונים מקו הבסיס של Rust ולתעד בקרות רידאקציה או שימור.
- לתאר עבודות הפעלה ובדיקות כדי שצוותי התמיכה יגיבו בצורה דטרמיניסטית להתראות רידאקציה.

## מלאי סיגנלים (טיוטה)

אינסטרומנטציה מתוכננת לפי ערוץ. כל שמות השדות תואמים לסכמת הטלמטריה של Android SDK (`org.hyperledger.iroha.android.telemetry.*`). שדות אופציונליים מסומנים ב‑`?`.

| מזהה סיגנל | ערוץ | שדות מרכזיים | סיווג PII/PHI | רידאקציה / שמירה | הערות |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Span עקבה | `authority_hash`, `route`, `status_code`, `latency_ms` | authority פומבי; route ללא סודות | לחשב hash ל‑authority (`blake2b_256`) לפני יצוא; לשמור 7 ימים | משקף `torii.http.request` ב‑Rust; hashing שומר על פרטיות alias מובייל. |
| `android.torii.http.retry` | אירוע | `route`, `retry_count`, `error_code`, `backoff_ms` | אין | ללא רידאקציה; לשמור 30 ימים | משמש לביקורות retry דטרמיניסטיות; שדות זהים ל‑Rust. |
| `android.pending_queue.depth` | מדד Gauge | `queue_type`, `depth` | אין | ללא רידאקציה; לשמור 90 ימים | תואם `pipeline.pending_queue_depth` ב‑Rust. |
| `android.keystore.attestation.result` | אירוע | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | alias (נגזר), מטא‑דאטה של מכשיר | להחליף alias בתווית דטרמיניסטית, לרדוק את המותג ל‑enum bucket | נדרש להיערכות AND2; צמתי Rust לא פולטים מטא‑דאטה של מכשיר. |
| `android.keystore.attestation.failure` | מונה | `alias_label`, `failure_reason` | אין לאחר רידאקציה של alias | ללא רידאקציה; לשמור 90 ימים | תומך בתרגילי chaos; `alias_label` נגזר מ‑alias ממושחל. |
| `android.telemetry.redaction.override` | אירוע | `override_id`, `actor_role_masked`, `reason`, `expires_at` | תפקיד השחקן הוא PII תפעולי | יצוא קטגוריית תפקיד ממוסכת; לשמור 365 ימים עם לוג ביקורת | לא קיים ב‑Rust; מפעילים חייבים לפתוח override דרך תמיכה. |
| `android.telemetry.export.status` | מונה | `backend`, `status` | אין | ללא רידאקציה; לשמור 30 ימים | פריטיות עם מוני סטטוס של Rust exporter. |
| `android.telemetry.redaction.failure` | מונה | `signal_id`, `reason` | אין | ללא רידאקציה; לשמור 30 ימים | נדרש כדי לשקף `streaming_privacy_redaction_fail_total` ב‑Rust. |
| `android.telemetry.device_profile` | מדד Gauge | `profile_id`, `sdk_level`, `hardware_tier` | מטא‑דאטה של מכשיר | להוציא buckets גסים (SDK major, hardware tier); לשמור 30 ימים | מאפשר דשבורדים עם פריטיות בלי לחשוף פרטי OEM. |
| `android.telemetry.network_context` | אירוע | `network_type`, `roaming` | ספק סלולר עלול להיות PII | להסיר `carrier_name` לגמרי; לשמור שדות אחרים 7 ימים | `ClientConfig.networkContextProvider` מספק snapshot מסונן כדי שאפליקציות יוכלו להוציא סוג רשת + roaming בלי לחשוף נתוני מנוי; דשבורדים מתייחסים לסיגנל כאל האנלוג המוביילי של `peer_host` ב‑Rust. |
| `android.telemetry.config.reload` | אירוע | `source`, `result`, `duration_ms` | אין | ללא רידאקציה; לשמור 30 ימים | משקף spans של reload config ב‑Rust. |
| `android.telemetry.chaos.scenario` | אירוע | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | פרופיל המכשיר bucketized | כמו `device_profile`; לשמור 30 ימים | נרשם במהלך תרגילי chaos הנדרשים ל‑AND7. |
| `android.telemetry.redaction.salt_version` | מדד Gauge | `salt_epoch`, `rotation_id` | אין | ללא רידאקציה; לשמור 365 ימים | עוקב אחר רוטציה של Blake2b salt; התראה אם epoch של Android שונה מ‑Rust. |
| `android.crash.report.capture` | אירוע | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | טביעת crash + מטא‑דאטה תהליך | לחשב hash ל‑`crash_id` עם salt משותף, ל‑bucketize מצב watchdog, להסיר stack frames לפני יצוא; לשמור 30 ימים | מופעל אוטומטית ב‑`ClientConfig.Builder.enableCrashTelemetryHandler()`; מזין דשבורדים בלי לחשוף עקבות מזהות. |
| `android.crash.report.upload` | מונה | `crash_id`, `backend`, `status`, `retry_count` | טביעת crash | להשתמש ב‑`crash_id` ממושחל ולהוציא רק סטטוס; לשמור 30 ימים | להוציא דרך `ClientConfig.crashTelemetryReporter()` או `CrashTelemetryHandler.recordUpload` כדי לשתף הבטחות Sigstore/OLTP כמו שאר הטלמטריה. |

### נקודות אינטגרציה

- `ClientConfig` מעביר כעת נתוני טלמטריה נגזרים מ‑manifest דרך `setTelemetryOptions(...)`/`setTelemetrySink(...)`, ורושם `TelemetryObserver` אוטומטית כדי שה‑authority הממושחל ומדדי salt יזרמו ללא observers ייעודיים. ראו `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` ואת המחלקות תחת `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- יישומים יכולים לקרוא `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` כדי לרשום `AndroidNetworkContextProvider` מבוסס רפלקציה, שמבצע שאילתות ל‑`ConnectivityManager` בזמן ריצה ומוציא את האירוע `android.telemetry.network_context` ללא תלות קומפילציה באנדרואיד.
- בדיקות היחידה `TelemetryOptionsTests` ו‑`TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) מגנות על עזרי hashing ועל hook ההטמעה של ClientConfig כדי שרגרסיות manifest יזוהו מיידית.
- ערכת ההפעלה וה‑labs מצטטת כעת APIs קונקרטיים במקום פסאודו‑קוד, כך שהמסמך וה‑runbook נשארים מסונכרנים עם ה‑SDK המופץ.

> **הערת תפעול:** גיליון owner/status נמצא ב‑`docs/source/sdk/android/readiness/signal_inventory_worksheet.md` ויש לעדכן אותו יחד עם הטבלה בכל checkpoint של AND7.

## Allowlists לפריטיות וזרימת schema‑diff

הממשל מחייב allowlist כפול כך שיצואי Android לא ידליפו מזהים ששירותי Rust מציגים במכוון. סעיף זה משקף את ה‑runbook (`docs/source/android_runbook.md` §2.3) אך שומר על תוכנית AND7 עצמאית.

| קטגוריה | יצואני Android | שירותי Rust | עוגן בדיקה |
|----------|-------------------|---------------|-----------------|
| הקשר authority/route | לבצע hash ל‑`authority`/`alias` באמצעות Blake2b-256 ולהסיר hostnames גולמיים של Torii לפני יצוא; להוציא `android.telemetry.redaction.salt_version` להוכחת רוטציה. | להוציא hostnames מלאים של Torii ו‑peer IDs לצורך קורלציה. | להשוות `android.torii.http.request` מול `torii.http.request` ב‑diff האחרון תחת `docs/source/sdk/android/readiness/schema_diffs/`, ואז להריץ `scripts/telemetry/check_redaction_status.py` לאימות epochs של salt. |
| זהות מכשיר/חותם | לבצע bucket ל‑`hardware_tier`/`device_profile`, לבצע hash ל‑controller aliases, ולא לייצא מספרים סידוריים. | להוציא `peer_id` של מאמת, `public_key` של controller ו‑queue hashes כמות שהם. | להתיישר עם `docs/source/sdk/mobile_device_profile_alignment.md`, להריץ בדיקות alias hashing בתוך `java/iroha_android/run_tests.sh`, ולשמור outputs של queue inspector במהלך labs. |
| מטא‑דאטה רשת | לייצא רק `network_type` + `roaming`; להסיר `carrier_name`. | לשמור מטא‑דאטה hostname/TLS של peers. | לשמור כל diff ב‑`readiness/schema_diffs/` ולהתריע אם ווידג׳ט Grafana “Network Context” מציג מחרוזות carrier. |
| Evidence של override/chaos | לייצא `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` עם תפקידים ממוסכים. | להוציא approvals של override ללא מסכה; ללא spans של chaos. | לבדוק את `docs/source/sdk/android/readiness/and7_operator_enablement.md` לאחר drills כדי לוודא שטוקני override ו‑chaos artefacts נמצאים לצד אירועי Rust לא ממוסכים. |

זרימת עבודה:

1. לאחר כל שינוי ב‑manifest או exporter, להריץ `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` ולמקם את ה‑JSON תחת `docs/source/sdk/android/readiness/schema_diffs/`.
2. לבדוק את ה‑diff מול הטבלה לעיל. אם Android מוציא שדה שקיים רק ב‑Rust (או להפך), לפתוח bug של AND7 readiness ולעדכן תוכנית זו ואת ה‑runbook.
3. בסקירות ops שבועיות, להריץ `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` ולתעד את epoch של ה‑salt ואת חותמת הזמן של diff בגיליון readiness.
4. לתעד כל חריגה ב‑`docs/source/sdk/android/readiness/signal_inventory_worksheet.md` כדי שחבילות הממשל ישקפו החלטות פריטיות.

> **הפניה לסכמה:** מזהי שדות קנוניים מקורם ב‑`android_telemetry_redaction.proto` (מוחש בזמן בניית Android SDK יחד עם Norito descriptors). הסכמה חושפת `authority_hash`, `alias_label`, `attestation_digest`, `device_brand_bucket` ו‑`actor_role_masked` שנמצאים בשימוש ב‑SDK וביצואני הטלמטריה.

`authority_hash` הוא digest קבוע של 32 בתים לערך authority של Torii. `attestation_digest` לוכד את טביעת האצבע של הצהרת attestation הקנונית, ו‑`device_brand_bucket` ממפה את מחרוזת המותג הגולמית של Android ל‑enum המאושר (`generic`, `oem`, `enterprise`). `actor_role_masked` נושא את קטגוריית התפקיד (`support`, `sre`, `audit`) במקום מזהה המשתמש הגולמי.

### יישור יצוא טלמטריית קריסה

טלמטריית קריסה משתמשת כעת באותם יצואני OpenTelemetry ובאותה שרשרת provenance כמו אותות Torii, וסוגרת את follow‑up של הממשל לגבי יצואנים כפולים. מטפל הקריסות מזין את האירוע `android.crash.report.capture` עם `crash_id` ממושחל (Blake2b-256 תוך שימוש ב‑salt שמתועד ב‑`android.telemetry.redaction.salt_version`), buckets של מצב תהליך ומטא‑דאטה מסוננת של ANR watchdog. stack traces נשארות על המכשיר ומסוכמות רק ב‑`has_native_trace` וב‑`anr_watchdog_bucket` לפני יצוא, כך ש‑PII ומחרוזות OEM לא יוצאות מהמכשיר.

העלאת קריסה יוצרת את מונה `android.crash.report.upload`, ומאפשרת ל‑SRE לבקר את אמינות ה‑backend בלי לדעת דבר על המשתמש או על ה‑stack. מכיוון ששני האותות משתמשים באותו יצואן Torii, הם יורשים את חתימות Sigstore, מדיניות השימור ו‑alerting hooks שכבר הוגדרו עבור AND7. כך runbooks יכולים לקשר מזהה crash ממושחל בין חבילות evidence של Android ושל Rust בלי pipeline ייעודי.

הפעילו את המטפל דרך `ClientConfig.Builder.enableCrashTelemetryHandler()` לאחר הגדרת telemetry options/sinks; ניתן להשתמש מחדש ב‑`ClientConfig.crashTelemetryReporter()` (או `CrashTelemetryHandler.recordUpload`) כדי להוציא תוצאות backend באותו pipeline חתום.

## הבדלי מדיניות מול קו הבסיס של Rust

הבדלים בין מדיניות הטלמטריה של Android ו‑Rust עם צעדי מיתון.

| קטגוריה | קו בסיס Rust | מדיניות Android | מיתון / אימות |
|----------|---------------|----------------|-------------------------|
| מזהי authority/peer | מחרוזות authority גלויות | `authority_hash` (Blake2b-256, salt מתחלף) | salt משותף מפורסם דרך `iroha_config.telemetry.redaction_salt`; בדיקת פריטיות מאשרת מיפוי הפיך עבור תמיכה. |
| מטא‑דאטה host/network | Hostnames/IPs של צמתים מיוצאים | סוג רשת + roaming בלבד | דשבורדי בריאות רשת עודכנו להשתמש בקטגוריות זמינות במקום hostnames. |
| מאפייני מכשיר | N/A (צד שרת) | פרופיל bucketized (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | תרגילי chaos מאמתים את המיפוי; ה‑runbook מתעד מסלול הסלמה כשצריך פרטים נוספים. |
| Overrides של רידאקציה | לא נתמך | טוקן override ידני נשמר ב‑Norito ledger (`actor_role_masked`, `reason`) | Overrides דורשים בקשה חתומה; audit log נשמר שנה. |
| עקבות attestation | attestation שרת באמצעות SRE בלבד | ה‑SDK מוציא תקציר attestation מסונן | להשוות hashes עם המאמת של Rust; hashing של alias מונע דליפות. |

Checklist אימות:

- בדיקות יחידה לרידאקציה עבור כל סיגנל מאמתות שדות ממושחלים/מוסתרים לפני שליחת exporter.
- כלי schema diff (משותף ל‑Rust) רץ מדי לילה כדי לאשר פריטיות שדות.
- סקריפט chaos rehearsal מתרגל את זרימת override ומאשר רישום audit.

## משימות יישום (לפני ממשל SRE)

1. **אימות מלאי** — לחצות את הטבלה מול hooks אמיתיים ב‑Android SDK והגדרות Norito schema. Owners: Android Observability TL, LLM.
2. **Telemetry Schema Diff** — להריץ את כלי diff המשותף מול מדדי Rust כדי ליצור ארטיפקטי פריטיות לריוויו SRE. Owner: SRE privacy lead.
3. **טיוטת Runbook (הושלמה 2026-02-03)** — `docs/source/android_runbook.md` מתעד כעת את זרימת override מקצה‑לקצה (סעיף 3) ואת מטריצת ההסלמה/תפקידים המורחבת (סעיף 3.1), וקושר helper‑ים של CLI, ראיות תקרית ו‑chaos scripts למדיניות הממשל. Owners: LLM עם עריכת Docs/Support.
4. **תוכן הפעלה** — להכין שקופיות תדרוך, הוראות lab ושאלות בדיקת ידע למפגש פברואר 2026. Owners: Docs/Support Manager, צוות enablement של SRE.

## זרימת הפעלה וחיבורי runbook

### 1. כיסוי smoke מקומי + CI

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` מרים sandbox של Torii, משחזר את ה‑fixture הקנוני multi‑source של SoraFS (באמצעות `ci/check_sorafs_orchestrator_adoption.sh`) ומזריע טלמטריה סינתטית של Android.
  - יצירת התעבורה מתבצעת ע״י `scripts/telemetry/generate_android_load.py`, שמקליט transcript של request/response תחת `artifacts/android/telemetry/load-generator.log` ומכבד headers, overrides לנתיב או מצב dry‑run.
  - ה‑helper מעתיק scoreboard/summary של SoraFS ל‑`${WORKDIR}/sorafs/` כדי שהתרגילים AND7 יוכיחו פריטיות multi‑source לפני הפעלת לקוחות מובייל.
- ה‑CI משתמש באותם כלים: `ci/check_android_dashboard_parity.sh` מריץ `scripts/telemetry/compare_dashboards.py` מול `dashboards/grafana/android_telemetry_overview.json`, הדשבורד הייחוסי של Rust ו‑allowlist `dashboards/data/android_rust_dashboard_allowances.json`, ומפיק את snapshot החתום `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- תרגילי chaos פועלים לפי `docs/source/sdk/android/telemetry_chaos_checklist.md`; סקריפט sample‑env יחד עם בדיקת parity של דשבורדים מייצרים את חבילת ה‑“ready” שמזינה את ביקורת ה‑burn‑in של AND7.

### 2. הנפקת override ושובל ביקורת

- `scripts/android_override_tool.py` הוא CLI קנוני להנפקה ולביטול overrides. `apply` בולע בקשה חתומה, מוציא manifest bundle (`telemetry_redaction_override.to` כברירת מחדל) ומוסיף שורת טוקן ממושחל ל‑`docs/source/sdk/android/telemetry_override_log.md`. `revoke` מסמן חותמת ביטול באותה שורה, ו‑`digest` כותב snapshot JSON מסונן הנדרש לממשל.
- ה‑CLI מסרב לשנות audit log אם אין כותרת טבלת Markdown, בהתאם לדרישת התאימות ב‑`docs/source/android_support_playbook.md`. הכיסוי היחידתי ב‑`scripts/tests/test_android_override_tool_cli.py` מגן על פרסור הטבלה, emitters של manifest וטיפול שגיאות.
- מפעילים מצרפים את ה‑manifest שנוצר, קטע הלוג המעודכן **וגם** digest JSON תחת `docs/source/sdk/android/readiness/override_logs/` בכל override; הלוג נשמר 365 ימים בהתאם להחלטת הממשל.

### 3. לכידת ראיות ושימור

- כל rehearsal או אירוע מייצר חבילה מובנית תחת `artifacts/android/telemetry/` הכוללת:
  - transcript של מחולל העומס ומונים מצטברים מ‑`generate_android_load.py`.
  - diff של דשבורדים (`android_vs_rust-<stamp>.json`) ו‑allowlist hash שנפלטים מ‑`ci/check_android_dashboard_parity.sh`.
  - delta של override log (אם הוענק override), ה‑manifest המתאים, ו‑digest JSON מעודכן.
- דוח ה‑SRE burn‑in מפנה לארטיפקטים הללו ול‑SoraFS scoreboard שהועתק ע״י `android_sample_env.sh`, ומספק שרשרת דטרמיניסטית של טלמטריה hashes → דשבורדים → מצב overrides.

## יישור פרופיל מכשיר בין SDKs

הדשבורדים מתרגמים את `hardware_tier` של Android ל‑`mobile_profile_class` הקנוני המוגדר ב‑`docs/source/sdk/mobile_device_profile_alignment.md` כדי ש‑AND7 ו‑IOS7 ישוו אותן קוהורטות:

- `lab` — נפלט כ‑`hardware_tier = emulator`, תואם ל‑`device_profile_bucket = simulator` ב‑Swift.
- `consumer` — נפלט כ‑`hardware_tier = consumer` (עם סיומת SDK major) ומקובץ עם buckets `iphone_small`/`iphone_large`/`ipad` של Swift.
- `enterprise` — נפלט כ‑`hardware_tier = enterprise`, מיושר עם bucket `mac_catalyst` של Swift ו‑managed iOS desktop runtimes עתידיים.

כל tier חדש חייב להתווסף למסמך היישור ולארטיפקטי schema diff לפני שהדשבורדים צורכים אותו.

## ממשל והפצה

- **חבילת pre‑read** — מסמך זה ונספחיו (schema diff, runbook diff, outline של readiness deck) יופצו לרשימת הדוא״ל של ממשל SRE לא יאוחר מ‑**2026-02-05**.
- **לולאת משוב** — הערות שנאספות בזמן הממשל יוזנו ל‑JIRA epic `AND7`; חסמים יוצגו ב‑`status.md` וב‑Android weekly stand‑up notes.
- **פרסום** — לאחר אישור, סיכום המדיניות יקושר מ‑`docs/source/android_support_playbook.md` ויוזכר ב‑FAQ טלמטריה משותף ב‑`docs/source/telemetry.md`.

## הערות ביקורת וציות

- המדיניות מכבדת GDPR/CCPA על‑ידי הסרת נתוני מנוי מובייל לפני היצוא; ה‑salt של authority hash מסתובב רבעונית ומאוחסן בכספת סודות משותפת.
- ארטיפקטי הפעלה ועדכוני runbook מתועדים ברג’יסטר הציות.
- בדיקות רבעוניות מאשרות ש‑overrides נשארים closed‑loop (ללא גישה מיושנת).

## תוצאת ממשל (2026-02-12)

מפגש ממשל SRE ב‑**2026-02-12** אישר את מדיניות רידאקציית Android ללא שינויים. החלטות מפתח (ראו `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **קבלת המדיניות.** אושר hashing של authority, bucketization של פרופיל מכשיר והסרת שמות carriers. מעקב רוטציית salt באמצעות `android.telemetry.redaction.salt_version` הופך לפריט ביקורת רבעוני.
- **תוכנית אימות.** אושרו כיסוי unit/integration, diffs ליליים ו‑chaos rehearsals רבעוניים. פעולה: לפרסם דו״ח פריטיות דשבורדים אחרי כל rehearsal.
- **ממשל overrides.** טוקני override מתועדים ב‑Norito אושרו עם חלון שמירה של 365 ימים. Support engineering אחראית על review של digest override log בסנכרוני התפעול החודשיים.

## סטטוס מעקב

1. **יישור פרופיל מכשיר (מועד 2026-03-01).** ✅ הושלם — המיפוי המשותף ב‑`docs/source/sdk/mobile_device_profile_alignment.md` מגדיר כיצד `hardware_tier` של Android ממופה ל‑`mobile_profile_class` הקנוני עבור דשבורדי הפריטיות וכלי diff.

## brief ממשל SRE הבא (Q2 2026)

פריט **AND7** מחייב שהמפגש הבא יקבל pre‑read תמציתי על רידאקציית Android. השתמשו בסעיף זה כ‑brief חי ועדכנו לפני כל ישיבת מועצה.

### Checklist הכנה

1. **Evidence bundle** — ייצאו את ה‑schema diff העדכני, צילומי דשבורד ו‑override log digest (ראו מטריצה להלן) ושימו אותם בתיקייה מתוארכת (לדוגמה `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) לפני שליחת ההזמנה.
2. **סיכום drills** — צרפו את לוג ה‑chaos rehearsal האחרון ואת snapshot של `android.telemetry.redaction.failure`; ודאו שהערות Alertmanager מתייחסות לאותו timestamp.
3. **Audit של overrides** — ודאו שכל overrides פעילים רשומים ברג’יסטר Norito ומסוכמים ב‑meeting deck. כללו תאריכי תפוגה ו‑incident IDs מתאימים.
4. **הערת אג׳נדה** — שלחו ל‑SRE chair 48 שעות לפני הישיבה עם קישור ל‑brief והדגישו החלטות נדרשות (סיגנלים חדשים, שינויי שימור או עדכוני מדיניות overrides).

### מטריצת ראיות

| ארטיפקט | מיקום | Owner | הערות |
|----------|----------|-------|-------|
| Schema diff מול Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | חייב להיווצר <72 שעות לפני הישיבה. |
| צילומי dashboard diff | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | לכלול `sorafs.fetch.*`, `android.telemetry.*` וצילומי Alertmanager. |
| Override digest | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | להריץ `scripts/android_override_tool.sh digest` (ראו README בתיקייה) על `telemetry_override_log.md` העדכני; הטוקנים נשארים ממושחלים. |
| Chaos rehearsal log | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | לצרף תקציר KPI (stall count, retry ratio, override usage). |

### שאלות פתוחות למועצה

- האם צריך לקצר את חלון שמירת overrides מ‑365 ימים עכשיו שה‑digest אוטומטי?
- האם `android.telemetry.device_profile` צריך לאמץ את התוויות המשותפות `mobile_profile_class` בגרסה הבאה, או להמתין ל‑Swift/JS SDK?
- האם נדרשת הנחיה נוספת לרזידנסיות נתונים אזורית כאשר אירועי Torii Norito-RPC יגיעו ל‑Android (follow‑up NRPC-3)?

### הליך Telemetry Schema Diff

הריצו את כלי ה‑schema diff לפחות פעם אחת לכל release candidate (ובכל שינוי באינסטרומנטציה של Android) כדי שה‑SRE council יקבל ארטיפקטי פריטיות עדכניים יחד עם dashboard diff:

1. יצאו את סכמות הטלמטריה של Android ו‑Rust להשוואה. ב‑CI ההגדרות נמצאות ב‑`configs/android_telemetry.json` ו‑`configs/rust_telemetry.json`.
2. הריצו `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`.
   - לחלופין, העבירו commits (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) כדי למשוך configs ישירות מ‑git; הסקריפט מקבע hashes בתוך הארטיפקט.
3. צרפו את ה‑JSON לחבילת readiness וקשרו אותו מ‑`status.md` ו‑`docs/source/telemetry.md`. ה‑diff מדגיש שדות שנוספו/הוסרו ודלתות שימור כדי שהמבקרים יאשרו פריטיות בלי להריץ שוב את הכלי.
4. אם ה‑diff חושף סטייה מותרת (למשל override signals של Android בלבד), עדכנו את allowlist שמופנה ב‑`ci/check_android_dashboard_parity.sh` וציינו את הנימוק ב‑README של תיקיית ה‑diff.

> **כללי ארכוב:** שמרו את חמשת ה‑diffs האחרונים תחת `docs/source/sdk/android/readiness/schema_diffs/` והעבירו snapshots ישנים ל‑`artifacts/android/telemetry/schema_diffs/` כדי שמבקרי ממשל יראו תמיד את הנתונים העדכניים ביותר.
