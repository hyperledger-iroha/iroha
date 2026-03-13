<!-- Hebrew translation of docs/source/zk/lifecycle.md -->

---
lang: he
direction: rtl
source: docs/source/zk/lifecycle.md
status: complete
translator: manual
---

<div dir="rtl">

# מחזור החיים של מפתחות אימות והוכחות

מסמך זה מתאר כיצד מפתחות אימות (VK) ומעטפות הוכחות אפס-ידע נעים בתוך Iroha v2. המטרה היא לרכז במקום אחד את מצב הרשת, נקודות הקצה של אפליקציית Torii וכלי ה-CLI כדי שמפעילים ומחברי SDK יקבלו במהירות תמונה שלמה.

## הזרימה ברמת העל

1. **יצירת VK סמכותי** - מפעיל או תוצר קומפיילר מייצר מפתח אימות והתחייבות באורך 32 בתים. המפתחות מנוהלים במרחב שמות לפי מנוע (`backend::name`).
2. **קבלה דרך Torii** - מפעילים מגישים פקודות ממשל חתומות אל Torii (`/v2/zk/vk/*`). עסקאות שהתקבלו רושמות או מעדכנות את רשומת ה-VK, נשמרות על השרשרת ומסונכרנות בין העמיתים.
3. **שימוש בחוזה/בריצה** - עסקאות וחוזים חכמים מפנים ל-VK באמצעות `(backend, name)` או מטמיעים מטען VK אינליין. בזמן ההרצה ההתחייבות נפתרת כחלק מאימות ההוכחה.
4. **אימות הוכחות** - לקוחות שולחים הוכחות ל-`/v2/zk/verify` או `/v2/zk/submit-proof`. האימות מתבצע בתוך `iroha_core::zk` במהלך ביצוע העסקה, והצלחה מייצרת `ProofRecord` שניתן לשאול דרך Torii (`/v2/zk/proofs*`).
5. **דיווח ברקע** - תהליך הפרובר האופציונלי של Torii (`torii.zk_prover_enabled=true`) סורק קבצים מצורפים, מאמת מטעני `ProofAttachment`, ומפרסם טלמטריה על גדלי ההוכחות ועל השהיית העיבוד. דוחות נמחקים אוטומטית לאחר ה-TTL המוגדר.

## מפתחות אימות

- רשומות מאוחסנות במצב-העולם תחת `verifying_keys[(backend, name)]`.
- בעת רישום VK חדש ההתחייבות חייבת להתאים למטען המגובה או לבייטים האינליין שנשלחו בעסקה. עדכון מעלה את הגרסה וחייב לשמור על סדר עולה.

### נקודות קצה רלוונטיות

- `POST /v2/zk/vk/register` - שליחת פקודת `RegisterVerifyingKey` חתומה.
- `POST /v2/zk/vk/update` - שליחת `UpdateVerifyingKey` עם גרסה גבוהה יותר.
- `POST /v2/zk/vk/deprecate` - סימון VK קיים כמדוכא.
- `GET  /v2/zk/vk` - רשימת VK עם פילטרים אופציונליים (`backend`, `status`, `name_contains`).
- `GET  /v2/zk/vk/{backend}/{name}` - שליפת רשומת VK בודדת.

### כלי CLI

`iroha_cli app zk` מספק עטיפות דקות ששולחות את ה-DTO ב-JSON ש-Torii מצפה לקבל:

- `iroha_cli app zk vk register --json path/to/register.json`
- `iroha_cli app zk vk update --json path/to/update.json`
- `iroha_cli app zk vk deprecate --json path/to/deprecate.json`
- `iroha_cli app zk vk get --backend halo2/ipa --name vk_transfer`

מבני ה-DTO ב-JSON תואמים בדיוק למטענים של `iroha_data_model::proof`. בייטים של VK אינליין נשמרים בבסיס64, והתחייבויות נשלחות כמחרוזת הקסדצימלית באותיות קטנות.

## מחזור החיים של ההוכחה

### הגשה ואימות

- מעטפות הוכחה מתקבלות ב-`/v2/zk/verify` (סינכרוני) או `/v2/zk/submit-proof` (לבדיקה מאוחרת). שתי הנקודות מקבלות מעטפות בקידוד Norito או DTO ב-JSON.
- בזמן ביצוע העסקה `iroha_core::smartcontracts::isi::zk::VerifyProof` מחשב גיבוב של בייטי ההוכחה יחד עם שם המנוע, מפיק `ProofId` ומבטיח ייחודיות על פני הלדג'ר.
- המאמת פותר התחייבויות VK מתוך מטען אינליין, מתוך `(backend, name)` שאליו מפנים או משילוב של השניים. מנועים שנרשמו תחת `debug/*` מדלגים על בדיקות קריפטוגרפיות למטרות פיתוח.
- התוצאה היא ש-`ProofRecord` שומר:
  - `backend` ו-`proof_hash`
  - `status` (`Submitted`, `Verified`, `Rejected`)
  - `verified_at_height` (גובה הבלוק שבו הושלם האימות)
  - `vk_ref` ו-`vk_commitment` אופציונליים
- מעטפות ZK1/TLV נבדקות בזמן האימות. תגיות מוכרות באורך 4 בתים נרשמות בצורה עצלה כדי לאפשר שאילתות מבוססות תגיות.

### ממשק השאילתות

`/v2/zk/proofs` ו-`/v2/zk/proofs/count` חושפים את הרשומות הרלוונטיות מהלדג'ר.

- פילטרים זמינים: `backend`, `status`, `has_tag`, `offset`, `limit`, `order=asc|desc`, `ids_only`.
- סינון לפי תגיות יעיל: התגיות מאונדקסות בזמן האימות ומוגשות מתוך אינדקס ייעודי של `(tag → proof ids)`.
- `ids_only=true` מחזיר אובייקטים בעלי `{ backend, hash }` בלבד לצורך דפדוף קל.
- `/v2/zk/proof/{backend}/{hash}` נשאר זמין לשליפות ישירות.

### כיסוי CLI

תחת `iroha_cli app zk proofs` נוספו פקודות משנה חדשות:

- `iroha_cli app zk proofs list [--backend halo2/ipa] [--status Verified] [--has-tag PROF] [--limit 20]`
- `iroha_cli app zk proofs count [--backend halo2/ipa] [--has-tag IPAK]`
- `iroha_cli app zk proofs get --backend halo2/ipa --hash 0123...`

כל הפקודות מחזירות תגובות Norito ב-JSON. הפילטרים תואמים אחד-לאחד לפרמטרי השאילתה של HTTP, כך שקל לסקריפט עימם דפדוף או להזין אותם לכלי ניטור.

## פרובר ברקע וטלמטריה

- נשלט באמצעות `torii.zk_prover_enabled`, ‏`torii.zk_prover_scan_period_secs`, ‏`torii.zk_prover_reports_ttl_secs`, ‏`torii.zk_prover_max_inflight`, ‏`torii.zk_prover_max_scan_bytes`, ‏`torii.zk_prover_max_scan_millis`, ‏`torii.zk_prover_keys_dir`, ‏`torii.zk_prover_allowed_backends`, ‏`torii.zk_prover_allowed_circuits` בקובצי `iroha_config`.
- קבצים מצורפים חייבים להתפענח כ-`ProofAttachment`/`ProofAttachmentList` (Norito או JSON). מעטפות ZK1/TLV מתויגות אך נדחות כ-payload עליון.
- מנועים מאושרים לפי prefix; ברירת מחדל `["halo2/"]`. ‏`groth16/…` ו-`stark/…` אינם נתמכים בבילדים פרודקשן.
- כל דוח רושם `latency_ms = processed_ms - created_ms`, כדי שמפעילים יוכלו לעקוב אחרי השהיית התור.
- הטלמטריה שמפורסמת:
  - `torii_zk_prover_attachment_bytes` (היסטוגרמה עם תווית `content_type`)
  - `torii_zk_prover_latency_ms` (היסטוגרמה)
  - `torii_zk_prover_inflight` (מונה מצב) ו-`torii_zk_prover_pending` (מונה מצב)
  - `torii_zk_prover_last_scan_bytes` ו-`torii_zk_prover_last_scan_ms` (מונה מצב)
  - `torii_zk_prover_budget_exhausted_total{reason}` (מונה)
  - `zk_verify_latency_ms` ו-`zk_verify_proof_bytes` (היסטוגרמות עם תווית `backend`)
- הטלמטריה זמינה תחת `/metrics` כאשר פרופיל הטלמטריה מאפשר חשיפה של מדדים.
- דוחות שגילם עבר את ה-TTL נמחקים בכל סריקה. מחיקה ידנית זמינה גם דרך `/v2/zk/prover/reports`.

הרצות Nightly Milestone 0 אוספות את ההיסטוגרמות החדשות ומפרסמות סיכומים לצד לוח המחוונים הקיים של מפעילי Torii, כך שאפשר לזהות במהירות רגרסיות בזמן האימות של ההוכחות.

</div>
