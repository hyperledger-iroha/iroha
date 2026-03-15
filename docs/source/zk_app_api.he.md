<!-- Hebrew translation of docs/source/zk_app_api.md -->

---
lang: he
direction: rtl
source: docs/source/zk_app_api.md
status: complete
translator: manual
---

<div dir="rtl">

# ממשק ZK App: קבצים מצורפים ודוחות פרובר (מדריך מפעיל)

המסמך מתאר את נקודות הקצה של Torii לטיפול בקבצים מצורפים ובדוחות פרובר ברקע. הממשק אינו קונצנזוסי: הוא לא משנה אימות, ביצוע או יצירת בלוקים. הייעוד הוא כלי מפעיל וזרימות UI/UX.

מאפיינים עיקריים:
- התנהגות דטרמיניסטית שאינה משפיעה על מזלגי קונצנזוס; השבתת העובד אינה משנה תוצאות.
- מוסתר תחת feature flag `app_api` (ברירת מחדל מופעל ב-Torii).
- מוגן ברייט-לימיטינג וטוקני API אופציונליים.
- אחסון דיסק כברירת מחדל תחת `./storage/torii`.

## קבצים מצורפים

מאחסנים הוכחות, JSON ועוד. לכל קובץ מזהה דטרמיניסטי.

נקודות קצה:
- `POST /v2/zk/attachments` — מעלה קובץ ומחזיר `{ id, size, content_type, created_ms }`.
- `GET  /v2/zk/attachments` — רשימת מטא-נתונים (תומך מסננים: `id`, ‏`content_type`, ‏`since_ms`, ‏`before_ms`, ‏`has_tag=<TAG>`, ‏`limit`, ‏`offset`, ‏`order`, ‏`ids_only`).
- `GET  /v2/zk/attachments/:id` — מוריד את הבייטים המאוחסנים לפי מזהה.
- `DELETE /v2/zk/attachments/:id` — מוחק קובץ ומטא-נתונים.
- `GET  /v2/zk/attachments/count` — `{ count }` עם מסננים זהים.
- `GET  /v2/zk/proof/{backend}/{hash}` — שליפת רשומת הוכחה (מחזיר JSON עם סטטוס, גובה אימות, VK וכו').
- `GET  /v2/zk/proofs` + `GET /count` — רשימה/ספירה עם מסננים (`backend`, ‏`status`, ‏`has_tag`, ‏`verified_from_height`, ‏`verified_until_height`, ‏`limit`, ‏`offset`, ‏`order`, ‏`ids_only`).

פרטים:
- מזהה = Blake2b-32 של גוף הבקשה לאחר סניטציה (hex).
- Content-Type מנורמל לפי sniffing; הסוג המוצהר נשמר ב-`provenance`.
- סניטציה: gzip/zstd מורחבים לפי `torii.attachments_max_expanded_bytes` ו-`torii.attachments_max_archive_depth`; מתקבלים רק סוגי MIME ב-`torii.attachments_allowed_mime_types`; מצב ריצה נקבע ב-`torii.attachments_sanitizer_mode`; יצוא של רשומות ישנות (ללא `provenance`) עובר סניטציה מחדש.
- גודל פר פריט מוגבל (`torii.attachments_max_bytes`, ברירת מחדל 4MiB).
- מכסה פר טננט (`torii.attachments_per_tenant_max_*`). טננט מזוהה לפי `X-API-Token` (hash `token:<blake32>`); ללא טוקן → `anon`. חריגה גוררת מחיקה דטרמיניסטית של קבצים ישנים.
- TTL: קבצים ישנים מ-`torii.attachments_ttl_secs` (ברירת מחדל 7 ימים) נמחקים כל ~60 שניות.
- מבנה אחסון: `storage/torii/zk_attachments/<id>.bin` + `.json`.

## דוחות פרובר ברקע

ה-worker (ברירת מחדל כבוי) סורק קבצים, מאמת מטעני `ProofAttachment`/`ProofAttachmentList`, ומפיק דוח JSON לכל קובץ. מעטפות ZK1/TLV אינן מתקבלות כ-payload עליון; הן רק מתויגות ומסומנות כ-`ok=false`:
- Norito (`application/x-norito`): חייב להתפענח כ-`ProofAttachment` או `ProofAttachmentList`.
- JSON (`application/json`, ‏`text/json`): חייב להתפענח כאובייקט `ProofAttachment`, ‏`ProofAttachmentList` (מחרוזת base64), או מערך `ProofAttachment`.
- אחרים: ניסיון JSON ואז Norito; כישלון → `ok: false`.

נקודות קצה:
- `GET /v2/zk/prover/reports` — רשימה (מסננים: `ok_only`, ‏`failed_only`, ‏`errors_only`, ‏`id`, ‏`content_type`, ‏`has_tag`, ‏`limit`, ‏`since_ms`, ‏`before_ms`, ‏`order`, ‏`offset`, ‏`latest`, ‏`ids_only`, ‏`messages_only`).
- `GET /v2/zk/prover/reports/:id` — דוח יחיד.
- `DELETE /v2/zk/prover/reports` — מחיקה המונית (מחזיר `{ deleted, ids }`).
- `DELETE /v2/zk/prover/reports/:id` — מחיקת דוח ספציפי.

הגדרות (Torii):
- `torii.zk_prover_enabled`, ‏`torii.zk_prover_scan_period_secs`, ‏`torii.zk_prover_reports_ttl_secs`.
- `torii.zk_prover_max_inflight`, ‏`torii.zk_prover_max_scan_bytes`, ‏`torii.zk_prover_max_scan_millis`.
- `torii.zk_prover_keys_dir`, ‏`torii.zk_prover_allowed_backends`, ‏`torii.zk_prover_allowed_circuits` (רשימות allowlist, prefix match).
- worker ממיין קבצים עודפים לפי יושן; אם קובץ בודד גדול מגבול הטננט → `413`.
- הדוחות נשמרים כ-JSON תחת `storage/torii/zk_prover/reports/`.

## CLI

`iroha app zk` מספק עטיפות.
- קבצים: `attach`, ‏`list-attachments`, ‏`download-attachment`, ‏`delete-attachment`.
- דוחות: `list-reports`, ‏`delete-reports`.
- פרובר: `prover start/stop/status`.
- בדיקות: `verify` / `submit-proof` עם `--json` או `--norito`.

## רישום מפתחות אימות

נקודות קצה נוחות לשליחת ISI:
- `POST /v2/zk/vk/{register,update,deprecate}`
- `GET /v2/zk/vk/{backend}/{name}`
- `GET /v2/zk/vk` (מסננים `backend`, ‏`status`, ‏`name_contains`, ‏`limit`, ‏`offset`, ‏`order`, ‏`ids_only`)

- `vk_bytes` (base64) — מצרף מפתח מלא; Torii יחושב את ה-commitment ויוודא מול `commitment_hex` אם נכלל. `vk_len` אופציונלי אך חייב להתאים לאורך הבייטים אם מוגדר.
- `commitment_hex` (hex באורך 64) — רישום לפי commitment בלבד; במקרה זה חובה לציין `vk_len` כדי לשמר מידע על אורך המפתח.

תשובת `GET` מחזירה אובייקט:

```json5
{
  "id": { "backend": "halo2/ipa", "name": "vk_main" },
  "record": {
    "version": 3,
    "circuit_id": "halo2/ipa::transfer_v3",
    "backend": "halo2-ipa-pasta",
    "curve": "pallas",
    "public_inputs_schema_hash": "…",
    "commitment": "…",
    "vk_len": 40960,
    "max_proof_bytes": 8192,
    "gas_schedule_id": "halo2_default",
    "metadata_uri_cid": "ipfs://…",
    "vk_bytes_cid": "ipfs://…",
    "activation_height": 1200,
    "deprecation_height": null,
    "withdraw_height": null,
    "status": "Active",
    "key": { "backend": "halo2/ipa", "bytes_b64": "..." }
  }
}
```

כאשר `ids_only=true` נקבל רק `{ "backend": "...", "name": "..." }`.
CLI: `iroha app zk vk register/update/deprecate/get`.

הערות:
- Commits הם hash מופרד (backend||bytes). נבדקים בעת שליחה.

### מנוי לאירועי רישום VK

`DataEventFilter.VerifyingKey` מאפשר Subscribe (JSON5). CLI לדוגמה:


### אירועי הוכחה

`DataEventFilter.Proof` ותסריטי CLI זמינים (רק מאומתים וכו').

## מגבלות ועבודה עתידית

- ה-worker אינו מריץ אלגוריתמי הוכחה; בעתיד ישולבו זרמי הוכחה/אימות תוך שמירה על non-forking.
- Retention לדוחות הוא ידני; קבצים מצורפים כבר עם TTL GC.
- API רשימה/ספירה להוכחות טרם קיים; עתידית יכלול פילטרים (`has_tag`) ודורש אינדוקס תגי ZK1.

## ממשקי ממשל (ZK Ballots)

עבור הצבעות ZK ראה Governance App API (`/v2/gov/ballots/zk*`). אם מספקים `private_key`, Torii חותמת ומגישה; אחרת מוחזר שלד ל-ISI לחתימה ושליחה.

</div>
