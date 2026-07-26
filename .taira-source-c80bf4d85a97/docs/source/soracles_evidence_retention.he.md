---
lang: he
direction: rtl
source: docs/source/soracles_evidence_retention.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3121ec15db54ca27fab0f0e11a5780839b69eb46c7fa70994d97a6116cc28cf1
source_last_modified: "2026-01-04T10:50:53.653513+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soracles_evidence_retention.md -->

# Soracles Evidence Retention & GC

פריט רודמאפ OR-14 דורש מדיניות retention ניתנת לביקורת עבור ארטיפקטי ראיות
של oracle וכלים לגזום חבילות ישנות מבלי לשנות hashes על-chain. מסמך זה מתעד
את ברירות המחדל ואת helper `iroha soracles evidence-gc` שנוסף יחד עם מטא-דטה
ב-`bundle.json` שמתעד מתי חבילה נוצרה.

## מדיניות retention

- **Observations, reports, connector responses, telemetry:** שומרים כברירת
  מחדל **180 ימים**. זה משמר את מסלול ingestion/audit המלא לאורך שני רבעונים
  תוך שמירה על אחסון אופרטורים מוגבל. התאימו את דגלי ה-GC אם מדיניות הממשל
  דורשת חלון קצר/ארוך יותר.
- **Dispute evidence:** שומרים **365 ימים** כך שמחלוקות שנפתחות מחדש או
  attestations איטיים עדיין ניתנים לאימות. ה-helper של GC שומר חבילות שמכילות
  ארטיפקטים של dispute תחת חלון **`--dispute-retention-days`** ארוך כברירת מחדל.
- **Hashes on-chain נשארים בלתי-משתנים.** `FeedEventRecord` שומר רק
  `evidence_hashes`; גיזום ארטיפקטים לעולם לא נוגע במצב הלדג'ר. צרפו דוחות GC
  לחבילות הממשל כאשר ארטיפקטים מוסרים כדי שה-audit trail יישאר שלם.
- **חותמת זמן לחבילה:** `bundle.json` כולל כעת `generated_at_unix` (שניות).
  GC מעדיף את החותמת הזו וחוזר ל-`mtime` כאשר היא חסרה.

## הרצת garbage collection

השתמשו ב-helper החדש ב-CLI כדי לגזום חבילות ישנות מחלון ה-retention ולמחוק
אופציונלית קבצים שאינם מוזכרים בארטיפקטים:

```bash
iroha soracles evidence-gc \
  --root artifacts/soracles \
  --retention-days 180 \
  --dispute-retention-days 365 \
  --prune-unreferenced \
  --report artifacts/soracles/gc_report.json
```

דגלים:
- `--root`: ספרייה שמכילה תיקיות חבילות (כל אחת עם `bundle.json`).
- `--retention-days`: להסיר חבילות ש-`generated_at_unix` (או `mtime` כאשר חסר)
  ישן מחלון ה-retention.
- `--dispute-retention-days`: חלון retention מינימלי לחבילות שמכילות ראיות
  dispute (ברירת מחדל **365**). השתמשו בו כדי לשמור payloads של dispute זמן
  ארוך יותר מאשר observations/reports.
- `--prune-unreferenced`: למחוק קבצים תחת `artifact_root` שאינם מוזכרים ב-
  `bundle.json`.
- `--dry-run`: לדווח מה היה נמחק מבלי למחוק דבר.
- `--report`: נתיב אופציונלי לסיכום JSON; ברירת מחדל `<root>/gc_report.json`.

הדוח כולל `removed_bundles`, `pruned_files`, `skipped_bundles`,
`retained_bundles`, `bytes_freed`, חלון ה-retention והאם הריצה הייתה dry-run.
צרפו את הדוח לצד הראיות שנותרו בעת העלאה ל-SoraFS כדי שמבקרי ממשל יוכלו לעקוב
מדוע ארטיפקטים מסוימים נגזמו.

## כללי ניקוי on-chain

- evidence hashes על-chain קבועים; GC מסיר רק עותקים off-chain.
- בעת גיזום, השאירו את `evidence_hashes` המקוריים כפי שהם וצרפו את דוח ה-GC
  לחבילת הממשל או ל-bundle ב-SoraFS. כך עומדים בדרישת האימוטביליות תוך שמירה
  על אחסון רזה.
- אם חבילה צריכה להתפרסם מחדש לאחר גיזום, שמרו על שמות ארטיפקט hashed זהים
  (או צרפו manifest של hashes שנגזמו) כך שמאמתים יבינו אילו רשומות חסרות בכוונה.

</div>
