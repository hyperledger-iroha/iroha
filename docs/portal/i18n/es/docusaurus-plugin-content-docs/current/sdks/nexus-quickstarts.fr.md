---
lang: es
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

La guía completa se encuentra en `docs/source/nexus_sdk_quickstarts.md`. Este resumen del portal cumple con los requisitos previos comunes y los comandos del SDK para que los desarrolladores verifiquen rápidamente su configuración.

## Configuración comuna

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Descargue el paquete de configuración Nexus, instale las dependencias de cada SDK y asegúrese de que los certificados TLS correspondan al perfil de lanzamiento (ver `docs/source/sora_nexus_operator_onboarding.md`).

## Óxido

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Referencias: `docs/source/sdk/rust.md`

## JavaScript/Mecanografiado

```bash
npm run demo:nexus
```

La instancia de script `ToriiClient` con las variables de entorno ci-dessus y muestra el último bloque.

## Rápido

```bash
make swift-nexus-demo
```

Utilice `Torii.Client` de `IrohaSwift` para recuperar `FindNetworkStatus`.

## androide

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Ejecute la prueba del dispositivo realizado con el punto de terminación de preparación Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Depannage

- Echecs TLS -> verificador del paquete CA del archivo comprimido de versión Nexus.
- `ERR_UNKNOWN_LANE` -> transeúnte `--lane-id`/`--dataspace-id` une fois le routage aplique multicarril.
- `ERR_SETTLEMENT_PAUSED` -> consultor [operaciones Nexus](../nexus/nexus-operations) para el proceso de incidente; la gouvernance a peut etre mis la lane en pausa.

Para más contexto y explicaciones del SDK, consulte `docs/source/nexus_sdk_quickstarts.md`.