---
lang: he
direction: rtl
source: docs/examples/docs_preview_invite_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/docs_preview_invite_template.md -->

# הזמנת תצוגה מקדימה לפורטל docs (תבנית)

השתמשו בתבנית זו בעת שליחת הוראות גישה לתצוגת preview למבקרים. החליפו את ה-placeholders
(`<...>`) בערכים הרלוונטיים, צרפו את ה-artefacts של descriptor + archive המוזכרים
בהודעה, ושמרו את הטקסט הסופי בכרטיס intake המתאים.

```text
נושא: [DOCS-SORA] הזמנת תצוגה מקדימה לפורטל docs <preview_tag> עבור <reviewer/org>

שלום <name>,

תודה שהתנדבתם לסקור את פורטל docs לפני GA. אושרתם לגל <wave_id>. נא בצעו את השלבים
הבאים לפני הגלישה לתצוגה המקדימה:

1. הורידו את ה-artefacts המאומתים מ-CI או SoraFS:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. הריצו את שער ה-checksum:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. הריצו את התצוגה עם enforcement של checksum מופעל:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. קראו את ההערות על acceptable-use, security ו-observability:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. הגישו feedback דרך <request_ticket> וסמנו כל ממצא ב-`<preview_tag>`.

תמיכה זמינה ב-<contact_channel>. תקריות או סוגיות אבטחה חייבות להיות מדווחות מיד דרך
<incident_channel>. אם אתם צריכים טוקנים ל-Torii API, בקשו אותם דרך הטיקט; אל תמחזרו
קרדנצ'לים של פרודקשן.

הגישה לתצוגה המקדימה תפוג ב-<end_date> אלא אם תוארך בכתב. אנו מתעדים checksums ומטאדטה
של ההזמנה לצרכי governance; עדכנו אותנו כשסיימתם כדי שנוכל להסיר את הגישה בצורה נקיה.

תודה שוב על העזרה לייצב את הפורטל!

- צוות DOCS-SORA
```

</div>
