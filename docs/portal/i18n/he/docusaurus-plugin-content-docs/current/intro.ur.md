---
lang: ur
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4767e58dc0377d6178e7181f45923a8e1ef824549cf521720b587f47dcf65d49
source_last_modified: "2026-01-03T18:07:58+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/intro.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# ברוכים הבאים לפורטל המפתחים של SORA Nexus

הפורטל למפתחים של SORA Nexus מרכז תיעוד אינטראקטיבי, מדריכי SDK והפניות API עבור מפעילי Nexus ותורמי Hyperledger Iroha. הוא משלים את אתר התיעוד הראשי בכך שהוא חושף מדריכים מעשיים ומפרטים שנוצרים ישירות מהמאגר הזה. עמוד הנחיתה כולל כעת נקודות כניסה ממותגות ל-Norito/SoraFS, snapshots של OpenAPI חתומים, והפניה ייעודית ל-Norito Streaming כדי שתורמים יוכלו למצוא את חוזה מישור הבקרה של ה-streaming בלי לחטט במפרט הבסיסי.

## מה אפשר לעשות כאן

- **ללמוד Norito** - התחילו עם הסקירה וה-quickstart כדי להבין את מודל הסריאליזציה וכלי ה-bytecode.
- **להרים SDKs** - עקבו אחרי ה-quickstarts של JavaScript ו-Rust כבר עכשיו; מדריכי Python, Swift ו-Android יצטרפו כאשר המתכונים יועברו.
- **לעיין בהפניות API** - עמוד OpenAPI של Torii מציג את מפרט ה-REST העדכני, וטבלאות התצורה מקשרות למקורות ה-Markdown הקנוניים.
- **להכין פריסות** - runbooks תפעוליים (telemetry, settlement, Nexus overlays) מועברים מ-`docs/source/` ויגיעו לכאן עם התקדמות ההגירה.

## מצב נוכחי

- ✅ עמוד נחיתה ממותג של Docusaurus v3 עם טיפוגרפיה מרועננת, hero/cards מונעי גרדיאנטים, ואריחי משאבים הכוללים את סיכום Norito Streaming.
- ✅ תוסף OpenAPI של Torii מחובר ל-`npm run sync-openapi`, עם בדיקות snapshots חתומים והגנות CSP שמיושמות על ידי `buildSecurityHeaders`.
- ✅ כיסוי preview ו-probe רץ ב-CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), וכעת משמש כשער לדוקומנט streaming, ל-quickstarts של SoraFS ול-checklists של reference לפני פרסום ארטיפקטים.
- ✅ quickstarts של Norito, SoraFS ו-SDK יחד עם מקטעי reference חיים בסרגל הצד; יבואי `docs/source/` חדשים (streaming, orchestration, runbooks) נוחתים כאן כאשר הם נכתבים.

## איך להשתתף

- ראו `docs/portal/README.md` עבור פקודות פיתוח מקומי (`npm install`, `npm run start`, `npm run build`).
- משימות הגירת התוכן מתועדות לצד פריטי roadmap `DOCS-*`. תרומות יתקבלו בברכה - העבירו מקטעים מתוך `docs/source/` והוסיפו את העמוד לסרגל הצד.
- אם אתם מוסיפים ארטיפקט שנוצר (specs, טבלאות תצורה), תעדו את פקודת ה-build כדי שתורמים עתידיים יוכלו לרענן אותו בקלות.
