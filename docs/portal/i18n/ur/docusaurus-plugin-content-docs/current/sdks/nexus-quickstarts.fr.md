---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

مکمل گائیڈ `docs/source/nexus_sdk_quickstarts.md` میں پایا جاسکتا ہے۔ اس پورٹل سمری میں مشترکہ شرائط اور ایس ڈی کے کمانڈز کو اجاگر کیا گیا ہے تاکہ ڈویلپرز اپنی تشکیل کی جلد تصدیق کرسکیں۔

## عام ترتیب

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

کنفیگریشن پیکیج Nexus ڈاؤن لوڈ کریں ، ہر SDK کی انحصار انسٹال کریں اور اس بات کو یقینی بنائیں کہ TLS سرٹیفکیٹ ریلیز پروفائل سے ملتے ہیں (`docs/source/sora_nexus_operator_onboarding.md` دیکھیں)۔

## مورچا

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

ریفز: `docs/source/sdk/rust.md`

## جاوا اسکرپٹ / ٹائپ اسکرپٹ

```bash
npm run demo:nexus
```

اسکرپٹ `ToriiClient` کو مندرجہ بالا ماحولیاتی متغیرات کے ساتھ انسٹیٹیٹ کرتا ہے اور آخری بلاک کو ظاہر کرتا ہے۔

## سوئفٹ

```bash
make swift-nexus-demo
```

`IrohaSwift` سے `Torii.Client` استعمال کریں `FindNetworkStatus` کو بازیافت کریں۔

## android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

منظم ڈیوائس ٹیسٹ چلاتا ہے جو اسٹیجنگ اینڈ پوائنٹ Nexus کو نشانہ بناتا ہے۔

## سی ایل آئی

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## خرابیوں کا سراغ لگانا

- TLS کی ناکامی -> ریلیز ٹربال Nexus کے CA بنڈل چیک کریں۔
-`ERR_UNKNOWN_LANE` -> پاس `--lane-id`/`--dataspace-id` ایک بار ملٹی لین روٹنگ کا اطلاق ہوتا ہے۔
- `ERR_SETTLEMENT_PAUSED` -> مشورہ کریں [Nexus آپریشن] (../nexus/nexus-operations) واقعے کے عمل کے لئے ؛ ہوسکتا ہے کہ گورننس نے لین کو روک لیا ہو۔

SDK کے ذریعہ مزید سیاق و سباق اور وضاحت کے لئے ، `docs/source/nexus_sdk_quickstarts.md` دیکھیں۔