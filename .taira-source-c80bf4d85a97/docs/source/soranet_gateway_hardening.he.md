---
lang: he
direction: rtl
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- התרגום העברי لـ docs/source/soranet_gateway_hardening.md -->

# הקשחת SoraGlobal Gateway ‏(SNNet-15H)

עוזר ההקשחה אוסף ראיות אבטחה/פרטיות לפני קידום בניות ה‑Gateway.

## פקודה
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## פלטים
- `gateway_hardening_summary.json` — סטטוס לכל קלט (SBOM, דוח חולשות, מדיניות HSM, פרופיל sandbox) וכן אות שימור. קלטים חסרים מסומנים כ־`warn` או `error`.
- `gateway_hardening_summary.md` — סיכום קריא לאדם עבור חבילות ממשל.

## הערות קבלה
- דוחות SBOM וחולשות חייבים להיות קיימים; קלטים חסרים מורידים את הסטטוס.
- שימור מעל 30 ימים מסמן `warn` לבדיקה; יש לספק ברירות מחדל מחמירות יותר לפני GA.
- השתמשו בארטיפקטי הסיכום כקבצים מצורפים לביקורות GAR/SOC ולנהלי תקריות.

</div>
