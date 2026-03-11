---
lang: ru
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3e859213a7d5cfddfefa688eb39686fe28560d9d061bdcd58b12d884116fa19
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: dispute-revocation-runbook
lang: he
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/dispute_revocation_runbook.md`. שמרו על סנכרון שתי הגרסאות עד שהמסמכים הישנים של Sphinx יוסרו.
:::

## מטרה

ראנבוק זה מנחה את מפעילי הממשל בהגשת סכסוכי קיבולת של SoraFS, בתיאום ביטולים ובהבטחת פינוי נתונים דטרמיניסטי.

## 1. הערכת האירוע

- **תנאי טריגר:** זיהוי הפרת SLA (זמינות/כשל PoR), מחסור ברפליקציה או מחלוקת על חיוב.
- **אישור טלמטריה:** לכידת snapshots של `/v1/sorafs/capacity/state` ו-`/v1/sorafs/capacity/telemetry` עבור הספק.
- **הודעה לבעלי עניין:** Storage Team (תפעול ספקים), Governance Council (גוף החלטה), Observability (עדכוני דשבורדים).

## 2. הכנת חבילת ראיות

1. אספו ארטיפקטים גולמיים (telemetry JSON, לוגים של CLI, הערות מבקר).
2. נרמלו לארכיון דטרמיניסטי (למשל tarball); תעדו:
   - digest BLAKE3-256 (`evidence_digest`)
   - סוג מדיה (`application/zip`, `application/jsonl`, וכדומה)
   - URI אירוח (object storage, pin של SoraFS, או endpoint נגיש דרך Torii)
3. אחסנו את החבילה בבאקט איסוף הראיות של הממשל עם גישת write-once.

## 3. הגשת הסכסוך

1. צרו JSON spec עבור `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. הריצו את ה-CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. בדקו את `dispute_summary.json` (אשרו סוג, digest של ראיות וחותמות זמן).
4. שלחו את ה-JSON של הבקשה ל-Torii `/v1/sorafs/capacity/dispute` דרך תור עסקאות הממשל. תעדו את ערך התגובה `dispute_id_hex`; הוא מעגן פעולות ביטול המשך ודוחות ביקורת.

## 4. פינוי וביטול

1. **חלון חסד:** הודיעו לספק על ביטול מתקרב; אפשרו פינוי נתונים מוצמדים כאשר המדיניות מאפשרת.
2. **יצירת `ProviderAdmissionRevocationV1`:**
   - השתמשו ב-`sorafs_manifest_stub provider-admission revoke` עם הסיבה שאושרה.
   - אמתו חתימות ו-digest הביטול.
3. **פרסום הביטול:**
   - שלחו את בקשת הביטול ל-Torii.
   - ודאו שה-adverts של הספק נחסמים (צפו לעלייה ב-`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **עדכון דשבורדים:** סמנו את הספק כמבוטל, ציינו את מזהה הסכסוך וקשרו את חבילת הראיות.

## 5. Post-mortem ומעקב

- תעדו את ציר הזמן, סיבת השורש ופעולות התיקון בטרקר האירועים של הממשל.
- קבעו השבה (slashing של stake, clawbacks של עמלות, החזרי לקוחות).
- תעדו לקחים; עדכנו ספי SLA או התראות ניטור במידת הצורך.

## 6. חומרי ייחוס

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (סעיף סכסוכים)
- `docs/source/sorafs/provider_admission_policy.md` (workflow ביטול)
- לוח תצפית: `SoraFS / Capacity Providers`

## רשימת בדיקה

- [ ] חבילת הראיות נאספה וחושבה.
- [ ] Payload הסכסוך אומת מקומית.
- [ ] עסקת הסכסוך ב-Torii התקבלה.
- [ ] בוצע ביטול (אם אושר).
- [ ] דשבורדים/ראנבוקים עודכנו.
- [ ] Post-mortem הוגש למועצת הממשל.
