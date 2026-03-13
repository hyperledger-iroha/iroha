---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

A guia completa esta em `docs/source/nexus_sdk_quickstarts.md`. Este resumo do portal destaca os prerequisitos compartilhados e os comandos por SDK para que os desenvolvedores verifiquem sua configuracao rapidamente.

## Configuracao compartilhada

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Baixe o pacote de configuracao do Nexus, instale as dependencias de cada SDK e garanta que os certificados TLS correspondam ao perfil de release (veja `docs/source/sora_nexus_operator_onboarding.md`).

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

O script instancia `ToriiClient` com as variaveis de ambiente acima e imprime o bloco mais recente.

## Swift

```bash
make swift-nexus-demo
```

Usa `Torii.Client` do `IrohaSwift` para buscar `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Executa o teste de dispositivo gerenciado apontando para o endpoint de staging do Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Solucao de problemas

- Falhas TLS -> confirme o bundle CA do tarball de release do Nexus.
- `ERR_UNKNOWN_LANE` -> passe `--lane-id`/`--dataspace-id` quando o roteamento multi-lane for imposto.
- `ERR_SETTLEMENT_PAUSED` -> verifique [Nexus operations](../nexus/nexus-operations) para o processo de incidente; a governanca pode ter pausado a lane.

Para mais contexto e explicacoes por SDK veja `docs/source/nexus_sdk_quickstarts.md`.
