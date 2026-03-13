---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الدليل كامل في `docs/source/nexus_sdk_quickstarts.md`. يعرض هذا الاستئناف للبوابة المتطلبات المسبقة المشتركة والأوامر بواسطة SDK حتى يتمكن المبرمجون من التحقق من تكوينهم بسرعة.

## حصة التكوين

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

قم بتنزيل حزمة تكوين Nexus، وقم بتثبيت تبعيات كل SDK وتأكد من أن شهادات TLS تتزامن مع ملف الإصدار (الإصدار `docs/source/sora_nexus_operator_onboarding.md`).

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

مثيل البرنامج النصي `ToriiClient` مزود بمتغيرات التشغيل وطباعة الكتلة الأخيرة.

## سويفت

```bash
make swift-nexus-demo
```

الولايات المتحدة الأمريكية `Torii.Client` de `IrohaSwift` للحصول على `FindNetworkStatus`.

## أندرويد

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

قم بتشغيل اختبار الجهاز المُدار الذي يصل إلى نقطة النهاية المرحلية لـ Nexus.

## كلي

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## حل المشكلة

- فشل TLS -> تأكيد حزمة CA لإصدار Nexus.
- `ERR_UNKNOWN_LANE` -> انتقل إلى `--lane-id`/`--dataspace-id` عند الدخول إلى البحر متعدد الممرات.
- `ERR_SETTLEMENT_PAUSED` -> مراجعة [عمليات Nexus](../nexus/nexus-operations) لعملية الأحداث؛ La gobernanza Pudo Pausar La Lane.

لمزيد من السياق والتفسيرات من خلال SDK، راجع `docs/source/nexus_sdk_quickstarts.md`.