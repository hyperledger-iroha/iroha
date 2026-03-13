---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Guia de início rápido `docs/source/nexus_sdk_quickstarts.md` موجود ہے۔ یہ پورٹل خلاصہ مشترکہ پیشگی تقاضوں اور ہر SDK کے کمانڈز کو نمایاں کرتا ہے تاکہ ڈویلپرز اپنی سیٹ اپ جلدی جانچ سکیں۔

## مشترکہ سیٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus کنفیگریشن بنڈل ڈاؤن لوڈ کریں, ہر SDK کی ڈپینڈنسیاں انسٹال کریں, اور یقینی بنائیں کہ TLS سرٹیفکیٹس ریلیز پروفائل سے میل کھاتے ہیں (دیکھیے `docs/source/sora_nexus_operator_onboarding.md`).

## Ferrugem

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Nome: `docs/source/sdk/rust.md`

##JavaScript/TypeScript

```bash
npm run demo:nexus
```

اسکرپٹ اوپر والی ماحولیات متغیرات کے ساتھ `ToriiClient` بناتا ہے اور تازہ ترین بلاک پرنٹ کرتا ہے۔

## Rápido

```bash
make swift-nexus-demo
```

`IrohaSwift` کے `Torii.Client` ou `FindNetworkStatus` حاصل کرتا ہے۔

##Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

O endpoint de teste Nexus é o endpoint de teste que é o endpoint de teste

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## مسئلہ حل

- TLS ناکامیاں -> Nexus ریلیز tarball سے CA بنڈل کی توثیق کریں۔
- `ERR_UNKNOWN_LANE` -> Roteamento multi-lane نافذ ہو تو `--lane-id`/`--dataspace-id` دیں۔
- `ERR_SETTLEMENT_PAUSED` -> واقعہ عمل کے لیے [operações Nexus](../nexus/nexus-operations) دیکھیں؛ ممکن ہے گورننس نے pista روک دی ہو۔

مزید سیاق و سباق اور SDK مخصوص وضاحتوں کے لیے `docs/source/nexus_sdk_quickstarts.md` دیکھیں۔