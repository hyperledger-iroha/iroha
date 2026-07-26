---
lang: ur
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/preview-integrity-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d03a803c6381a6f28be0b1d41ab9649da90a1bdc2269ecca9c1c06669e490c33
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# תוכנית תצוגה מקדימה עם בקרת checksum

תוכנית זו מפרטת את העבודה שנותרה כדי שכל ארטיפקט תצוגה מקדימה בפורטל יהיה ניתן לאימות לפני פרסום. המטרה היא להבטיח שסוקרים מורידים את הצילום שנבנה ב-CI בדיוק, שמניפסט ה-checksum בלתי ניתן לשינוי, ושניתן לגלות את התצוגה המקדימה דרך SoraFS עם מטא-דאטה של Norito.

## יעדים

- **בנויים דטרמיניסטיים:** להבטיח ש-`npm run build` מפיק פלט ניתן לשחזור ותמיד יוצר `build/checksums.sha256`.
- **תצוגות מקדימות מאומתות:** לדרוש שכל ארטיפקט תצוגה מקדימה יכלול מניפסט checksum ולסרב לפרסום כאשר האימות נכשל.
- **מטא-דאטה שפורסמה דרך Norito:** לשמר מתארי תצוגה מקדימה (מטא-דאטה של commit, digest checksum, CID של SoraFS) כ-Norito JSON כדי שכלי ממשל יוכלו לבצע ביקורת על שחרורים.
- **כלים למפעילים:** לספק סקריפט אימות חד-שלבי שהצרכנים יכולים להריץ מקומית (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); הסקריפט כעת עוטף את זרימת האימות של checksum + המתאר מקצה לקצה. פקודת התצוגה המקדימה הסטנדרטית (`npm run serve`) מפעילה כעת עוזר זה אוטומטית לפני `docusaurus serve` כך שצילומי מצב מקומיים יישארו מוגנים ב-checksum (כאשר `npm run serve:verified` נשמר כאליאס מפורש).

## שלב 1 — אכיפה ב-CI

1. לעדכן את `.github/workflows/docs-portal-preview.yml` כדי:
   - להריץ `node docs/portal/scripts/write-checksums.mjs` אחרי בניית Docusaurus (כבר מופעל מקומית).
   - לבצע `cd build && sha256sum -c checksums.sha256` ולהכשיל את ה-job במקרה של אי התאמה.
   - לארוז את תיקיית build כ-`artifacts/preview-site.tar.gz`, להעתיק את מניפסט ה-checksum, להריץ `scripts/generate-preview-descriptor.mjs`, ולהריץ `scripts/sorafs-package-preview.sh` עם תצורת JSON (ראו `docs/examples/sorafs_preview_publish.json`) כך שה-workflow יפיק גם מטא-דאטה וגם bundle SoraFS דטרמיניסטי.
   - להעלות את האתר הסטטי, ארטיפקטי המטא-דאטה (`docs-portal-preview`, `docs-portal-preview-metadata`), ואת bundle ה-SoraFS (`docs-portal-preview-sorafs`) כדי שניתן יהיה לבדוק את המניפסט, תקציר ה-CAR והתוכנית ללא בנייה מחדש.
2. להוסיף תגובת CI עם badge שמסכמת את תוצאת אימות ה-checksum ב-pull requests (ממומש באמצעות שלב תגובת GitHub Script ב-`docs-portal-preview.yml`).
3. לתעד את ה-workflow ב-`docs/portal/README.md` (סעיף CI) ולקשר את שלבי האימות בצ'ק-ליסט הפרסום.

## סקריפט אימות

`docs/portal/scripts/preview_verify.sh` מאמת ארטיפקטי תצוגה מקדימה שהורדו בלי צורך בהרצות ידניות של `sha256sum`. השתמשו ב-`npm run serve` (או באליאס המפורש `npm run serve:verified`) כדי להריץ את הסקריפט ולהפעיל `docusaurus serve` בצעד אחד כאשר משתפים צילומי מצב מקומיים. לוגיקת האימות:

1. מפעילה את כלי ה-SHA המתאים (`sha256sum` או `shasum -a 256`) מול `build/checksums.sha256`.
2. משווה אופציונלית את digest/שם הקובץ של מתאר התצוגה המקדימה `checksums_manifest`, ואם סופק - את digest/שם הקובץ של ארכיון התצוגה המקדימה.
3. מסיים בקוד לא אפס כאשר מתגלה אי התאמה כדי לאפשר לסוקרים לחסום תצוגות מקדימות שעברו שינוי.

דוגמת שימוש (לאחר חילוץ ארטיפקטי CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

מהנדסי CI ושחרור צריכים להריץ את הסקריפט בכל פעם שהם מורידים bundle תצוגה מקדימה או מצרפים ארטיפקטים לכרטיס שחרור.

## שלב 2 — פרסום ב-SoraFS

1. להרחיב את Workflow התצוגה המקדימה עם Job שמבצע:
   - העלאת האתר הבנוי ל-gateway של staging ב-SoraFS באמצעות `sorafs_cli car pack` ו-`manifest submit`.
   - לכידת digest המניפסט שהוחזר ו-CID של SoraFS.
   - סריאליזציה של `{ commit, branch, checksum_manifest, cid }` ל-Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. לשמור את המתאר לצד ארטיפקט הבנייה ולהציג את ה-CID בתגובת ה-pull request.
3. להוסיף בדיקות אינטגרציה שמריצות `sorafs_cli` במצב dry-run כדי להבטיח ששינויים עתידיים ישמרו על תאימות סכמת המטא-דאטה.

## שלב 3 — ממשל וביקורת

1. לפרסם סכמת Norito (`PreviewDescriptorV1`) המתארת את מבנה המתאר תחת `docs/portal/schemas/`.
2. לעדכן את צ'ק-ליסט הפרסום DOCS-SORA כדי לדרוש:
   - להריץ `sorafs_cli manifest verify` מול ה-CID שהועלה.
   - לתעד את digest מניפסט ה-checksum ואת ה-CID בתיאור ה-PR של השחרור.
3. לחבר את אוטומציית הממשל כדי להצליב את המתאר עם מניפסט ה-checksum במהלך הצבעות שחרור.

## תוצרים ובעלות

| אבן דרך | בעלים | יעד | הערות |
|---------|-------|-----|-------|
| אכיפת checksum ב-CI הושלמה | תשתית Docs | שבוע 1 | מוסיף שער כשל והעלאות ארטיפקטים. |
| פרסום תצוגות מקדימות ב-SoraFS | תשתית Docs / צוות Storage | שבוע 2 | דורש גישה לאישורי staging ועדכוני סכמת Norito. |
| שילוב ממשל | מוביל Docs/DevRel / WG ממשל | שבוע 3 | מפרסם סכימה ומעדכן צ'ק-ליסטים ופריטי מפת דרכים. |

## שאלות פתוחות

- איזו סביבת SoraFS צריכה לאחסן ארטיפקטי תצוגה מקדימה (staging לעומת lane ייעודי לתצוגה מקדימה)?
- האם נדרשות חתימות כפולות (Ed25519 + ML-DSA) על מתאר התצוגה המקדימה לפני פרסום?
- האם Workflow ה-CI צריך לנעול את הגדרת ה-orchestrator (`orchestrator_tuning.json`) בעת הרצת `sorafs_cli` כדי לשמור על מניפסטים ניתנים לשחזור?

רשמו החלטות ב-`docs/portal/docs/reference/publishing-checklist.md` ועדכנו תוכנית זו לאחר פתרון הסוגיות.
