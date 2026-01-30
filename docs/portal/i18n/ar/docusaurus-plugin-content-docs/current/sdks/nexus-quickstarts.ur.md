---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

مکمل quickstart `docs/source/nexus_sdk_quickstarts.md` میں موجود ہے۔ یہ پورٹل خلاصہ مشترکہ پیشگی تقاضوں اور ہر SDK کے کمانڈز کو نمایاں کرتا ہے تاکہ ڈویلپرز اپنی سیٹ اپ جلدی جانچ سکیں۔

## مشترکہ سیٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus کنفیگریشن بنڈل ڈاؤن لوڈ کریں، ہر SDK کی ڈپینڈنسیاں انسٹال کریں، اور یقینی بنائیں کہ TLS سرٹیفکیٹس ریلیز پروفائل سے میل کھاتے ہیں (دیکھیے `docs/source/sora_nexus_operator_onboarding.md`).

## Rust

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

حوالہ: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

اسکرپٹ اوپر والی ماحولیات متغیرات کے ساتھ `ToriiClient` بناتا ہے اور تازہ ترین بلاک پرنٹ کرتا ہے۔

## Swift

```bash
make swift-nexus-demo
```

`IrohaSwift` کے `Torii.Client` سے `FindNetworkStatus` حاصل کرتا ہے۔

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

منظم ڈیوائس ٹیسٹ چلاتا ہے جو Nexus کے staging endpoint کو ہٹ کرتا ہے۔

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## مسئلہ حل

- TLS ناکامیاں -> Nexus ریلیز tarball سے CA بنڈل کی توثیق کریں۔
- `ERR_UNKNOWN_LANE` -> جب multi-lane routing نافذ ہو تو `--lane-id`/`--dataspace-id` دیں۔
- `ERR_SETTLEMENT_PAUSED` -> واقعہ عمل کے لیے [Nexus operations](../nexus/nexus-operations) دیکھیں؛ ممکن ہے گورننس نے lane روک دی ہو۔

مزید سیاق و سباق اور SDK مخصوص وضاحتوں کے لیے `docs/source/nexus_sdk_quickstarts.md` دیکھیں۔
