---
lang: he
direction: rtl
source: docs/source/soranet_gateway_billing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fc115dfe974fd80c707df0fef676bf48dd6bf3f584c8be5b4f306ed470e9db57
source_last_modified: "2025-11-21T13:08:05.986629+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet_gateway_billing.md -->

# חיוב שער SoraGlobal (SN15-M0-9)

הערה זו מתעדת את קטלוג החיוב M0 עבור CDN שער SoraGlobal ומספקת את הכלים
לדירוג שימוש, אכיפת guardrails והפקת תחזיות לדג'ר. כל הסכומים נקובים
ב‑**micro-XOR ‏(1e-6 XOR)** אלא אם צוין אחרת.

## ארטיפקטים קנוניים
- קטלוג מונים: `configs/soranet/gateway_m0/meter_catalog.json`
- Guardrails: `configs/soranet/gateway_m0/billing_guardrails.json`
- דוגמת snapshot שימוש: `configs/soranet/gateway_m0/billing_usage_sample.json`
- תבנית התאמה: `docs/examples/soranet_gateway_billing/reconciliation_report_template.md`

## קטלוג מונים
| מזהה מונה | יחידה | מחיר בסיס (micro-XOR) | מכפילי אזור (bps) | מדרגות הנחה (bps @ סף) |
|----------|------|------------------------|--------------------|-------------------------|
| `http.request` | request | 5 | NA 10000, EU 9500, APAC 11000, LATAM 10250 | 500 @ 1,000,000; 900 @ 5,000,000 |
| `http.egress_gib` | gibibyte | 250000 | NA 10000, EU 9500, APAC 11000, LATAM 10250 | 300 @ 100; 1000 @ 300 |
| `dns.doh_query` | request | 3 | NA 10000, EU 9800, APAC 10500, LATAM 10000 | 1000 @ 2,000,000 |
| `waf.decision` | decision | 20 | GLOBAL 10000 | — |
| `car.verification_ms` | ms | 10 | GLOBAL 10000 | 1500 @ 500,000 |
| `storage.gib_month` | gib-month | 180000 | NA 10000, EU 9700, APAC 10300, LATAM 10000 | 500 @ 50; 1000 @ 200 |

הערות:
- מכפילי אזור והנחות מוחלים על סך ההוצאה של שורת פריט, לא על מחיר ליחידה,
  כדי לשמור על דטרמיניזם בנפחים גבוהים.
- אם אזור חסר ממפת המכפילים, ברירת המחדל היא `10000` (1.0x).

## Harness חיוב (`cargo xtask soranet-gateway-billing`)

הריצו את צינור הדירוג מול snapshot שימוש:

```
cargo xtask soranet-gateway-billing \
  --usage configs/soranet/gateway_m0/billing_usage_sample.json \
  --catalog configs/soranet/gateway_m0/meter_catalog.json \
  --guardrails configs/soranet/gateway_m0/billing_guardrails.json \
  --payer i105... \
  --treasury i105... \
  --asset xor#wonderland \
  --out artifacts/soranet/gateway_billing
```

פלטים (כל הנתיבים תחת ספריית `--out`):
- `meter_catalog.json` — עותק של הקטלוג ששימש בהרצה.
- `billing_usage_snapshot.json` — קלט שימוש מנורמל.
- `billing_invoice.json` — חשבונית Norito JSON קנונית עם פסקי guardrail.
- `billing_invoice.csv` — יצוא שורות פריט בטבלה.
- `billing_invoice.parquet` — יצוא Parquet (סכמת Arrow) לכלי BI.
- `billing_ledger_projection.json` — `TransferAssetBatch` שמכוון לדג'ר XOR
  עם חיווט payer/treasury והסכום לתשלום.
- `billing_reconciliation_report.md` — דוח התאמה ממולא מראש הנגזר מהתבנית
  וממטא‑נתוני ההרצה.
- מטא‑נתוני החשבונית כוללים כעת `normalized_entries` (אזורים באותיות גדולות,
  איחוד כפילויות) ו‑`merge_notes` כדי שכלי התאמה יוכלו לבצע diff למיזוגי שימוש.

התנהגות guardrail:
- תקרת רכה (ברירת מחדל 140,000,000 micro-XOR) מייצרת דגל התראה; תקרת קשיחה
  (ברירת מחדל 220,000,000 micro-XOR) חוסמת את תחזית הדג'ר אלא אם הקורא
  ממשיך במפורש.
- רצפת חשבונית מינימלית (ברירת מחדל 1,000,000 micro-XOR) מונעת הפקת חשבוניות
  זעירות.
- אזורים לא מוכרים נדחים (הקטלוג חייב למנות מכפיל אזור); זוגות מונה/אזור
  כפולים מאוחדים עם אזורים באותיות גדולות ונרשמים בחשבונית לצורך audit replay.

## תהליך מחלוקת והתאמה
1. לאמת את snapshot השימוש מול הקטלוג (מזהי מונה, יחידות, אזורים).
2. לאשר ש‑`billing_invoice.json` תואם ליצואי CSV/Parquet.
3. לבדוק דגלי guardrail ולצרף ראיות התראה/override בעת הצורך.
4. לוודא שסכום `billing_ledger_projection.json` שווה ל‑`total_micros` של החשבונית
   ומפנה לחשבונות payer/treasury ול‑asset definition הצפויים.
5. למלא את `billing_reconciliation_report.md` (או להתחיל מהתבנית תחת
   `docs/examples/soranet_gateway_billing/reconciliation_report_template.md`)
   עם קישורי ראיות, אישורים וכל התאמה מבוקשת.

## כללי דירוג ועיגול
- מכפילי אזור ומדרגות הנחה מבוטאים ב‑bps ומוחלים על סך שורת הפריט
  (כמות × מחיר בסיס), מעוגל ל‑micro-XOR הקרוב ביותר.
- מדרגות הנחה בוחרות את הסף הגבוה ביותר החל על כל שורה.
- הסכומים נשמרים ומיוצאים ב‑micro-XOR; תחזיות הדג'ר ממירות את הסכום הסופי
  ל‑`Numeric` עם שש ספרות אחרי הנקודה לפני הפקת ה‑transfer batch.

</div>
