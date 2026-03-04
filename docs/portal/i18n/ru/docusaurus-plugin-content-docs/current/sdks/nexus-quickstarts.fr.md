---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Полное руководство находится в `docs/source/nexus_sdk_quickstarts.md`. Это возобновление порта выполнено с учетом предварительных требований сообщества и команд SDK для проверяемой ускоренной настройки конфигурации.

## Конфигурация коммуны

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Выполните зарядку пакета конфигурации Nexus, установите зависимости каждого пакета SDK и убедитесь, что сертификаты TLS соответствуют профилю выпуска (смотрите `docs/source/sora_nexus_operator_onboarding.md`).

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

Экземпляр сценария `ToriiClient` с переменными окружающей среды и прикреплением последнего блока.

## Свифт

```bash
make swift-nexus-demo
```

Используйте `Torii.Client` от `IrohaSwift` для рекуператора `FindNetworkStatus`.

## Андроид

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Выполните проверку устройства, где будет указана точка завершения промежуточной подготовки Nexus.

## интерфейс командной строки

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Депаннаж

- Проверка TLS -> проверка пакета CA архива выпуска Nexus.
- `ERR_UNKNOWN_LANE` -> passer `--lane-id`/`--dataspace-id` является ненужным многополосным приложением маршрутизации.
- `ERR_SETTLEMENT_PAUSED` -> консультант [Nexus операции](../nexus/nexus-operations) для обработки инцидента; la gouvernance a peut etre mis la Lane en Pause.

Плюс контекст и пояснения в SDK см. `docs/source/nexus_sdk_quickstarts.md`.