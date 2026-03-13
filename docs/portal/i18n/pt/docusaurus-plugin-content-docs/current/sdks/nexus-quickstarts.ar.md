---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

O nome do arquivo é `docs/source/nexus_sdk_quickstarts.md`. Você pode usar o SDK do SDK para obter mais informações sobre o SDK do seu site. Isso é algo que você pode fazer.

## الاعداد المشترك

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Para obter o Nexus, você pode instalar o SDK no SDK e usar o TLS para usar. Nome (`docs/source/sora_nexus_operator_onboarding.md`).

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

A chave `ToriiClient` pode ser usada para remover o problema.

## Rápido

```bash
make swift-nexus-demo
```

Use `Torii.Client` para `IrohaSwift` para `FindNetworkStatus`.

##Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

يشغل اختبار الجهاز المدار الذي يستهدف نقطة نهاية staging لنكسس.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## استكشاف الاخطاء واصلاحها

- اعطال TLS -> تاكد من حزمة CA القادمة من tarball اصدار Nexus.
- `ERR_UNKNOWN_LANE` -> Selecione `--lane-id`/`--dataspace-id`.
- `ERR_SETTLEMENT_PAUSED` -> راجع [operações Nexus](../nexus/nexus-operations) لعملية الحوادث؛ قد تكون الحوكمة اوقفت المسار.

Para obter mais informações sobre o SDK, use `docs/source/nexus_sdk_quickstarts.md`.