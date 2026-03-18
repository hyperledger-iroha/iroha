---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

مکمل گائیڈ `docs/source/nexus_sdk_quickstarts.md` میں ہے۔ اس پورٹل جائزہ میں مشترکہ شرائط اور فی ایس ڈی کے کے کمانڈز کو نمایاں کیا گیا ہے تاکہ ڈویلپرز کو ان کی تشکیل کو جلدی سے چیک کیا جاسکے۔

## مشترکہ ترتیب

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus کنفیگریشن پیکیج ڈاؤن لوڈ کریں ، ہر SDK کے لئے انحصار انسٹال کریں ، اور تصدیق کریں کہ TLS سرٹیفکیٹ ریلیز پروفائل سے ملتے ہیں (`docs/source/sora_nexus_operator_onboarding.md` دیکھیں)۔

## مورچا

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

ریفز: `docs/source/sdk/rust.md`

## جاوا اسکرپٹ/ٹائپ اسکرپٹ

```bash
npm run demo:nexus
```

اسکرپٹ `ToriiClient` کو مندرجہ بالا ماحولیاتی متغیرات کے ساتھ انسٹیٹ کرتا ہے اور آخری بلاک پرنٹ کرتا ہے۔

## سوئفٹ

```bash
make swift-nexus-demo
```

`IrohaSwift` سے `Torii.Client` استعمال کریں `FindNetworkStatus` حاصل کریں۔

## android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexus اسٹیجنگ اختتامی نقطہ کی طرف اشارہ کرتے ہوئے منظم ڈیوائس ٹیسٹ چلاتا ہے۔

## سی ایل آئی

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## خرابیوں کا سراغ لگانا

- TLS کی ناکامی -> Nexus ریلیز ٹربال کے CA بنڈل کی تصدیق کریں۔
-`ERR_UNKNOWN_LANE` -> پاس `--lane-id`/`--dataspace-id` جب ملٹی لین روٹنگ لازمی ہے۔
- `ERR_SETTLEMENT_PAUSED` -> واقعہ کے عمل کے لئے [Nexus آپریشنز] (../nexus/nexus-operations) چیک کریں۔ گورننس لین کو روکنے میں کامیاب رہی۔

مزید سیاق و سباق اور وضاحتوں کے لئے SDK `docs/source/nexus_sdk_quickstarts.md` دیکھیں۔