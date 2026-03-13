---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Полный краткий старт находится в `docs/source/nexus_sdk_quickstarts.md`. Этот обзорный портал использует общие предпосылки и команды для каждого SDK, чтобы разработчики могли быстро проверить качество.

## Общая настройка

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Настройте конфигурационный пакет Nexus, установите в зависимости от каждого SDK и убедитесь, что TLS-сертификаты соответствуют профилю релиза (см. `docs/source/sora_nexus_operator_onboarding.md`).

## Ржавчина

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

См.: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

Скрипт реализует `ToriiClient` с переменными окружениями выше и печатает последний блок.

## Свифт

```bash
make swift-nexus-demo
```

Использует `Torii.Client` из `IrohaSwift`, чтобы получить `FindNetworkStatus`.

## Андроид

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Запускает тестовое управляемое устройство, обращаясь к staging-эндпоинту Nexus.

## интерфейс командной строки

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Устранение неполадок

- Сбои TLS -> проверьте пакет CA из архива версии Nexus.
- `ERR_UNKNOWN_LANE` -> передайте `--lane-id`/`--dataspace-id`, когда будет включена маршрутизация многополосной.
- `ERR_SETTLEMENT_PAUSED` -> смотрите [Nexus операции](../nexus/nexus-operations) для инцидента в процессе; Возможно, управление приостановила пер.

Для более глубокого контекста и пояснений по SDK см. `docs/source/nexus_sdk_quickstarts.md`.