---
lang: es
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

La guia completa esta en `docs/source/nexus_sdk_quickstarts.md`. Este resumen del portal resalta los prerrequisitos compartidos y los comandos por SDK para que los desarrolladores verifiquen su configuracion rapido.

## Configuracion compartida

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Descarga el paquete de configuracion de Nexus, instala las dependencias de cada SDK y confirma que los certificados TLS coinciden con el perfil de release (ver `docs/source/sora_nexus_operator_onboarding.md`).

## Rust

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Refs: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

El script instancia `ToriiClient` con las variables de entorno de arriba e imprime el ultimo bloque.

## Swift

```bash
make swift-nexus-demo
```

Usa `Torii.Client` de `IrohaSwift` para obtener `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Ejecuta la prueba de dispositivo administrado que apunta al endpoint de staging de Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Solucion de problemas

- Fallas TLS -> confirma el bundle CA del tarball de release de Nexus.
- `ERR_UNKNOWN_LANE` -> pasa `--lane-id`/`--dataspace-id` cuando el enrutamiento multi-lane sea obligatorio.
- `ERR_SETTLEMENT_PAUSED` -> revisa [Nexus operations](../nexus/nexus-operations) para el proceso de incidentes; la gobernanza pudo pausar la lane.

Para mas contexto y explicaciones por SDK consulta `docs/source/nexus_sdk_quickstarts.md`.
