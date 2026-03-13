---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الدليل الكامل هو `docs/source/nexus_sdk_quickstarts.md`. تتخلص هذه البوابة من المتطلبات الأساسية وأوامر SDK حتى يتمكن المطورون من التحقق من تكوينهم بسرعة.

## مشاركة التكوين

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

قم بإضافة حزمة تكوين Nexus، وقم بتثبيتها كتبعيات لكل SDK وتضمن أن شهادات TLS تتوافق مع ملف الإصدار (حتى `docs/source/sora_nexus_operator_onboarding.md`).

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

مثيل البرنامج النصي `ToriiClient` كما هو الحال في البيئة المحيطة المتغيرة ويطبع الكتلة الأحدث.

## سويفت

```bash
make swift-nexus-demo
```

الولايات المتحدة الأمريكية `Torii.Client` do `IrohaSwift` للحافلة `FindNetworkStatus`.

## أندرويد

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

قم بتشغيل اختبار الجهاز الذي يتم إدارته بناءً على نقطة نهاية التدريج Nexus.

## كلي

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## حل المشاكل

- Falhas TLS -> قم بتأكيد حزمة CA لإصدار Nexus.
- `ERR_UNKNOWN_LANE` -> قم بتمرير `--lane-id`/`--dataspace-id` عند التدوير أو التدوير متعدد الممرات.
- `ERR_SETTLEMENT_PAUSED` -> التحقق من [عمليات Nexus](../nexus/nexus-operations) لمعالجة الحادث؛ يمكن للحاكم أن يوقف الممر.

لمزيد من السياق والشرح لـ SDK و`docs/source/nexus_sdk_quickstarts.md`.