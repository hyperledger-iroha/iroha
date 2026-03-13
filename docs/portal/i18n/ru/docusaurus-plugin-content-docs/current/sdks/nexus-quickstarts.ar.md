---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Установите флажок `docs/source/nexus_sdk_quickstarts.md`. Вы можете получить доступ к данным в разделе «Создание программного обеспечения» и использовать SDK. Он был убит в 2007 году в 2017 году.

## الاعداد المشترك

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

В случае установки Nexus, вы можете установить SDK и использовать его в дальнейшем. TLS используется для подключения (сообщение `docs/source/sora_nexus_operator_onboarding.md`).

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

Для получения дополнительной информации `ToriiClient` необходимо установить كتلة.

## Свифт

```bash
make swift-nexus-demo
```

Установите `Torii.Client` на `IrohaSwift` и `FindNetworkStatus`.

## Андроид

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

В 2017 году в фильме "Старый мир" была организована постановка фильма.

## интерфейс командной строки

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## استكشاف الاخطاء واصلاحها

- Откройте TLS -> Загрузить в CA файл в tar-архиве Nexus.
- `ERR_UNKNOWN_LANE` -> `--lane-id`/`--dataspace-id` находится в режиме ожидания.
- `ERR_SETTLEMENT_PAUSED` -> راجع [Nexus операции](../nexus/nexus-operations) لعملية الحوادث؛ Это произошло в 2007 году.

Загрузите файл SDK `docs/source/nexus_sdk_quickstarts.md`.