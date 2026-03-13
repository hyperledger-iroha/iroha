---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Полная книга находится в `docs/source/nexus_sdk_quickstarts.md`. На портале возобновляются резервные копии предварительных требований и команды для SDK, чтобы выполнить быструю проверку конфигурации.

## Совместная конфигурация

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Выгрузите пакет конфигурации Nexus, установите зависимости каждого SDK и подтвердите, что сертификаты TLS совпадают с версией `docs/source/sora_nexus_operator_onboarding.md`.

## Ржавчина

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Ссылки: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

Экземпляр сценария `ToriiClient` с переменными прибытия и ввода последнего блока.

## Свифт

```bash
make swift-nexus-demo
```

Используйте `Torii.Client` от `IrohaSwift` для получения `FindNetworkStatus`.

## Андроид

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Выполните запуск административного устройства, которое будет подключено к конечной точке промежуточного хранения Nexus.

## интерфейс командной строки

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Решение проблем

- Сбой TLS -> подтверждение пакета CA архива выпуска Nexus.
- `ERR_UNKNOWN_LANE` -> pasa `--lane-id`/`--dataspace-id`, когда требуется многополосное морское плавание.
- `ERR_SETTLEMENT_PAUSED` -> проверка [Nexus операции](../nexus/nexus-operations) для обработки инцидентов; la gobernanza pudo pausar la Lane.

Для контекста и объяснений для SDK обратитесь к `docs/source/nexus_sdk_quickstarts.md`.