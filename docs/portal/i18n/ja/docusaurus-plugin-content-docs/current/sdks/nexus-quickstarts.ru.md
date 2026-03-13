---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`docs/source/nexus_sdk_quickstarts.md` によるクイックスタート。 Этот обзор портала подчеркивает общие предпосылки и команды для каждого SDK, чтобы разработчики могли быстроそうです。

## Общая настройка

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Скачайте конфигурационный пакет Nexus, установите зависимости каждого SDK и убедитесь, что TLS-сертификаты соответствуют профилю релиза (см. `docs/source/sora_nexus_operator_onboarding.md`)。

## 錆びる

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

`ToriiClient` は、最高のパフォーマンスを提供します。

## スウィフト

```bash
make swift-nexus-demo
```

`Torii.Client` または `IrohaSwift`、`FindNetworkStatus` を確認してください。

## アンドロイド

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

- TLS -> CA バンドルと tarball の Nexus を確認します。
- `ERR_UNKNOWN_LANE` -> передайте `--lane-id`/`--dataspace-id`、マルチレーン。
- `ERR_SETTLEMENT_PAUSED` -> [Nexus 操作](../nexus/nexus-operations) を実行します。 возможно、ガバナンスはレーンです。

SDK を使用すると、SDK が更新されます。 `docs/source/nexus_sdk_quickstarts.md`。