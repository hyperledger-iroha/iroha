---
lang: ur
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مقامی -> عالمی ایڈریس کٹ

یہ صفحہ مونو-ریپو سے `docs/source/sns/local_to_global_toolkit.md` کی عکاسی کرتا ہے۔ پیکیجز سی ایل آئی مددگار اور رن بوکس ** ایڈ آر -5 سی ** روڈ میپ آئٹم کے ذریعہ درکار ہیں۔

## خلاصہ

- `scripts/address_local_toolkit.sh` پیدا کرنے کے لئے `iroha` CLI لپیٹ گیا:
  - `audit.json` - `iroha tools address audit --format json` کی تشکیل شدہ آؤٹ پٹ۔
  - `normalized.txt` - ہر مقامی ڈومین سلیکٹر کے لئے تبدیل شدہ IH58 (ترجیحی) / کمپریسڈ (`sora`) (دوسرا بہترین) لٹرلز۔
- اسکرپٹ کو ایڈریس انجشن ڈیش بورڈ (`dashboards/grafana/address_ingest.json`) کے ساتھ جوڑیں
  اور الرٹ مینجر قواعد (`dashboards/alerts/address_ingest_rules.yml`) اس لوکل -8/کٹ اوور کو جانچنے کے لئے
  مقامی 12 محفوظ ہے۔ لوکل -8 اور لوکل -12 تصادم پینل اور الرٹس کا مشاہدہ کریں
  `AddressLocal8Resurgence` ، `AddressLocal12Collision` ، اور `AddressInvalidRatioSlo` سے پہلے
  واضح تبدیلیوں کو فروغ دیں۔
- [ایڈریس ڈسپلے کے رہنما خطوط] (address-display-guidelines.md) اور دی کا حوالہ دیں
  [ایڈریس مینی فیسٹ رن بک] (../../../source/runbooks/address_manifest_ops.md) UX سیاق و سباق اور واقعہ کے جواب کے لئے۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

اختیارات:

- `--format compressed (`SORA`)` `sora...` کے لئے IH58 کے بجائے آؤٹ پٹ۔
- `--no-append-domain` بغیر ڈومین کے لغویوں کو خارج کرنے کے لئے۔
- تبادلوں کے قدم کو چھوڑنے کے لئے `--audit-only`۔
- `--allow-errors` اسکیننگ جاری رکھنے کے لئے جب خراب شدہ قطاریں نمودار ہوتی ہیں (CLI کے طرز عمل سے مماثل ہوتی ہے)۔

اسکرپٹ پھانسی کے اختتام پر نمونے والے راستے لکھتا ہے۔ دونوں فائلوں کو منسلک کریں
Grafana ایکس کے اسکرین شاٹ کے ساتھ آپ کا تبدیلی کے انتظام کا ٹکٹ جو صفر ثابت کرتا ہے
> = 30 دن کے لئے لوکل -8 کا پتہ لگانے اور صفر لوکل -12 تصادم۔

## CI انضمام

1. اسکرپٹ کو کسی سرشار ملازمت میں چلائیں اور اس کے آؤٹ پٹ کو اپ لوڈ کریں۔
2. بلاک ضم ہوجاتا ہے جب `audit.json` مقامی سلیکٹرز (`domain.kind = local12`) کی اطلاع دیتا ہے۔
   اس کی ڈیفالٹ ویلیو `true` پر (صرف `false` کو دیو/ٹیسٹ کلسٹرز میں اوور رائڈ
   رجعت پسندی کی تشخیص کریں) اور شامل کریں
   `iroha tools address normalize --fail-on-warning --only-local` to CI تاکہ یہ کوشش کرے
   پیداوار تک پہنچنے سے پہلے رجعت ناکام ہوجاتی ہے۔

مزید تفصیلات ، شواہد چیک لسٹس اور اس کے ٹکڑے کے لئے ماخذ دستاویز سے مشورہ کریں
نوٹ جاری کریں جو آپ کے مؤکلوں کو کٹور کا اعلان کرتے وقت دوبارہ استعمال کرسکتے ہیں۔