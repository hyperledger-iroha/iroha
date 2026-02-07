---
lang: es
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

La guia completa esta en `docs/source/nexus_sdk_quickstarts.md`. Este resumen del portal resalta los requisitos compartidos y los comandos por SDK para que los desarrolladores verifiquen su configuración rápida.

## Configuración compartida

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Descargue el paquete de configuración de Nexus, instale las dependencias de cada SDK y confirme que los certificados TLS coinciden con el perfil de liberación (ver `docs/source/sora_nexus_operator_onboarding.md`).

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

La instancia de script `ToriiClient` con las variables de entorno de arriba e imprime el último bloque.

## Rápido

```bash
make swift-nexus-demo
```

Usa `Torii.Client` de `IrohaSwift` para obtener `FindNetworkStatus`.

## androide

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Ejecuta la prueba de dispositivo administrado que apunta al endpoint de staging de Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Solución de problemas

- Fallas TLS -> confirma el paquete CA del tarball de lanzamiento de Nexus.
- `ERR_UNKNOWN_LANE` -> pasa `--lane-id`/`--dataspace-id` cuando el enrutamiento multicarril sea obligatorio.
- `ERR_SETTLEMENT_PAUSED` -> revisa [Nexus operaciones](../nexus/nexus-operations) para el proceso de incidentes; la gobernanza pudo pausar el carril.

Para más contexto y explicaciones por SDK consulte `docs/source/nexus_sdk_quickstarts.md`.