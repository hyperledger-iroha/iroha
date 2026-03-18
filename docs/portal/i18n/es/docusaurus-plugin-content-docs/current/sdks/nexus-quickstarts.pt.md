---
lang: es
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

La guía completa está en `docs/source/nexus_sdk_quickstarts.md`. Este resumen del portal destaca los requisitos previos compartidos y los comandos por SDK para que los desarrolladores verifiquen su configuración rápidamente.

## Configuracao compartilhada

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Bajo el paquete de configuración de Nexus, instálelo como dependencias de cada SDK y garantice que los certificados TLS corresponden al perfil de liberación (veja `docs/source/sora_nexus_operator_onboarding.md`).

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

La instancia de script `ToriiClient` con variaciones de ambiente acima e imprime el bloque más reciente.

## Rápido

```bash
make swift-nexus-demo
```

Usa `Torii.Client` do `IrohaSwift` para buscar `FindNetworkStatus`.

## androide

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Ejecute la prueba del dispositivo gerenciado activando el punto final de preparación de Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Solución de problemas

- Falhas TLS -> confirme el paquete CA del tarball de lanzamiento de Nexus.
- `ERR_UNKNOWN_LANE` -> pase `--lane-id`/`--dataspace-id` cuando el roteamento multicarril para imposto.
- `ERR_SETTLEMENT_PAUSED` -> verificar [Nexus operaciones](../nexus/nexus-operations) para o proceso de incidente; a gobernadora pode ter pausado un carril.

Para más contexto y explicaciones por SDK veja `docs/source/nexus_sdk_quickstarts.md`.