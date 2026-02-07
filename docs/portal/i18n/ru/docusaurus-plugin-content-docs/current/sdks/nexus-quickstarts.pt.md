---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Полное руководство по `docs/source/nexus_sdk_quickstarts.md`. Это резюме портала содержит предварительные требования для сопоставления и команды SDK для быстрой проверки вашей конфигурации.

## Настройка совместимости

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Пакет конфигурации Nexus можно установить как зависимость каждого SDK и гарантировать, что сертификаты TLS соответствуют версии выпуска (`docs/source/sora_nexus_operator_onboarding.md`).

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

Экземпляр сценария `ToriiClient` включает в себя варианты окружающей среды, а также загрузку или блокировку более недавнего режима.

## Свифт

```bash
make swift-nexus-demo
```

США `Torii.Client` до `IrohaSwift` для автобуса `FindNetworkStatus`.

## Андроид

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Выполнение проверки устройства происходит автоматически для конечной точки промежуточного выполнения Nexus.

## интерфейс командной строки

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Решение проблем

- Falhas TLS -> подтвердите пакет CA для выпуска архива Nexus.
- `ERR_UNKNOWN_LANE` -> passe `--lane-id`/`--dataspace-id`, когда многополосный или ротаменто для импоста.
- `ERR_SETTLEMENT_PAUSED` -> проверка [Nexus операции](../nexus/nexus-operations) для обработки инцидента; управление может пройти по переулку.

Для большего контекста и пояснений для SDK типа `docs/source/nexus_sdk_quickstarts.md`.