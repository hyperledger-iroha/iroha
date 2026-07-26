---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

مکمل کوئک اسٹارٹ `docs/source/nexus_sdk_quickstarts.md` میں واقع ہے۔ اس پورٹل جائزہ میں ہر ایس ڈی کے کے لئے مشترکہ شرائط اور احکامات کو اجاگر کیا گیا ہے تاکہ ڈویلپرز اپنے سیٹ اپ کو جلدی سے جانچ کرسکیں۔

## عمومی سیٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus کنفیگریشن پیکیج ڈاؤن لوڈ کریں ، ہر SDK کی انحصار انسٹال کریں ، اور اس بات کو یقینی بنائیں کہ TLS سرٹیفکیٹ ریلیز پروفائل سے ملتے ہیں (`docs/source/sora_nexus_operator_onboarding.md` دیکھیں)۔

## مورچا

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

ملاحظہ کریں: `docs/source/sdk/rust.md`

## جاوا اسکرپٹ / ٹائپ اسکرپٹ

```bash
npm run demo:nexus
```

اسکرپٹ `ToriiClient` کو اوپر ماحول کے متغیر کے ساتھ شروع کرتا ہے اور آخری بلاک پرنٹ کرتا ہے۔

## سوئفٹ

```bash
make swift-nexus-demo
```

`IrohaSwift` سے `Torii.Client` استعمال کرتا ہے `FindNetworkStatus` حاصل کرنے کے لئے۔

## android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

ایک منظم ڈیوائس ٹیسٹ چلاتا ہے جو اسٹیجنگ اینڈ پوائنٹ Nexus تک رسائی حاصل کرتا ہے۔

## سی ایل آئی

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## خرابیوں کا سراغ لگانا

- TLS کی ناکامی -> ریلیز ٹربال Nexus سے CA بنڈل چیک کریں۔
-`ERR_UNKNOWN_LANE` -> پاس `--lane-id`/`--dataspace-id` جب ملٹی لین روٹنگ فعال ہوجائے۔
- `ERR_SETTLEMENT_PAUSED` -> واقعے کے عمل کے لئے [Nexus آپریشن] (../nexus/nexus-operations) دیکھیں۔ شاید گورننس نے لین کو معطل کردیا۔

ایس ڈی کے کے گہرے سیاق و سباق اور وضاحت کے لئے ، `docs/source/nexus_sdk_quickstarts.md` دیکھیں۔