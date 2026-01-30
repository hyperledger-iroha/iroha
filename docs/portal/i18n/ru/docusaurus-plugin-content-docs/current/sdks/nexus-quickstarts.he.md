---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sdks/nexus-quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df870d5fa5a2a8c0f11136acacb4224cb91d629869bfd637d0e19e2b8b58fd89
source_last_modified: "2025-11-14T04:43:21.096431+00:00"
translation_last_reviewed: 2026-01-30
---

Полный quickstart находится в `docs/source/nexus_sdk_quickstarts.md`. Этот обзор портала подчеркивает общие предпосылки и команды для каждого SDK, чтобы разработчики могли быстро проверить настройку.

## Общая настройка

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Скачайте конфигурационный пакет Nexus, установите зависимости каждого SDK и убедитесь, что TLS-сертификаты соответствуют профилю релиза (см. `docs/source/sora_nexus_operator_onboarding.md`).

## Rust

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

Скрипт инициализирует `ToriiClient` с переменными окружения выше и печатает последний блок.

## Swift

```bash
make swift-nexus-demo
```

Использует `Torii.Client` из `IrohaSwift`, чтобы получить `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Запускает тест управляемого устройства, обращающийся к staging-эндпоинту Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Устранение неполадок

- Сбои TLS -> проверьте CA bundle из tarball релиза Nexus.
- `ERR_UNKNOWN_LANE` -> передайте `--lane-id`/`--dataspace-id`, когда будет включена маршрутизация multi-lane.
- `ERR_SETTLEMENT_PAUSED` -> смотрите [Nexus operations](../nexus/nexus-operations) для процесса инцидента; возможно, governance приостановила lane.

Для более глубокого контекста и пояснений по SDK см. `docs/source/nexus_sdk_quickstarts.md`.
