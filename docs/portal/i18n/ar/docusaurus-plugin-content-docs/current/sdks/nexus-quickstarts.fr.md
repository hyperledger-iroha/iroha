---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

تم العثور على الدليل كاملاً في `docs/source/nexus_sdk_quickstarts.md`. يتم استئناف هذا المنفذ من خلال المتطلبات الأساسية المشتركة وأوامر SDK حتى يتمكن المطورون من التحقق من تكوينهم بسرعة.

## جماعة التكوين

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

قم بتنزيل حزمة التكوين Nexus، وقم بتثبيت تبعيات كل SDK وتأكد من أن شهادات TLS تتوافق مع ملف تعريف الإصدار (يظهر `docs/source/sora_nexus_operator_onboarding.md`).

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

يظهر مثيل البرنامج النصي `ToriiClient` مع متغيرات البيئة الخاصة به ويعرض الكتلة الأحدث.

## سويفت

```bash
make swift-nexus-demo
```

استخدم `Torii.Client` de `IrohaSwift` لاسترداد `FindNetworkStatus`.

## أندرويد

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

قم بتنفيذ اختبار الجهاز الذي يشير إلى نقطة إنهاء التدريج Nexus.

## كلي

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## ديباناج

- Echecs TLS -> التحقق من حزمة CA لإصدار Tarball Nexus.
- `ERR_UNKNOWN_LANE` -> تمرير `--lane-id`/`--dataspace-id` مرة أخرى للتوجيه متعدد المسارات.
- `ERR_SETTLEMENT_PAUSED` -> مستشار [عمليات Nexus](../nexus/nexus-operations) لمعالجة الحادث؛ يمكن للحكم أن يكون خاطئًا مؤقتًا.

لمزيد من السياق والشرح لـ SDK، راجع `docs/source/nexus_sdk_quickstarts.md`.