---
lang: he
direction: rtl
source: docs/source/soranet_gateway_billing_m0.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ab232fba530de14a767bf1832bc00e19a4c82e6ca431ef5c875a9f62c321509d
source_last_modified: "2025-11-21T12:20:45.435832+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet_gateway_billing_m0.md -->

# חבילת חיוב M0 לשער SoraGlobal

SN15-M0-9 מתעד את ארטיפקטי תצוגת החיוב ששימשו במהלך תרגילי PoP מוקדמים.
העזר `soranet-gateway-billing-m0` כותב חבילה דטרמיניסטית כך שצוותי
אוצר/תפעול/ממשל יוכלו לצרף את אותן ראיות לכרטיסים ולחזרות.

## שימוש

```
cargo xtask soranet-gateway-billing-m0 \
  --billing-period 2026-11 \
  --output-dir artifacts/soranet/gateway_m0/billing_demo
```

ברירות מחדל:
- תקופת חיוב: `2026-11`
- ספריית פלט: `configs/soranet/gateway_m0/billing/`

## תוכן החבילה

- `billing_meter_catalog.json` — מזהי/יחידות מונה, מכפילי אזור, ותקרות.
- `billing_rating_plan.toml` — כללי דירוג/הנחה/מדרגות (bps, micros).
- `billing_ledger_hooks.toml` — כללי רישום לחיובים/הכנסות/escrow/החזרים.
- `billing_guardrails.yaml` — ספי התראה לתקרות, מחלוקות, ועיכוב אימות.
- תבניות: `billing_invoice_template.md`, `billing_dispute_template.md`,
  `billing_reconciliation_template.md`.
- `billing_m0_summary.json` — נתיבים יחסיים לכל הקבצים לצירוף מהיר.

שמרו את החבילה ללא שינוי עבור תרגילי M0; מחזרו עם `--output-dir` כדי לייצר
עותקים ייחודיים לסביבה מבלי לגעת בברירות המחדל הקנוניות שב‑
`configs/soranet/gateway_m0/billing/`.

</div>
