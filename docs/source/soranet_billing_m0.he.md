---
lang: he
direction: rtl
source: docs/source/soranet_billing_m0.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 62a3cc35e474922cd7c3ff8a23b89f776a49f5a0aa24fdad56890fd5f7d46a81
source_last_modified: "2026-01-03T18:07:58.038711+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet_billing_m0.md -->

# חיוב שער SoraGlobal (SN15-M0-9)

ה‑`soranet-gateway-billing-m0` helper מספק את חבילת התצוגה המקדימה לחיוב עבור
CDN שער SoraGlobal. הוא שומר על קטלוג המונים וארטיפקטי החיוב דטרמיניסטיים כך
שתרגילי PoP וחבילות ממשל יסתמכו על אותן ראיות.

## הרצת המחולל

```bash
cargo run -p xtask --bin xtask -- soranet-gateway-billing-m0 \
  --billing-period 2026-11 \
  --output-dir configs/soranet/gateway_m0/billing
```

פלטים:

- `billing_meter_catalog.json` + `billing_meter_catalog.{csv,parquet}` — שישה
  מונים (בקשות/הוצאת HTTP, שאילתות DoH, החלטות WAF, אימות CAR, אחסון) עם מדרגות,
  הנחות, מכפילי אזור ותקרות burst.
- `billing_rating_plan.toml` — נובים חוזיים (הנחות התחייבות/תשלום מראש, skim לאוצר,
  החזקת מחלוקת) לצד כרטיסי תעריף לכל מונה.
- `billing_ledger_hooks.toml` + `billing_ledger_projection.json` — כללי רישום
  receivable / revenue / escrow / treasury והקרנת עבודה על בסיס חבילת שימוש לדוגמה.
- `billing_guardrails.yaml` — תקרות לכל מונה ונתיבי התראה עבור אימות promtool /
  Alertmanager.
- תבניות חשבונית, מחלוקת והתאמה נשמרות מסונכרנות עם ארטיפקטי הדירוג + הלדג'ר.
- `billing_m0_summary.json` — נתיבים יחסיים לכל ארטיפקט עבור חבילות ראיות ממשל.

## הערות

- מכפילי אזור והנחות מיושמים בייצוא CSV/Parquet כך שמיישרי חיוב יכולים לבצע diff
  מול רשומות הלדג'ר ללא סקריפטים ייעודיים.
- הקרנת הלדג'ר משתמשת בדוגמת שימוש M0 (בקשות, egress, DoH, WAF, אימות CAR, אחסון)
  ומחילה הנחות התחייבות + תשלום מראש וכן החזקת מחלוקת ו‑skim לאוצר כפי שהוגדרו בתכנית הדירוג.
- Guardrails משקפים `burst_cap` לכל מונה; התראות מכסות קפיצות שימוש, צמיחת egress,
  השהיית אימות trustless, ופיגור במחלוקות.

</div>
