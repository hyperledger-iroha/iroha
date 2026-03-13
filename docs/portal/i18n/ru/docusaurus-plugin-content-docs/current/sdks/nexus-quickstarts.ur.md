---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Краткое руководство по `docs/source/nexus_sdk_quickstarts.md`. Используйте встроенный SDK, чтобы получить больше информации о SDK. ہے تاکہ ڈویلپرز اپنی سیٹ اپ جلدی جانچ سکیں۔

## مشترکہ سیٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus Может быть использован SDK или SDK. Используйте TLS, чтобы получить доступ к данным по TLS. (دیکھیے `docs/source/sora_nexus_operator_onboarding.md`).

## Ржавчина

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Сообщение: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

Для получения дополнительной информации обратитесь к `ToriiClient`. ترین بلاک پرنٹ کرتا ہے۔

## Свифт

```bash
make swift-nexus-demo
```

`IrohaSwift` کے `Torii.Client` سے `FindNetworkStatus` حاصل کرتا ہے۔

## Андроид

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Если вы хотите использовать промежуточную конечную точку Nexus, вы можете использовать промежуточную конечную точку.

## интерфейс командной строки

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## مسئلہ حل

- TLS-файл -> Nexus tarball سے CA بنڈل کی توثیق کریں۔
- `ERR_UNKNOWN_LANE` -> Многополосная маршрутизация в `--lane-id`/`--dataspace-id` دیں۔
- `ERR_SETTLEMENT_PAUSED` -> واقعہ عمل کے لیے [Nexus операции](../nexus/nexus-operations) دیکھیں؛ ممکن ہے گورننس نے переулок روک دی ہو۔

Загрузите и установите SDK для использования с `docs/source/nexus_sdk_quickstarts.md`.