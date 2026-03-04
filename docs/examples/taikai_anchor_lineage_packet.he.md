---
lang: he
direction: rtl
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/taikai_anchor_lineage_packet.md -->

# חבילת שושלת עוגן Taikai (תבנית) (SN13-C)

פריט מפת הדרכים **SN13-C - Manifests & SoraNS anchors** מחייב שכל סיבוב alias יספק
חבילת ראיות דטרמיניסטית. העתיקו את התבנית הזו אל תיקיית artefacts של ה-rollout (לדוגמה
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) והחליפו את ה-placeholders
לפני הגשת החבילה לממשל.

## 1. מטא-נתונים

| שדה | ערך |
|-----|-----|
| מזהה אירוע | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Namespace / שם alias | `<sora / docs>` |
| תיקיית ראיות | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| איש קשר מפעיל | `<name + email>` |
| כרטיס GAR / RPT | `<governance ticket or GAR digest>` |

## Helper לחבילה (אופציונלי)

העתיקו artefacts מה-spool והפיקו סיכום JSON (אופציונלית חתום) לפני מילוי שאר הסעיפים:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

ה-helper מושך `taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`, envelopes
ו-sentinels מתיקיית spool של Taikai (`config.da_ingest.manifest_store_dir/taikai`), כך
שתיקיית הראיות כבר מכילה את הקבצים המדויקים המוזכרים למטה.

## 2. לֶג'ר שושלת ו-hint

צרפו גם את לֶג'ר השושלת על הדיסק וגם את JSON ה-hint ש-Torii כתב לחלון הזה. הם נלקחים ישירות
מ-
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` ו-
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefact | קובץ | SHA-256 | הערות |
|----------|-------|---------|-------|
| לֶג'ר שושלת | `taikai-trm-state-docs.json` | `<sha256>` | מוכיח את digest/החלון של המניפסט הקודם. |
| Hint שושלת | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | נתפס לפני העלאה לעוגן SoraNS. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. לכידת payload של העוגן

תעדו את payload ה-POST ש-Torii שלח לשירות העוגן. ה-payload כולל `envelope_base64`,
`ssm_base64`, `trm_base64` והאובייקט inline `lineage_hint`; הביקורות מסתמכות על לכידה זו
כדי להוכיח את ה-hint שנשלח ל-SoraNS. Torii כותב כעת JSON זה אוטומטית בשם
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
בתוך תיקיית ה-spool של Taikai (`config.da_ingest.manifest_store_dir/taikai/`), כך שהמפעילים
יכולים להעתיק אותו ישירות במקום לגרד לוגים של HTTP.

| Artefact | קובץ | SHA-256 | הערות |
|----------|-------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | בקשה גולמית שהועתקה מ-`taikai-anchor-request-*.json` (Taikai spool). |

## 4. אישור digest של המניפסט

| שדה | ערך |
|-----|-----|
| digest מניפסט חדש | `<hex digest>` |
| digest מניפסט קודם (מתוך hint) | `<hex digest>` |
| תחילת/סוף חלון | `<start seq> / <end seq>` |
| חותמת זמן קבלה | `<ISO8601>` |

הפנו ל-hashes של ledger/hint שנרשמו למעלה כדי ש-reviewers יוכלו לאמת את החלון שהוחלף.

## 5. מדדים / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (לפי alias): `<file path + hash>`

ספקו export של Prometheus/Grafana או פלט `curl` שמראה את עליית המונה ואת מערך `/status`
עבור alias זה.

## 6. מניפסט לתיקיית הראיות

צרו מניפסט דטרמיניסטי לתיקיית הראיות (spool files, payload capture, metric snapshots) כדי
שהממשל יוכל לאמת כל hash בלי לפרוס את הארכיון.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefact | קובץ | SHA-256 | הערות |
|----------|-------|---------|-------|
| מניפסט ראיות | `manifest.json` | `<sha256>` | צרפו לחבילת הממשל / GAR. |

## 7. צ'קליסט

- [ ] לֶג'ר השושלת הועתק + בוצע hash.
- [ ] Hint השושלת הועתק + בוצע hash.
- [ ] payload של Anchor POST נתפס ובוצע hash.
- [ ] טבלת digest של המניפסט מולאה.
- [ ] Snapshot-ים של מדדים יוצאו (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] נוצר מניפסט באמצעות `scripts/repo_evidence_manifest.py`.
- [ ] החבילה הועלתה לממשל עם hashes + פרטי קשר.

תחזוקה של תבנית זו לכל סיבוב alias שומרת על חבילת הממשל של SoraNS ניתנת לשחזור וקושרת את
hints השושלת ישירות לראיות GAR/RPT.

</div>
