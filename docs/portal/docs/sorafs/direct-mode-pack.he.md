---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59d36561f60e28f8d7d5794e953755d3221daf8f0627e0be6a540115b0ad1f05
source_last_modified: "2025-11-20T08:09:37.293192+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: direct-mode-pack
title: חבילת fallback למצב ישיר של SoraFS ‏(SNNet-5a)
sidebar_label: חבילת מצב ישיר
description: הגדרות נדרשות, בדיקות תאימות ושלבי פריסה בהפעלת SoraFS במצב Torii/QUIC ישיר במהלך מעבר SNNet-5a.
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/direct_mode_pack.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של Sphinx יופסק.
:::

מעגלי SoraNet נשארים נתיב התעבורה ברירת המחדל ל‑SoraFS, אך סעיף מפת הדרכים **SNNet-5a** מחייב fallback רגולטורי כדי שמפעילים יוכלו לשמור על גישת קריאה דטרמיניסטית בזמן שהשקת האנונימיות מושלמת. חבילה זו מרכזת את כפתורי ה‑CLI/SDK, פרופילי הקונפיגורציה, בדיקות התאימות ורשימת הפריסה הדרושים להפעלת SoraFS במצב Torii/QUIC ישיר בלי לגעת בתעבורות הפרטיות.

ה‑fallback חל על staging וסביבות פרודקשן רגולטוריות עד ש‑SNNet-5 עד SNNet-9 יעברו את שערי המוכנות. שמרו את הארטיפקטים שלהלן לצד חומרי הפריסה הרגילים של SoraFS כך שמפעילים יוכלו לעבור בין מצב אנונימי למצב ישיר לפי הצורך.

## 1. דגלי CLI ו‑SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` משבית תזמון ריליי ומחייב תעבורת Torii/QUIC. עזרת ה‑CLI מציגה כעת `direct-only` כערך תקין.
- על ה‑SDK להגדיר `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` בכל פעם שהם חושפים מתג "מצב ישיר". ה‑bindings שנוצרים ב‑`iroha::ClientOptions` וב‑`iroha_android` מעבירים את אותו enum.
- כלי gateway (`sorafs_fetch`, bindings של Python) יכולים לפענח את מתג direct-only דרך עוזרי Norito JSON משותפים כדי שהאוטומציה תקבל התנהגות זהה.

תעדו את הדגל ב‑runbooks הפונים לשותפים והעבירו מתגי תכונה דרך `iroha_config` ולא דרך משתני סביבה.

## 2. פרופילי מדיניות של ה‑gateway

השתמשו ב‑Norito JSON כדי להתמיד קונפיגורציה דטרמיניסטית של האורקסטרטור. פרופיל הדוגמה ב‑`docs/examples/sorafs_direct_mode_policy.json` מקודד:

- `transport_policy: "direct_only"` — דוחה ספקים שמפרסמים רק תעבורות ריליי של SoraNet.
- `max_providers: 2` — מגביל את העמיתים הישירים לנקודות הקצה Torii/QUIC האמינות ביותר. התאימו לפי דרישות תאימות אזוריות.
- `telemetry_region: "regulated-eu"` — מסמן את המטריקות כך שלוחות מחוונים וביקורות יבדילו ריצות fallback.
- תקציבי retry שמרניים (`retry_budget: 2`, `provider_failure_threshold: 3`) כדי להימנע מהסתרת gateway לא מוגדרים היטב.

טעינו את ה‑JSON דרך `sorafs_cli fetch --config` (אוטומציה) או דרך bindings של ה‑SDK (`config_from_json`) לפני חשיפת המדיניות למפעילים. שמרו את פלט ה‑scoreboard (`persist_path`) לצורכי ביקורת.

מנגנוני האכיפה בצד ה‑gateway מתועדים ב‑`docs/examples/sorafs_gateway_direct_mode.toml`. התבנית משקפת את הפלט של `iroha app sorafs gateway direct-mode enable`, מנטרלת בדיקות envelope/admission, מחווטת ברירות מחדל של rate-limit, וממלאת את טבלת `direct_mode` בשמות מארחים הנגזרים מתוכנית וב‑digests של manifest. החליפו את ערכי מציין המקום בתוכנית ההשקה לפני שמקבעים את הקטע בניהול תצורה.

## 3. חבילת בדיקות תאימות

מוכנות מצב ישיר כוללת כעת כיסוי באורקסטרטור וב‑CLI:

- `direct_only_policy_rejects_soranet_only_providers` מבטיח ש‑`TransportPolicy::DirectOnly` ייכשל במהירות כאשר כל advert מועמד תומך רק בריליי SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` מבטיח שתעבורות Torii/QUIC ישמשו כאשר הן זמינות ושמריליי SoraNet יוחרגו מהסשן.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` מנתח את `docs/examples/sorafs_direct_mode_policy.json` כדי לוודא שהמסמכים נשארים מתואמים עם עוזרי הכלים.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` מפעיל את `sorafs_cli fetch --transport-policy=direct-only` מול gateway Torii מדומה, ומספק smoke test לסביבות רגולטוריות שמקבעות תעבורה ישירה.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` עוטף את אותה פקודה עם JSON מדיניות ושמירת scoreboard לאוטומציית rollout.

הריצו את החבילה הממוקדת לפני פרסום עדכונים:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

אם קומפילציית ה‑workspace נכשלת בגלל שינויים upstream, רשמו את השגיאה החוסמת ב‑`status.md` והריצו מחדש לאחר שההתלות מתעדכנת.

## 4. הרצות smoke אוטומטיות

כיסוי CLI לבדו לא חושף רגרסיות ספציפיות לסביבה (למשל סטייה במדיניות gateway או אי התאמות במניפסט). עוזר smoke ייעודי נמצא ב‑`scripts/sorafs_direct_mode_smoke.sh` ועוטף את `sorafs_cli fetch` עם מדיניות האורקסטרטור במצב ישיר, שמירת scoreboard ולכידת סיכום.

דוגמה לשימוש:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- הסקריפט מכבד גם דגלי CLI וגם קובצי תצורה key=value (ראו `docs/examples/sorafs_direct_mode_smoke.conf`). מלאו את digest של המניפסט ואת ערכי adverts של הספקים עם ערכי פרודקשן לפני ההרצה.
- `--policy` ברירת המחדל היא `docs/examples/sorafs_direct_mode_policy.json`, אך ניתן לספק כל JSON של אורקסטרטור שנוצר על ידי `sorafs_orchestrator::bindings::config_to_json`. ה‑CLI מקבל את המדיניות דרך `--orchestrator-config=PATH`, ומאפשר הרצות שחוזרות על עצמן בלי לכוון דגלים ידנית.
- כאשר `sorafs_cli` לא נמצא ב‑`PATH`, העוזר בונה אותו מ‑crate `sorafs_orchestrator` (פרופיל release) כדי שהרצות ה‑smoke יפעילו את תשתית מצב ישיר שנשלחת.
- פלטים:
  - payload מורכב (`--output`, ברירת מחדל `artifacts/sorafs_direct_mode/payload.bin`).
  - סיכום fetch (`--summary`, ברירת מחדל ליד ה‑payload) המכיל את אזור הטלמטריה ודוחות הספקים ששימשו כראיות ל‑rollout.
  - snapshot של scoreboard הנשמר בנתיב שהוגדר ב‑JSON המדיניות (למשל `fetch_state/direct_mode_scoreboard.json`). ארכבו אותו יחד עם הסיכום בכרטיסי שינוי.
- אוטומציית שער אימוץ: לאחר שה‑fetch מסתיים העוזר מפעיל את `cargo xtask sorafs-adoption-check` תוך שימוש בנתיבי scoreboard ו‑summary שנשמרו. הקוורום הנדרש ברירת מחדל הוא מספר הספקים שצוינו בשורת הפקודה; אפשר לעקוף עם `--min-providers=<n>` כשצריך מדגם גדול יותר. דוחות האימוץ נכתבים ליד הסיכום (`--adoption-report=<path>` יכול לקבוע מיקום מותאם אישית) והעוזר מעביר `--require-direct-only` כברירת מחדל (בהתאם ל‑fallback) ו‑`--require-telemetry` בכל פעם שתספקו את הדגל התואם. השתמשו ב‑`XTASK_SORAFS_ADOPTION_FLAGS` כדי להעביר ארגומנטים נוספים ל‑xtask (למשל `--allow-single-source` במהלך downgrade מאושר כדי שהשער גם יסבול וגם יאכוף את ה‑fallback). דלגו על שער האימוץ עם `--skip-adoption-check` רק בעת אבחון מקומי; מפת הדרכים דורשת שכל הרצה רגולטורית במצב ישיר תכלול חבילת דוח אימוץ.

## 5. רשימת בדיקות לפריסה

1. **קיפאון קונפיגורציה:** אחסנו את פרופיל JSON של מצב ישיר במאגר `iroha_config` ורשמו את ההאש בכרטיס השינוי.
2. **ביקורת gateway:** ודאו שנקודות הקצה Torii אוכפות TLS, TLVs של יכולות ולוגי ביקורת לפני המעבר למצב ישיר. פרסמו את פרופיל מדיניות ה‑gateway למפעילים.
3. **אישור תאימות:** שתפו את ה‑playbook המעודכן עם סוקרי תאימות/רגולציה ותעדו אישורים להפעלה מחוץ לשכבת האנונימיות.
4. **Dry run:** הריצו את חבילת התאימות ועוד fetch ב‑staging מול ספקי Torii מהימנים. ארכבו את פלטי ה‑scoreboard ואת סיכומי ה‑CLI.
5. **מעבר לפרודקשן:** הכריזו על חלון שינוי, העבירו את `transport_policy` ל‑`direct_only` (אם בחרתם ב‑`soranet-first`), ועקבו אחר לוחות המחוונים של מצב ישיר (לטנטיות `sorafs_fetch`, מוני כשל ספקים). תעדו את תוכנית ה‑rollback כדי לחזור ל‑SoraNet-first לאחר ש‑SNNet-4/5/5a/5b/6a/7/8/12/13 יסיימו ב‑`roadmap.md:532`.
6. **סקירה אחרי שינוי:** צרפו snapshots של scoreboard, סיכומי fetch ותוצאות ניטור לכרטיס השינוי. עדכנו את `status.md` עם התאריך האפקטיבי וכל חריגה.

שמרו את הרשימה לצד ה‑runbook `sorafs_node_ops` כדי שמפעילים יוכלו לתרגל את הזרימה לפני מעבר חי. כש‑SNNet-5 יעבור ל‑GA, הסירו את ה‑fallback לאחר אישור פריטייות בטלמטריה בייצור.

## 6. דרישות ראיות ושער אימוץ

לכידות מצב ישיר חייבות עדיין לעמוד בשער אימוץ SF-6c. צרפו את ה‑scoreboard, ה‑summary, מעטפת ה‑manifest ודוח האימוץ בכל הרצה כדי ש‑`cargo xtask sorafs-adoption-check` יוכל לאמת את מצב ה‑fallback. שדות חסרים גורמים לכשל בשער, לכן רשמו את המטאדאטה הצפויה בכרטיסי שינוי.

- **מטאדאטה של תעבורה:** `scoreboard.json` חייב להכריז `transport_policy="direct_only"` (ולהפוך `transport_policy_override=true` כאשר אתם מכריחים downgrade). שמרו את שדות מדיניות האנונימיות המזווגים מלאים גם כאשר הם יורשים ברירות מחדל כדי שהסוקרים יראו האם סטיתם מתוכנית האנונימיות המדורגת.
- **מונה ספקים:** סשנים של gateway-only חייבים לשמר `provider_count=0` ולמלא `gateway_provider_count=<n>` במספר ספקי Torii שנעשה בהם שימוש. הימנעו מעריכת ה‑JSON ידנית: ה‑CLI/SDK כבר נגזר מהם את המונים, ושער האימוץ דוחה לכידות שחסרות את ההפרדה.
- **ראיות manifest:** כאשר gateways של Torii משתתפים, העבירו את `--gateway-manifest-envelope <path>` החתום (או המקביל ב‑SDK) כדי ש‑`gateway_manifest_provided` יחד עם `gateway_manifest_id`/`gateway_manifest_cid` ירשמו ב‑`scoreboard.json`. ודאו ש‑`summary.json` נושא את אותו `manifest_id`/`manifest_cid`; בדיקת האימוץ נכשלת אם אחד הקבצים חסר את הזוג.
- **ציפיות טלמטריה:** כשטלמטריה מלווה את הלכידה, הריצו את השער עם `--require-telemetry` כדי שהדוח יוכיח שהמטריקות נשלחו. חזרות מבודדות יכולות להשמיט את הדגל, אך CI וכרטיסי שינוי צריכים לתעד את ההיעדר.

דוגמה:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

צרפו את `adoption_report.json` לצד ה‑scoreboard, ה‑summary, מעטפת ה‑manifest וחבילת לוגים של smoke. ארטיפקטים אלה משקפים את מה שעבודת האימוץ ב‑CI (`ci/check_sorafs_orchestrator_adoption.sh`) אוכפת ושומרים על downgrade במצב ישיר בר־ביקורת.
