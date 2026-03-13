---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

La référence est `docs/source/nexus_sdk_quickstarts.md`. Vous avez besoin de plus d'informations sur le SDK et le SDK pour plus de détails. اعداداتهم بسرعة.

## الاعداد المشترك

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Vous pouvez également utiliser le logiciel Nexus, ainsi que le SDK et utiliser TLS pour créer un lien `docs/source/sora_nexus_operator_onboarding.md`).

## Rouille

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Nom : `docs/source/sdk/rust.md`

## JavaScript/TypeScript

```bash
npm run demo:nexus
```

يقوم السكربت بتهيئة `ToriiClient` باستخدام متغيرات البيئة اعلاه ويطبع اخر كتلة.

## Rapide

```bash
make swift-nexus-demo
```

Remplacez `Torii.Client` par `IrohaSwift` ou `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

يشغل اختبار الجهاز المدار الذي يستهدف نقطة نهاية mise en scène لنكسس.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## استكشاف الاخطاء واصلاحها

- Utiliser TLS -> Utiliser CA pour l'archive tar Nexus.
- `ERR_UNKNOWN_LANE` -> مرر `--lane-id`/`--dataspace-id` عندما يتم فرض التوجيه متعدد المسارات.
- `ERR_SETTLEMENT_PAUSED` -> راجع [Opérations Nexus](../nexus/nexus-operations) لعملية الحوادث؛ قد تكون الحوكمة اوقفت المسار.

Vous devez utiliser le SDK `docs/source/nexus_sdk_quickstarts.md`.