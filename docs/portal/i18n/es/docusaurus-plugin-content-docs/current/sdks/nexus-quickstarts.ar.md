---
lang: es
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الدليل الكامل موجود في `docs/source/nexus_sdk_quickstarts.md`. يسلط هذا الملخص في البوابة الضوء على المتطلبات المشتركة y لكل SDK حتى يتمكن المطورون من التحقق من اعداداتهم بسرعة.

## الاعداد المشترك

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Para instalar el software Nexus, instalar el SDK y utilizar TLS para conectarlo ( `docs/source/sora_nexus_operator_onboarding.md`).

## Óxido

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Título: `docs/source/sdk/rust.md`

## JavaScript/Mecanografiado

```bash
npm run demo:nexus
```

يقوم السكربت بتهيئة `ToriiClient` باستخدام متغيرات البيئة اعلاه ويطبع اخر كتلة.

## Rápido

```bash
make swift-nexus-demo
```

Conecte `Torii.Client` a `IrohaSwift` y `FindNetworkStatus`.

## androide

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

يشغل اختبار الجهاز المدار الذي يستهدف نقطة نهاية puesta en escena لنكسس.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## استكشاف الاخطاء واصلاحها

- Utilice TLS -> Para acceder a CA desde el tarball Nexus.
- `ERR_UNKNOWN_LANE` -> مرر `--lane-id`/`--dataspace-id` عندما يتم فرض التوجيه متعدد المسارات.
- `ERR_SETTLEMENT_PAUSED` -> راجع [Operaciones Nexus](../nexus/nexus-operations) لعملية الحوادث؛ قد تكون الحوكمة اوقفت المسار.

Utilice el SDK `docs/source/nexus_sdk_quickstarts.md` para configurar y configurar archivos.