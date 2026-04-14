<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## ספר הפעלה של QR לא מקוון

ספר הפעלה זה מגדיר הגדרות קבועות מראש של `ecc`/ממד/fps עבור מצלמה רועשת
סביבות בעת שימוש בהובלת QR לא מקוונת.

### הגדרות מומלצות מראש

| סביבה | סגנון | ECC | מימד | FPS | גודל נתח | קבוצת זוגיות | הערות |
| --- | --- | --- | --- | --- | --- | --- | --- |
| תאורה מבוקרת, טווח קצר | `sakura` | `M` | `360` | `12` | `360` | `0` | התפוקה הגבוהה ביותר, יתירות מינימלית. |
| רעש מצלמה נייד אופייני | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | הגדרה מראש מאוזנת מועדפת (`~3 KB/s`) עבור מכשירים מעורבים. |
| בוהק גבוה, טשטוש תנועה, מצלמות נמוכות | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | תפוקה נמוכה יותר, עמידות הפענוח החזקה ביותר. |

### קידוד/פענוח רשימת ביקורת

1. מקודד עם ידיות הובלה מפורשות.
2. אמת עם לכידת לולאת סורק לפני ההפצה.
3. הצמד את אותו פרופיל סגנון בעזרי השמעת SDK כדי לשמור על שוויון תצוגה מקדימה.

דוגמה:

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### אימות לולאת סורק (פרופיל sakura-storm 3 KB/s)

השתמש באותו פרופיל תחבורה בכל נתיבי הלכידה:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

יעדי אימות:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- אנדרואיד: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- דפדפן/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

קבלה:

- שחזור מטען מלא מצליח עם מסגרת נתונים אחת שנשמטה לכל קבוצת זוגיות.
- אין חוסר התאמה של סכום בדיקה/-payload-hash בלולאת הלכידה הרגילה.