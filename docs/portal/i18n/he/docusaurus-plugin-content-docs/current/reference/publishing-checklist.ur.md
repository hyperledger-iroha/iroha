---
lang: ur
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: af4fb2963bf312cf488a35422f04a99c7bffd6acac0d47eb2bb65288a5c85e30
source_last_modified: "2025-11-04T12:03:18.019059+00:00"
translation_last_reviewed: 2026-01-30
---

# צ'ק-ליסט לפרסום

השתמשו בצ'ק-ליסט הזה בכל פעם שאתם מעדכנים את פורטל המפתחים. הוא מבטיח שבניית ה-CI, פריסת GitHub Pages ובדיקות smoke ידניות מכסות כל סעיף לפני release או אבן דרך של roadmap.

## 1. ולידציה מקומית

- `npm run sync-openapi -- --version=current --latest` (הוסיפו אחד או יותר flags `--mirror=<label>` כאשר Torii OpenAPI משתנה עבור snapshot קפוא).
- `npm run build` – ודאו שהטקסט של ה-hero `Build on Iroha with confidence` עדיין מופיע ב-`build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – בדקו את manifeste ה-checksums (הוסיפו `--descriptor`/`--archive` בעת בדיקת artefacts שהורדו מ-CI).
- `npm run serve` – מפעיל את helper ה-preview שמוגן ב-checksum ומוודא את המניפסט לפני קריאה ל-`docusaurus serve`, כדי שה-reviewers לא יגלשו snapshot לא חתום (ה-alias `serve:verified` נשאר לקריאות מפורשות).
- בצעו spot-check ל-markdown שנגעתם בו דרך `npm run start` ושרת ה-live reload.

## 2. בדיקות pull request

- ודאו שה-job `docs-portal-build` הצליח ב-`.github/workflows/check-docs.yml`.
- אשרו ש-`ci/check_docs_portal.sh` רץ (לוגי CI מציגים את hero smoke check).
- ודאו ש-workflow ה-preview העלה manifeste (`build/checksums.sha256`) וש-script ה-verify של ה-preview הצליח (הלוגים מציגים את הפלט של `scripts/preview_verify.sh`).
- הוסיפו את כתובת ה-URL של preview שפורסמה מסביבת GitHub Pages לתיאור ה-PR.

## 3. אישור לפי סעיף

| Section | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | טקסט ה-hero מוצג, כרטיסי quickstart מקשרים לנתיבים תקינים, וכפתורי CTA נפתרים. |
| Norito | Norito WG | מדריכי overview ו-getting-started מפנים לדגלי ה-CLI העדכניים ולתיעוד סכימת Norito. |
| SoraFS | Storage Team | ה-quickstart רץ עד הסוף, שדות דו"ח ה-manifest מתועדים, והוראות סימולציית fetch מאומתות. |
| SDK guides | SDK leads | מדריכי Rust/Python/JS מקמפלים את הדוגמאות העדכניות ומקשרים ל-repos חיים. |
| Reference | Docs/DevRel | האינדקס כולל את ה-specs העדכניים ביותר, ורפרנס הקודק Norito תואם ל-`norito.md`. |
| Preview artifact | Docs/DevRel | ה-artefact `docs-portal-preview` מצורף ל-PR, ה-smoke checks עוברים, והקישור שותף עם reviewers. |
| Security & Try it sandbox | Docs/DevRel · Security | OAuth device-code login מוגדר (`DOCS_OAUTH_*`), checklist `security-hardening.md` בוצע, וכותרות CSP/Trusted Types אומתו דרך `npm run build` או `npm run probe:portal`. |

סמנו כל שורה כחלק מבדיקת ה-PR, או ציינו משימות המשך כדי שמעקב הסטטוס יישאר מדויק.

## 4. הערות release

- כללו את `https://docs.iroha.tech/` (או כתובת הסביבה מתוך job הפריסה) בהערות ה-release ובעדכוני הסטטוס.
- ציינו במפורש כל סעיף חדש או שהשתנה כדי שהצוותים downstream ידעו היכן להריץ שוב את בדיקות ה-smoke שלהם.
