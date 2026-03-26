---
id: suffix-catalog
lang: ur
direction: rtl
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Sora Name Service لاحقہ کیٹلاگ

SNS روڈمیپ ہر منظور شدہ لاحقہ (SN-1/SN-2) کو ٹریک کرتا ہے۔ یہ صفحہ
source-of-truth کیٹلاگ کی عکاسی کرتا ہے تاکہ registrars، DNS gateways یا
wallet tooling چلانے والے آپریٹرز status docs کو scrape کئے بغیر وہی
پیرامیٹرز لوڈ کر سکیں۔

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consumers:** `iroha sns policy`، SNS onboarding kits، KPI dashboards، اور
  DNS/Gateway release scripts ایک ہی JSON bundle پڑھتے ہیں۔
- **Statuses:** `active` (رجسٹریشن کی اجازت)، `paused` (عارضی طور پر محدود)،
  `revoked` (اعلان شدہ مگر فی الحال دستیاب نہیں)۔

## کیٹلاگ اسکیمہ

| فیلڈ | قسم | وضاحت |
|------|-----|-------|
| `suffix` | string | انسان دوست لاحقہ جس کے شروع میں ڈاٹ ہو۔ |
| `suffix_id` | `u16` | شناخت جو ledger میں `SuffixPolicyV1::suffix_id` کے طور پر محفوظ ہے۔ |
| `status` | enum | `active`, `paused`, یا `revoked` جو لانچ ریڈینس بیان کرتے ہیں۔ |
| `steward_account` | string | stewardship کے لئے ذمہ دار اکاؤنٹ (registrar policy hooks سے میل کھاتا ہے)۔ |
| `fund_splitter_account` | string | اکاؤنٹ جو `fee_split` کے مطابق routing سے پہلے ادائیگیاں وصول کرتا ہے۔ |
| `payment_asset_id` | string | settlement کے لئے استعمال ہونے والا asset (`61CtjvNd9T3THAR65GsMVHr82Bjc` ابتدائی cohort کے لئے)۔ |
| `min_term_years` / `max_term_years` | integer | پالیسی سے خریداری مدت کی حدیں۔ |
| `grace_period_days` / `redemption_period_days` | integer | renewal سیفٹی ونڈوز جو Torii نافذ کرتا ہے۔ |
| `referral_cap_bps` | integer | governance کے تحت اجازت یافتہ referral carve-out کی زیادہ سے زیادہ حد (basis points)۔ |
| `reserved_labels` | array | governance-protected label objects `{label, assigned_to, release_at_ms, note}`۔ |
| `pricing` | array | tier objects جن میں `label_regex`, `base_price`, `auction_kind` اور مدت کی حدیں شامل ہوں۔ |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` basis points میں تقسیم۔ |
| `policy_version` | integer | مونوتونک کاؤنٹر جو governance کے پالیسی میں ترمیم پر بڑھتا ہے۔ |

## موجودہ کیٹلاگ

| لاحقہ | ID (`hex`) | Steward | Fund splitter | حالت | ادائیگی asset | referral حد (bps) | مدت (min - max سال) | Grace / Redemption (دن) | pricing tiers (regex -> base price / auction) | reserved labels | fee split (T/S/R/E bps) | policy version |
|-------|------------|---------|---------------|------|--------------|-------------------|----------------------|--------------------------|----------------------------------------------|----------------|-------------------------|----------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | فعال | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | معطل | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> i105...`, `guardian -> i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | منسوخ | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON excerpt

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "soraカタカナ...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
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

## Automation notes

1. JSON snapshot لوڈ کریں اور اسے operators میں تقسیم کرنے سے پہلے hash/sign کریں۔
2. Registrar tooling کو `suffix_id`, term limits اور pricing کو catalog سے ظاہر کرنا
   چاہئے جب کوئی درخواست `/v1/sns/*` پر پہنچے۔
3. DNS/Gateway helpers GAR templates بناتے وقت reserved label metadata پڑھتے ہیں
   تاکہ DNS responses governance controls کے ساتھ aligned رہیں۔
4. KPI annex jobs dashboards کے exports کو suffix metadata کے ساتھ tag کرتے ہیں
   تاکہ alerts یہاں درج launch state کے مطابق ہوں۔
