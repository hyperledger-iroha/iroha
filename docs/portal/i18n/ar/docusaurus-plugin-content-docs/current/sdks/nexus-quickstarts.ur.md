---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

مكتمل Quickstart `docs/source/nexus_sdk_quickstarts.md` موجود. تم طلب ملخص للبوابة وإمكانية تطوير SDK لإدارة البيانات وتفعيل موقع ويب Microsoft.

## مشترکہ سيٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

تتيح تهيئة Nexus إمكانية تنزيل ملفات SDK وSDK الكاملة للإنترنت، بالإضافة إلى إحدى الميزات الرائعة لشبكة TLS المتميزة الرقم (رقم `docs/source/sora_nexus_operator_onboarding.md`).

## الصدأ

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

حوالہ: `docs/source/sdk/rust.md`

## جافا سكريبت / تايب سكريبت

```bash
npm run demo:nexus
```

تم تحويل التحولات البرمجية الأولى إلى `ToriiClient` وهي الآن أحدث نقرات سوداء.

## سويفت

```bash
make swift-nexus-demo
```

`IrohaSwift` إلى `Torii.Client` `FindNetworkStatus` حصل على حق.

## أندرويد

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

منظم كل يوم يائس و Nexus هو نقطة نهاية التدريج التي لا نهاية لها.

## كلي

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## مسئلہ حل

- اسم TLS -> Nexus tarball هي بطاقة الائتمان المعتمدة في كاليفورنيا.
- `ERR_UNKNOWN_LANE` -> يجب توجيه التوجيه متعدد المسارات إلى `--lane-id`/`--dataspace-id`.
- `ERR_SETTLEMENT_PAUSED` -> عملية فعلية [عمليات Nexus](../nexus/nexus-operations) د. قد لا يكون هناك مسار ترفيهي.

المزيد من السباق والسباق وSDK مخصوص وضاحتوں لـ `docs/source/nexus_sdk_quickstarts.md` د.