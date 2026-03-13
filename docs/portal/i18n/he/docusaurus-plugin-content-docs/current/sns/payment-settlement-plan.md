---
id: payment-settlement-plan
lang: he
direction: rtl
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# תוכנית תשלומים וסליקה של SNS

> מקור קנוני: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

משימת ה-roadmap **SN-5 -- Payment & Settlement Service** מציגה שכבת תשלום
דטרמיניסטית עבור Sora Name Service. כל רישום, חידוש או החזר חייב להנפיק
payload Norito מובנה כדי שהאוצרות, stewards והממשל יוכלו לשחזר את הזרימות
הכספיות בלי גליונות. עמוד זה מתמצת את המפרט לקהל הפורטל.

## מודל הכנסות

- דמי הבסיס (`gross_fee`) נגזרים ממטריצת המחירים של registrar.
- האוצרות מקבל `gross_fee x 0.70`, וה-stewards מקבלים את היתרה פחות בונוס
  referral (עד 10 %).
- holdbacks אופציונליים מאפשרים לממשל לעצור תשלומי steward במהלך מחלוקות.
- חבילות settlement חושפות בלוק `ledger_projection` עם ISIs מסוג `Transfer`
  כך שאוטומציה יכולה לפרסם תנועות XOR ישירות ל-Torii.

## שירותים ואוטומציה

| רכיב | מטרה | ראיה |
|------|------|------|
| `sns_settlementd` | מיישם מדיניות, חותם על bundles, ומציג `/v2/sns/settlements`. | JSON bundle + hash. |
| Settlement queue & writer | תור idempotent + שולח ledger מונע על ידי `iroha_cli app sns settlement ledger`. | bundle hash <-> tx hash manifest. |
| Reconciliation job | diff יומי + דוח חודשי תחת `docs/source/sns/reports/`. | Markdown + JSON digest. |
| Refund desk | החזרים מאושרי ממשל דרך `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

עוזרי CI משקפים את הזרימות האלו:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## תצפית ודיווח

- דשבורדים: `dashboards/grafana/sns_payment_settlement.json` עבור סיכומי אוצרות
  מול stewards, תשלומי referral, עומק תור וזמן השהיית החזרים.
- התראות: `dashboards/alerts/sns_payment_settlement_rules.yml` מנטר גיל pending,
  כשלי reconciliation וסטיות ledger.
- דוחות: digests יומיים (`settlement_YYYYMMDD.{json,md}`) מתגלגלים לדוחות חודשיים
  (`settlement_YYYYMM.md`) שמועלים גם ל-Git וגם לאחסון אובייקטים של הממשל
  (`s3://sora-governance/sns/settlements/<period>/`).
- חבילות ממשל מאגדות דשבורדים, לוגים של CLI ואישורים לפני חתימת council.

## צ'קליסט השקה

1. בנו אבטיפוס לעוזרי quote + ledger ותפסו bundle של staging.
2. הפעילו `sns_settlementd` עם queue + writer, חברו דשבורדים והפעילו בדיקות
   התראה (`promtool test rules ...`).
3. מסרו את עוזר ההחזרים ואת תבנית הדוח החודשי; שכפלו ארטיפקטים אל
   `docs/portal/docs/sns/reports/`.
4. הריצו חזרה עם שותפים (חודש מלא של settlements) ותעדו את הצבעת הממשל
   שמכריזה על השלמת SN-5.

חזרו למסמך המקור עבור הגדרות הסכימה המדויקות, שאלות פתוחות ותיקונים עתידיים.
