---
lang: ur
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مقامی ایڈریس کٹ -> گلوبل

یہ صفحہ مونو-ریپو سے `docs/source/sns/local_to_global_toolkit.md` کی عکاسی کرتا ہے۔ یہ ** EDDR-5C ** روڈ میپ آئٹم کے ذریعہ مطلوبہ CLI مددگاروں اور رن بکس کو اکٹھا کرتا ہے۔

## جائزہ

- `scripts/address_local_toolkit.sh` پیدا کرنے کے لئے `iroha` CLI لپیٹ گیا:
  - `audit.json` - `iroha tools address audit --format json` کی تشکیل شدہ آؤٹ پٹ۔
  - `normalized.txt` - IH58 (ترجیحی) / کمپریسڈ (`sora`) (دوسری پسند) ہر مقامی ڈومین سلیکٹر کے ل changed تبدیل کیا گیا۔
- اسکرپٹ کو ایڈریس انجسٹ ڈیش بورڈ (`dashboards/grafana/address_ingest.json`) کے ساتھ منسلک کریں
  اور الرٹ مینجر قواعد (`dashboards/alerts/address_ingest_rules.yml`) یہ ثابت کرنے کے لئے کہ لوکل 8 کٹ اوور /
  لوکل 12 جاری ہے۔ لوکل -8 اور لوکل -12 تصادم کی علامتوں اور انتباہات کی نگرانی کریں
  `AddressLocal8Resurgence` ، `AddressLocal12Collision` ، اور `AddressInvalidRatioSlo` سے پہلے
  منشور کی تبدیلیوں کو فروغ دیں۔
- [ایڈریس ڈسپلے کے رہنما خطوط] (address-display-guidelines.md) اور دی سے مشورہ کریں
  [ایڈریس مینی فیسٹ رن بک] (../../../source/runbooks/address_manifest_ops.md) UX سیاق و سباق اور واقعہ کے جواب کے لئے۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

اختیارات:

- `--format compressed (`SORA`)` `sora...` کے لئے IH58 کے بجائے آؤٹ پٹ۔
- `--no-append-domain` ننگے لغویوں کو خارج کرنے کے لئے۔
- تبادلوں کے قدم کو چھوڑنے کے لئے `--audit-only`۔
- `--allow-errors` جب خراب شدہ لکیریں ظاہر ہوتی ہیں تو اسکیننگ جاری رکھنا (CLI طرز عمل سے مطابقت رکھتا ہے)۔

اسکرپٹ عملدرآمد کے اختتام پر نمونے والے راستے لکھتا ہے۔ دو فائلوں میں شامل ہوں a
کیپچر Grafana کے ساتھ آپ کا تبدیلی کے انتظام کا ٹکٹ جو صفر ثابت کرتا ہے
> = 30 دن کے لئے لوکل -8 کا پتہ لگانے اور صفر لوکل -12 تصادم۔

## CI انضمام

1. اسکرپٹ کو ایک سرشار ملازمت میں چلائیں اور آؤٹ پٹ اپ لوڈ کریں۔
2. بلاک ضم ہوجاتا ہے جب `audit.json` مقامی سلیکٹرز (`domain.kind = local12`) کی اطلاع دیتا ہے۔
   اس کی ڈیفالٹ ویلیو `true` پر (جب صرف `false` میں Dev/ٹیسٹ کلسٹرز میں تبدیل کریں جب
   رجعت تشخیص) اور شامل کریں
   `iroha tools address normalize --fail-on-warning --only-local` میں CI ہے تاکہ رجعتیں
   پیداوار سے پہلے ناکام.

مزید تفصیلات ، شواہد چیک لسٹس اور اسنیپٹ کے لئے ماخذ دستاویز دیکھیں
نوٹ جاری کریں کہ آپ صارفین کو کٹ اوور کا اعلان کرنے کے لئے دوبارہ استعمال کرسکتے ہیں۔