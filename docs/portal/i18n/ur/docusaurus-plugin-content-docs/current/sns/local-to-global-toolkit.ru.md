---
lang: ur
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ٹول کٹ لوکل -> عالمی پتے

یہ صفحہ مونو-ریپو سے `docs/source/sns/local_to_global_toolkit.md` کی عکاسی کرتا ہے۔ اس میں CLI مددگاروں اور رن بکس شامل ہیں جو ** EDDR-5C ** روڈ میپ آئٹم کے ذریعہ درکار ہیں۔

## جائزہ

- `scripts/address_local_toolkit.sh` حاصل کرنے کے لئے CLI `iroha` کو لپیٹتا ہے:
  - `audit.json` - ساختہ آؤٹ پٹ `iroha tools address audit --format json`۔
  - `normalized.txt`- تبدیل شدہ I105 (ترجیحی) / کمپریسڈ (`sora`) (دوسرا انتخاب) ہر مقامی ڈومین سلیکٹر کے ل liters لفظی۔
- اسکرپٹ کو ڈیش بورڈ انجسٹ ایڈریسز (`dashboards/grafana/address_ingest.json`) کے ساتھ مل کر استعمال کریں
  اور الرٹ مینجر قواعد (`dashboards/alerts/address_ingest_rules.yml`) کٹ اوور لوکل -8 / کی حفاظت کو ثابت کرنے کے لئے
  مقامی 12۔ لوکل -8 اور لوکل -12 تصادم پینل اور الرٹس کی نگرانی کریں
  `AddressLocal8Resurgence` ، `AddressLocal12Collision` ، اور `AddressInvalidRatioSlo` سے پہلے
  تبدیلیوں کو فروغ دینا ظاہر ہوتا ہے۔
- [ایڈریس ڈسپلے گائیڈ لائنز] (address-display-guidelines.md) اور دیکھیں
  [ایڈریس مینی فیسٹ رن بک] (../../../source/runbooks/address_manifest_ops.md) UX اور واقعہ کے ردعمل کے سیاق و سباق کے لئے۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

اختیارات:

- `--format i105 (`SORA`)` `sora...` سے I105 کی بجائے آؤٹ پٹ۔
- ننگے لٹریلز آؤٹ پٹ کے لئے `domainless output (default)`۔
- تبادلوں کے قدم کو چھوڑنے کے لئے `--audit-only`۔
- `--allow-errors` اسکیننگ جاری رکھنے کے لئے جب غلط لکیریں ہوں (سلوک CLI کی طرح ہی ہے)۔

اسکرپٹ پر عمل درآمد کے اختتام پر نمونے کی راہیں دکھاتی ہیں۔ دونوں فائلوں کو منسلک کریں
Grafana اسکرین شاٹ کے ساتھ تبدیل کریں
کم از کم> = 30 دن کے لئے مقامی -8 کا پتہ لگانے اور صفر لوکل -12 تصادم۔

## CI انضمام

1. اسکرپٹ کو علیحدہ ملازمت اور بوجھ کے نتائج میں چلائیں۔
2. بلاک ضم ہوجاتا ہے جب `audit.json` مقامی سلیکٹرز (`domain.kind = local12`) کی اطلاع دیتا ہے۔
   ڈیفالٹ ویلیو `true` کے ساتھ (جب رجعت پسندی کی تشخیص کرتے ہو تو صرف دیو/ٹیسٹ میں `false` میں تبدیل ہوتا ہے) اور
   رجعت کی کوششوں کی اجازت دینے کے لئے CI میں `iroha tools address normalize` شامل کریں
   پیداوار میں گر گیا۔

تفصیلات ، شواہد چیک لسٹس اور ریلیز نوٹ کے اسنیپٹ کے لئے ماخذ دستاویز دیکھیں ، جو ہوسکتا ہے
گاہکوں کے لئے کٹ اوور کا اعلان کرتے وقت استعمال کریں۔