---
lang: he
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Полный התחלה מהירה находится в `docs/source/nexus_sdk_quickstarts.md`. Этот обзор портала подчеркивает общие предпосылки и команды для каждого SDK, чтобы разработчики моглип.

## Общая настройка

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

צור תיקי Nexus, התקן את ה-SDK של ה-SDK ואת הציוד, התקן את ה-TSK соответствуют профилю релиза (см. `docs/source/sora_nexus_operator_onboarding.md`).

## חלודה

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

סמ.: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

סקריפט инициализирует `ToriiClient` с переменными окружения выше и печатает последний блок.

## סוויפט

```bash
make swift-nexus-demo
```

Использует `Torii.Client` из `IrohaSwift`, чтобы получить `FindNetworkStatus`.

## אנדרואיד

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

- Сбои TLS -> הצג חבילה של CA ב-tarball релиза Nexus.
- `ERR_UNKNOWN_LANE` -> передайте `--lane-id`/`--dataspace-id`, когда будет включена маршрутизация multi-lane.
- `ERR_SETTLEMENT_PAUSED` -> смотрите [Nexus פעולות](../nexus/nexus-operations) для процесса инцидента; возможно, governance приостановила ליין.

Для более глубокого контекста и пояснений по SDK см. `docs/source/nexus_sdk_quickstarts.md`.