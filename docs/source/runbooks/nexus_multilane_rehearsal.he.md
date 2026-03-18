---
lang: he
direction: rtl
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# ראנבוק רהרסל השקה רב‑ליין ל‑Nexus

ראנבוק זה מדריך את רהרסל Phase B4 של Nexus. הוא מאמת שה‑`iroha_config` המאושר
ע"י governance יחד עם מניפסט genesis רב‑ליין מתנהגים באופן דטרמיניסטי בטלמטריה,
בניתוב וב‑rollback drills.

## היקף

- להריץ את שלושת ה‑lanes של Nexus (`core`, `governance`, `zk`) עם ingress מעורב של
  Torii (טרנזקציות, פריסות חוזים, פעולות ממשל) תוך שימוש ב‑seed חתום `NEXUS-REH-2026Q1`.
- ללכוד artefacts טלמטריה/trace הנדרשים לקבלת B4 (Prometheus scrape, OTLP export,
  לוגים מובנים, Norito admission traces, מטריקות RBC).
- לבצע את drill ה‑rollback `B4-RB-2026Q1` מיד לאחר ה‑dry‑run ולאשר שהפרופיל החד‑ליין
  חוזר נקי.

## תנאי קדם

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` משקף את אישור
   GOV-2026-03-19 (מניפסטים חתומים + ראשי תיבות הסוקרים).
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, עם
   `nexus.enabled = true` מוטמע) ו‑`defaults/nexus/genesis.json` תואמים ל‑hashes
   המאושרים; `kagami genesis bootstrap --profile nexus` מדווח את אותו digest
   שנרשם ב‑tracker.
3. קטלוג ה‑lanes תואם את פריסת שלושת ה‑lanes המאושרת; `irohad --sora --config defaults/nexus/config.toml`
   אמור להציג את ה‑banner של Nexus router.
4. ה‑CI הרב‑ליין ירוק: `ci/check_nexus_multilane_pipeline.sh`
   (מריץ `integration_tests/tests/nexus/multilane_pipeline.rs` דרך
   `.github/workflows/integration_tests_multilane.yml`) ו‑
   `ci/check_nexus_multilane.sh` (כיסוי router) עוברים כדי שהפרופיל של Nexus
   יישאר multi‑lane‑ready (`nexus.enabled = true`, hashes של קטלוג Sora שלמים,
   storage תחת `blocks/lane_{id:03}_{slug}` ולוגי merge מוכנים). לתעד digests
   של artefacts ב‑tracker כשה‑defaults bundle משתנה.
5. Dashboards + alertים עבור מטריקות Nexus מיובאים לתיקיית Grafana של הרהרסל;
   נתיבי alert מצביעים ל‑PagerDuty של הרהרסל.
6. ה‑lanes של Torii SDK מוגדרים בהתאם לטבלת מדיניות הניתוב ויכולים לשחזר את
   עומס הרהרסל מקומית.

## ציר זמן

| שלב | חלון יעד | Owner(s) | קריטריון יציאה |
|-------|-------------|----------|---------------|
| הכנה | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed פורסם, dashboards מוכנים, nodes הוקמו. |
| הקפאת staging | Apr 8 2026 18:00 UTC | @release-eng | hashes של config/genesis אומתו מחדש; הודעת freeze נשלחה. |
| ביצוע | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Checklist הושלמה ללא אירועים חוסמים; חבילת טלמטריה הועברה לארכיון. |
| Drill rollback | מייד לאחר הביצוע | @sre-core | `B4-RB-2026Q1` הושלם; טלמטריית rollback נאספה. |
| רטרוספקטיבה | עד Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | מסמך רטרו/לקחים + tracker חסמים פורסם. |

## Checklist ביצוע (Apr 9 2026 15:00 UTC)

1. **אימות config** — `iroha_cli config show --actual` בכל node; לוודא hashes תואמים ל‑tracker.
2. **Warm‑up lanes** — להריץ את seed לשני slots ולאמת `nexus_lane_state_total` בפעילות בכל שלושת ה‑lanes.
3. **לכידת טלמטריה** — לצלם Prometheus `/metrics`, דגימות OTLP, לוגים מובנים של Torii (לכל lane/dataspace), ומטריקות RBC.
4. **Hooks של governance** — להריץ תת‑סט של טרנזקציות governance ולאמת ניתוב ליין + תגים בטלמטריה.
5. **Drill אירוע** — לדמות רוויה של lane לפי התכנית; לוודא שההתראות נשלחות והתגובה נרשמת.
6. **Drill rollback `B4-RB-2026Q1`** — להחיל פרופיל single‑lane, להריץ את checklist rollback, לאסוף ראיות טלמטריה ולהחזיר את חבילת Nexus.
7. **העלאת artefacts** — להעלות את חבילת הטלמטריה, עקבות Torii ו‑drill log ל‑Nexus evidence bucket; לקשר ב‑`docs/source/nexus_transition_notes.md`.
8. **Manifests/validation** — להריץ `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` כדי להפיק `telemetry_manifest.json`
   + `.sha256`, ואז לצרף את המניפסט ל‑tracker. הכלי מנרמל גבולות slots (נשמרים כמספרים שלמים)
   ונכשל מהר אם חסר אחד הרמזים, כדי לשמור על דטרמיניזם של artefacts governance.

## פלטים

- Checklist רהרסל חתום + drill log של אירועים.
- חבילת טלמטריה (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Manifest טלמטריה + digest שנוצר בסקריפט האימות.
- מסמך רטרוספקטיבה שמסכם חסמים, מיטיגציות והקצאות.

## סיכום ביצוע — Apr 9 2026

- הרהרסל בוצע 15:00 UTC–16:12 UTC עם seed `NEXUS-REH-2026Q1`; שלושת ה‑lanes החזיקו
  ~2.4k TEU לכל slot ו‑`nexus_lane_state_total` דיווח על envelopes מאוזנים.
- חבילת הטלמטריה נשמרה ב‑`artifacts/nexus/rehearsals/2026q1/` (כולל
  `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, incident log,
  ו‑rollback evidence). checksums נרשמו ב‑`docs/source/project_tracker/nexus_rehearsal_2026q1.md`.
- Drill rollback `B4-RB-2026Q1` הושלם ב‑16:18 UTC; פרופיל single‑lane הוחל מחדש
  בתוך 6m42s ללא lanes תקועים, ואז חבילת Nexus הופעלה מחדש לאחר אישור טלמטריה.
- אירוע רוויה בליין שהוזרק ב‑slot 842 (forced headroom clamp) הפעיל את ההתראות
  המצופות; playbook המיטיגציה סגר את ה‑page תוך 11m עם ציר זמן PagerDuty מתועד.
- לא נמצאו חסמים; משימות המשך (אוטומציה ל‑TEU headroom logging, סקריפט אימות חבילת טלמטריה)
  מתועדות ברטרו של Apr 15.

## הסלמה

- אירועים חוסמים או רגרסיות טלמטריה עוצרים את הרהרסל ומחייבים הסלמה לגובירננס בתוך 4 שעות עבודה.
- כל חריגה מחבילת config/genesis המאושרת מחייבת רהרסל מחדש לאחר אישור חוזר.

## אימות חבילת טלמטריה (הושלם)

הריצו `scripts/telemetry/validate_nexus_telemetry_pack.py` אחרי כל רהרסל כדי להוכיח
שהחבילה מכילה artefacts קנוניים (Prometheus export, OTLP NDJSON, Torii structured logs, rollback log)
וללכוד את digests ה‑SHA‑256. הכלי כותב `telemetry_manifest.json` והקובץ `.sha256` התואם
כדי שה‑governance תוכל לצטט hashes ישירות בחבילת הרטרו.

ב‑Apr 9 2026 המניפסט המאומת נמצא לצד artefacts תחת
`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` עם digest ב‑
`telemetry_manifest.json.sha256`. צרפו את שני הקבצים ל‑tracker בעת פרסום הרטרו.

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

העבירו `--require-slot-range` / `--require-workload-seed` ב‑CI כדי לחסום העלאות ללא
האנוטציות הללו. השתמשו ב‑`--expected <name>` כדי להוסיף artefacts נוספים (למשל DA receipts)
כאשר תכנית הרהרסל דורשת זאת.

</div>
