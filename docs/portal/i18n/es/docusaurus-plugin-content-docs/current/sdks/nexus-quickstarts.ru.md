---
lang: es
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Inicio rápido de inicio rápido en `docs/source/nexus_sdk_quickstarts.md`. Este portal portátil contiene predisposiciones y comandos para el SDK del archivo, que pueden proporcionar mayor información sobre la instalación.

## Общая настройка

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Descargue el paquete de configuración Nexus, instale el SDK del archivo de configuración y descargue los certificados TLS соответствуют профилю релиза (см. `docs/source/sora_nexus_operator_onboarding.md`).

## Óxido

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

См.: `docs/source/sdk/rust.md`

## JavaScript/Mecanografiado

```bash
npm run demo:nexus
```

El script se inicia con `ToriiClient` en el bloque de configuración permanente.

## Rápido

```bash
make swift-nexus-demo
```

Utilice `Torii.Client` y `IrohaSwift`, luego utilice `FindNetworkStatus`.

## androide

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Запускает тест управляемого устройства, обращающийся к подпоинту Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Устранение неполадок

- Utilice TLS -> proteja el paquete CA en la versión tarball Nexus.
- `ERR_UNKNOWN_LANE` -> передайте `--lane-id`/`--dataspace-id`, когда будет включена маршрутизация multicarril.
- `ERR_SETTLEMENT_PAUSED` -> смотрите [Operaciones Nexus](../nexus/nexus-operations) для процесса инцидента; возможно, gobernanza приостановила carril.

Este es un gran conjunto de contenidos y archivos de SDK. `docs/source/nexus_sdk_quickstarts.md`.