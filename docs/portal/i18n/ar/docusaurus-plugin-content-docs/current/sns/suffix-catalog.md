---
id: suffix-catalog
lang: ar
direction: rtl
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# كتالوج لاحقات خدمة اسماء سورا

يتتبع roadmap لـ SNS كل لاحقة معتمدة (SN-1/SN-2). تعكس هذه الصفحة كتالوج
المصدر الحقيقي حتى يتمكن المشغلون الذين يديرون registrars وبوابات DNS او
ادوات المحافظ من تحميل نفس المعلمات دون كشط مستندات الحالة.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **المستهلكون:** `iroha sns policy`، اطقم onboarding لـ SNS، لوحات KPI، وبرامج
  اصدار DNS/Gateway تقرأ نفس حزمة JSON.
- **الحالات:** `active` (التسجيلات مسموحة)، `paused` (مقيد مؤقتا)، `revoked`
  (معلن لكنه غير متاح حاليا).

## مخطط الكتالوج

| الحقل | النوع | الوصف |
|-------|-------|-------|
| `suffix` | string | لاحقة مقروءة للبشر مع نقطة بادئة. |
| `suffix_id` | `u16` | معرف مخزن على الدفتر في `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused` او `revoked` تصف جاهزية الاطلاق. |
| `steward_account` | string | حساب مسؤول عن stewardship (يطابق hooks سياسة المسجل). |
| `fund_splitter_account` | string | حساب يستلم المدفوعات قبل التوجيه وفق `fee_split`. |
| `payment_asset_id` | string | الاصل المستخدم للتسوية (`xor#sora` للدفعة الاولية). |
| `min_term_years` / `max_term_years` | integer | حدود مدة الشراء من السياسة. |
| `grace_period_days` / `redemption_period_days` | integer | نوافذ امان التجديد التي يفرضها Torii. |
| `referral_cap_bps` | integer | الحد الاقصى لـ referral carve-out المسموح به حوكما (basis points). |
| `reserved_labels` | array | كائنات تسميات محمية بالحوكمة `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | كائنات tier مع `label_regex`, `base_price`, `auction_kind`, وحدود المدة. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` تقسيم بنقاط اساس. |
| `policy_version` | integer | عداد متزايد يزاد عند تعديل الحوكمة للسياسة. |

## الكتالوج الحالي

| اللاحقة | المعرف (`hex`) | Steward | Fund splitter | الحالة | اصل الدفع | حد referral (bps) | المدة (min - max سنوات) | Grace / Redemption (ايام) | شرائح التسعير (regex -> السعر الاساسي / المزاد) | التسميات المحجوزة | تقسيم الرسوم (T/S/R/E bps) | نسخة السياسة |
|---------|---------------|---------|---------------|--------|-----------|--------------------|--------------------------|---------------------------|-------------------------------------------------|-------------------|-----------------------------|-------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | نشط | `xor#sora` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | معلق | `xor#sora` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> i105...`, `guardian -> i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | ملغي | `xor#sora` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## مقتطف JSON

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "i105...",
      "payment_asset_id": "xor#sora",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "xor#sora", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## ملاحظات الاتمتة

1. حمل snapshot JSON وقم بعمل hash/توقيع قبل توزيعه على المشغلين.
2. ينبغي لادوات registrar عرض `suffix_id` وحدود المدة والتسعير من الكتالوج عندما
   تصل طلبات `/v1/sns/*`.
3. تقرأ مساعدات DNS/Gateway بيانات labels المحجوزة عند توليد قوالب GAR حتى تبقى
   ردود DNS متوافقة مع ضوابط الحوكمة.
4. تضع مهام ملاحق KPI tags على exports لوحات المتابعة ببيانات اللاحقة حتى تطابق
   التنبيهات حالة الاطلاق المسجلة هنا.
