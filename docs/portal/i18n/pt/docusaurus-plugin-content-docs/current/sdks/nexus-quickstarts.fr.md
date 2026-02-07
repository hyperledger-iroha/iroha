---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

O guia completo é encontrado em `docs/source/nexus_sdk_quickstarts.md`. Este resumo do portal atendeu aos pré-requisitos comuns e aos comandos do SDK para que os desenvolvedores verifiquem rapidamente sua configuração.

## Configuração da comunidade

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Baixe o pacote de configuração Nexus, instale as dependências de cada SDK e certifique-se de que os certificados TLS correspondentes ao perfil de lançamento (veja `docs/source/sora_nexus_operator_onboarding.md`).

## Ferrugem

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Referências: `docs/source/sdk/rust.md`

##JavaScript/TypeScript

```bash
npm run demo:nexus
```

A instância do script `ToriiClient` com as variáveis de ambiente ci-dessus e exibe o último bloco.

## Rápido

```bash
make swift-nexus-demo
```

Utilize `Torii.Client` de `IrohaSwift` para recuperar `FindNetworkStatus`.

##Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Execute o teste do dispositivo aqui, verificando o ponto de terminais de teste Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Depanagem

- Echecs TLS -> verifica o pacote CA do tarball de lançamento Nexus.
- `ERR_UNKNOWN_LANE` -> passe `--lane-id`/`--dataspace-id` uma vez para o roteamento do aplicativo multi-lane.
- `ERR_SETTLEMENT_PAUSED` -> consultor [operações Nexus](../nexus/nexus-operations) para o processo de incidente; la gouvernance a peut etre mis la lane en pause.

Para mais contexto e explicações do SDK, veja `docs/source/nexus_sdk_quickstarts.md`.