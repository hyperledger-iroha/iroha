---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sdks/nexus-quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6675f680feee60aa71881e3a5883599370a31fc663b2a935a36dd9305d1d6acd
source_last_modified: "2025-11-14T04:43:21.096631+00:00"
translation_last_reviewed: 2026-01-30
---

الدليل الكامل موجود في `docs/source/nexus_sdk_quickstarts.md`. يسلط هذا الملخص في البوابة الضوء على المتطلبات المشتركة والاوامر لكل SDK حتى يتمكن المطورون من التحقق من اعداداتهم بسرعة.

## الاعداد المشترك

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

قم بتنزيل حزمة تهيئة Nexus، وثبت تبعيات كل SDK، وتاكد من تطابق شهادات TLS مع ملف الاصدار (انظر `docs/source/sora_nexus_operator_onboarding.md`).

## Rust

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

المراجع: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

يقوم السكربت بتهيئة `ToriiClient` باستخدام متغيرات البيئة اعلاه ويطبع اخر كتلة.

## Swift

```bash
make swift-nexus-demo
```

يستخدم `Torii.Client` من `IrohaSwift` لجلب `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

يشغل اختبار الجهاز المدار الذي يستهدف نقطة نهاية staging لنكسس.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## استكشاف الاخطاء واصلاحها

- اعطال TLS -> تاكد من حزمة CA القادمة من tarball اصدار Nexus.
- `ERR_UNKNOWN_LANE` -> مرر `--lane-id`/`--dataspace-id` عندما يتم فرض التوجيه متعدد المسارات.
- `ERR_SETTLEMENT_PAUSED` -> راجع [Nexus operations](../nexus/nexus-operations) لعملية الحوادث؛ قد تكون الحوكمة اوقفت المسار.

للسياق الاضافي والشروح الخاصة بكل SDK راجع `docs/source/nexus_sdk_quickstarts.md`.
