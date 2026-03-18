---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Veja o guia de início rápido em `docs/source/nexus_sdk_quickstarts.md`. Este portal permite que você obtenha recursos e comandos para o SDK, para que você possa instalar o seu site проверить настройку.

## Общая настройка

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Baixe o pacote de configuração Nexus, instale o SDK e atualize-o TLS-сертификаты соответствуют профилю релиза (см. `docs/source/sora_nexus_operator_onboarding.md`).

## Ferrugem

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Exemplo: `docs/source/sdk/rust.md`

##JavaScript/TypeScript

```bash
npm run demo:nexus
```

Скрипт инициализирует `ToriiClient` com a configuração atual e o bloqueio do bloco.

## Rápido

```bash
make swift-nexus-demo
```

Use `Torii.Client` ou `IrohaSwift`, selecione `FindNetworkStatus`.

##Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Para executar o teste de teste, execute o teste de teste Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Устранение неполадок

- Сбои TLS -> prove o pacote CA da versão tarball Nexus.
- `ERR_UNKNOWN_LANE` -> передайте `--lane-id`/`--dataspace-id`, когда будет включена маршрутизация multi-lane.
- `ERR_SETTLEMENT_PAUSED` -> смотрите [operações Nexus](../nexus/nexus-operations) para o processo de início; возможно, governança приостановила lane.

Para maior conexão de globo e uso no SDK см. `docs/source/nexus_sdk_quickstarts.md`.