---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Démarrage rapide `docs/source/nexus_sdk_quickstarts.md` Démarrage rapide Le SDK est actuellement en cours de développement et le SDK est en cours de réalisation. ڈویلپرز اپنی سیٹ اپ جلدی جانچ سکیں۔

## مشترکہ سیٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus Le kit de développement logiciel SDK est également disponible pour le SDK. Le service TLS est en ligne avec le numéro de téléphone (`docs/source/sora_nexus_operator_onboarding.md`).

## Rouille

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Nom : `docs/source/sdk/rust.md`

## JavaScript/TypeScript

```bash
npm run demo:nexus
```

اسکرپٹ اوپر والی ماحولیات متغیرات کے ساتھ `ToriiClient` بناتا ہے اور تازہ ترین بلاک پرنٹ کرتا ہے۔

## Rapide

```bash
make swift-nexus-demo
```

`IrohaSwift` et `Torii.Client` et `FindNetworkStatus` sont en vente

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Il s'agit d'un point de terminaison intermédiaire pour Nexus et d'un point de terminaison intermédiaire.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## مسئلہ حل

- TLS ناکامیاں -> Nexus ریلیز tarball سے CA بنڈل کی توثیق کریں۔
- `ERR_UNKNOWN_LANE` -> Pour le routage multivoie et `--lane-id`/`--dataspace-id`
- `ERR_SETTLEMENT_PAUSED` -> واقعہ عمل کے لیے [Opérations Nexus](../nexus/nexus-operations) دیکھیں؛ ممکن ہے گورننس نے lane روک دی ہو۔

Vous avez besoin d'un SDK et vous l'avez utilisé pour le SDK `docs/source/nexus_sdk_quickstarts.md`.