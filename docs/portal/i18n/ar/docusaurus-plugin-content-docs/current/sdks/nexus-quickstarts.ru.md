---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

تأتي البداية السريعة الكاملة إلى `docs/source/nexus_sdk_quickstarts.md`. توفر بوابة البحث هذه جميع الاقتراحات والأوامر الخاصة بـ SDK التي يمكن للمصممين التحقق من صحتها.

## التركيب الجيد

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

قم بتنزيل حزمة التكوين Nexus، وحافظ على دقة SDK وانضم إلى شهادة TLS نسخة الملف الشخصي (سم. `docs/source/sora_nexus_operator_onboarding.md`).

## الصدأ

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

سم .: `docs/source/sdk/rust.md`

## جافا سكريبت / تايب سكريبت

```bash
npm run demo:nexus
```

يتم تهيئة البرنامج النصي `ToriiClient` مع الحفظ الإضافي وحذف الكتلة التالية.

## سويفت

```bash
make swift-nexus-demo
```

استخدم `Torii.Client` من `IrohaSwift` لتحصل على `FindNetworkStatus`.

## أندرويد

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

قم بإجراء اختبار التحكم في الجهاز، والذي يتم ربطه بنقطة التدريج Nexus.

## كلي

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Устранение неполадок

- استخدم TLS -> تحقق من حزمة CA من إصدار tarball Nexus.
- `ERR_UNKNOWN_LANE` -> انتقل إلى `--lane-id`/`--dataspace-id`، عندما يتضمن التخطيط متعدد المسارات.
- `ERR_SETTLEMENT_PAUSED` -> امسح [عمليات Nexus](../nexus/nexus-operations) لحادثة العملية؛ من الممكن أن يؤدي الحكم إلى حارة.

لمزيد من المحتوى والتفاعل مع SDK sm. `docs/source/nexus_sdk_quickstarts.md`.