---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الدليل الكامل موجود في `docs/source/nexus_sdk_quickstarts.md`. يسلط هذا الملخص في البوابة الضوء على المتطلبات المشتركة والوامر لكل SDK حتى يتمكن المتقدمون من التحقق من إعداداتهم بسرعة.

## الاعداد

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

قم بتنزيل حزمة تهيئة Nexus، وتوافق تبعيات كل SDK، وتأكد من تطابق شهادات TLS مع ملف الاصدار (انظر `docs/source/sora_nexus_operator_onboarding.md`).

## الصدأ

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

المراجع: `docs/source/sdk/rust.md`

## جافا سكريبت / تايب سكريبت

```bash
npm run demo:nexus
```

يقوم السكربت بتهيئة `ToriiClient` باستخدام متغيرات البيئة علاه ويطبع اخر كتلة.

## سويفت

```bash
make swift-nexus-demo
```

يستخدم `Torii.Client` من `IrohaSwift` لجلب `FindNetworkStatus`.

## أندرويد

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

الغرض من اختبار الجهاز المستهدف هو نقطة نهاية التدريج لنكسس.

## كلي

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## استكشاف الاخطاء واصلاحها

- إعطاءال TLS -> متأكد من حزمة CA القادمة من tarball Nexus.
- `ERR_UNKNOWN_LANE` -> مرر `--lane-id`/`--dataspace-id` عندما يتم فرض توجيه المسارات المتعددة.
- `ERR_SETTLEMENT_PAUSED` -> راجع [عمليات Nexus](../nexus/nexus-operations) الخطوة التالية؛ قد تكون لا توقف المسار.

للسياق الاضافي والشروح الخاصة بكل SDK `docs/source/nexus_sdk_quickstarts.md`.