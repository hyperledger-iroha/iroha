---
lang: he
direction: rtl
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/docs_preview_invite_email.md -->

# הזמנת תצוגה מקדימה לפורטל docs (דוא"ל לדוגמה)

השתמשו בדוגמה זו בעת ניסוח ההודעה היוצאת. היא משקפת את הטקסט המדויק שנשלח
למבקרי קהילת W2 (`preview-2025-06-15`) כך שגלים עתידיים יוכלו לשמר את הטון,
הנחיות האימות ושביל הראיות בלי לחטט בטיקטים ישנים. עדכנו קישורי artefacts,
האשים, מזהי בקשה ותאריכים לפני שליחת הזמנה חדשה.

```text
נושא: [DOCS-SORA] הזמנת תצוגה מקדימה לפורטל docs preview-2025-06-15 עבור Horizon Wallet

שלום Sam,

תודה שוב על הצעת Horizon Wallet ל-preview הקהילתי W2. גל W2 אושר, כך שאפשר להתחיל
בסיקור מיד לאחר ביצוע השלבים למטה. אנא שמרו על סודיות artefacts וטוקני הגישה:
כל הזמנה מתועדת ב-DOCS-SORA-Preview-W2 והמבקרים יאמתו את האישורים.

1. הורידו את ה-artefacts המאומתים (אותם bits ששילחנו ל-SoraFS ול-CI):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. אמתו את ה-bundle לפני חילוץ:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. הריצו את התצוגה עם enforcement של checksum:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. עברו על runbooks מחוזקים לפני הבדיקה:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. שלחו feedback דרך DOCS-SORA-Preview-REQ-C04 וסמנו כל ממצא עם
   `docs-preview/w2`. אם תרצו intake מובנה, השתמשו בטופס:
   docs/examples/docs_preview_feedback_form.md.

התמיכה זמינה ב-Matrix (`#docs-preview:matrix.org`) ויש office hours
ב-2025-06-18 15:00 UTC. להסלמות אבטחה או תקריות, פנו מיד ל-docs on-call
דרך ops@sora.org או +1-555-0109; אל תחכו ל-office hours.

הגישה ל-preview עבור Horizon Wallet בתוקף 2025-06-15 -> 2025-06-29. אנא הודיעו
כשתסיימו כדי שנבטל מפתחות גישה זמניים ונרשום סגירה ב-tracker.

תודה על העזרה לקדם את הפורטל ל-GA!

- צוות DOCS-SORA
```

</div>
