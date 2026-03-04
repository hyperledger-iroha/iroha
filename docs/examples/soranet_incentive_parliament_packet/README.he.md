---
lang: he
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_incentive_parliament_packet/README.md -->

# חבילת פרלמנט תמריצי Relay של SoraNet

חבילה זו כוללת את ה-artefacts הנדרשים לפרלמנט Sora לאישור תשלומי relay אוטומטיים (SNNet-7):

- `reward_config.json` - תצורת מנוע תגמולים הניתנת לסריאליזציה ב-Norito, מוכנה לטעינה ע"י `iroha app sorafs incentives service init`. `budget_approval_id` תואם ל-hash הרשום בפרוטוקולי הממשל.
- `shadow_daemon.json` - מיפוי מוטבים ו-bonds שמנוצל ע"י harness ה-replay (`shadow-run`) וה-daemon בפרודקשן.
- `economic_analysis.md` - סיכום הוגנות עבור סימולציית shadow 2025-10 -> 2025-11.
- `rollback_plan.md` - playbook תפעולי להשבתת תשלומים אוטומטיים.
- artefacts תומכים: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## בדיקות תקינות

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

השוו את ה-digests לערכים שנרשמו בפרוטוקולי הפרלמנט. אמתו את חתימת ה-shadow-run כמתואר ב-
`docs/source/soranet/reports/incentive_shadow_run.md`.

## עדכון החבילה

1. רעננו את `reward_config.json` בכל שינוי במשקלי תגמול, בתשלום הבסיס או ב-hash האישור.
2. הריצו מחדש סימולציית shadow של 60 ימים, עדכנו את `economic_analysis.md` בממצאים החדשים, ובצעו commit ל-JSON ולחתימה המנותקת.
3. הציגו את החבילה המעודכנת לפרלמנט יחד עם exports של דשבורדי Observatory בעת בקשת חידוש.

</div>
