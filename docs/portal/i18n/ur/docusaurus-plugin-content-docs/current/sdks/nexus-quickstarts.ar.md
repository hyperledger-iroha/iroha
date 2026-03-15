---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

مکمل گائیڈ `docs/source/nexus_sdk_quickstarts.md` پر ہے۔ اس پورٹل سمری میں ہر ایس ڈی کے کے لئے مشترکہ ضروریات اور احکامات پر روشنی ڈالی گئی ہے تاکہ ڈویلپر اپنی ترتیبات کو جلدی سے چیک کرسکیں۔

## عام سیٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus کنفیگریشن پیکیج ڈاؤن لوڈ کریں ، ہر SDK کے لئے انحصار انسٹال کریں ، اور اس بات کو یقینی بنائیں کہ TLS سرٹیفکیٹ ریلیز فائل سے مماثل ہیں (`docs/source/sora_nexus_operator_onboarding.md` دیکھیں)۔

## مورچا

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

حوالہ جات: `docs/source/sdk/rust.md`

## جاوا اسکرپٹ / ٹائپ اسکرپٹ

```bash
npm run demo:nexus
```

اسکرپٹ میں `ToriiClient` کو مندرجہ بالا ماحولیاتی متغیرات کا استعمال کرتے ہوئے شروع کیا گیا ہے اور آخری بلاک پرنٹ کرتا ہے۔

## سوئفٹ

```bash
make swift-nexus-demo
```

`Torii.Client` `IrohaSwift` سے `FindNetworkStatus` کو لانے کے لئے استعمال کیا جاتا ہے۔

## android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

گٹھ جوڑ اسٹیجنگ اینڈ پوائنٹ کو نشانہ بناتے ہوئے ایک منظم آلہ ٹیسٹ چلاتا ہے۔

## سی ایل آئی

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## خرابیوں کا سراغ لگانا

- TLS خرابی -> ٹربال ورژن Nexus سے آنے والے CA پیکیج کو چیک کریں۔
- `ERR_UNKNOWN_LANE` -> پاس `--lane-id`/`--dataspace-id` جب ملٹیپاتھ روٹنگ نافذ کیا جاتا ہے۔
- `ERR_SETTLEMENT_PAUSED` -> دیکھیں [Nexus آپریشنز] (../nexus/nexus-operations) واقعہ کے عمل کے لئے۔ ہوسکتا ہے کہ گورننس نے اس کورس کو روک دیا ہو۔

اضافی سیاق و سباق اور ہر SDK کی وضاحت کے لئے `docs/source/nexus_sdk_quickstarts.md` دیکھیں۔