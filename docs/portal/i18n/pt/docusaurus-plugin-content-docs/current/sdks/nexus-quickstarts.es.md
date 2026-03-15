---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

O guia completo está em `docs/source/nexus_sdk_quickstarts.md`. Este resumo do portal resalta os pré-requisitos compartilhados e os comandos do SDK para que os desenvolvedores verifiquem sua configuração rápida.

## Configuração compartilhada

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Descarregue o pacote de configuração Nexus, instale as dependências de cada SDK e confirme se os certificados TLS coincidem com o perfil de lançamento (ver `docs/source/sora_nexus_operator_onboarding.md`).

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

A instância do script `ToriiClient` com as variáveis de ambiente ascendente e imprime o último bloco.

## Rápido

```bash
make swift-nexus-demo
```

Usa `Torii.Client` de `IrohaSwift` para obter `FindNetworkStatus`.

##Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Execute o teste de dispositivo administrado que foi direcionado ao endpoint de teste de Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Solução de problemas

- Falha no TLS -> confirma o pacote CA do tarball de lançamento do Nexus.
- `ERR_UNKNOWN_LANE` -> pasa `--lane-id`/`--dataspace-id` quando o enrutamiento multi-lane sea é obrigatório.
- `ERR_SETTLEMENT_PAUSED` -> revisão [Nexus operações](../nexus/nexus-operations) para o processo de incidentes; la gobernanza pode pausar la lane.

Para mais contexto e explicações do SDK consulte `docs/source/nexus_sdk_quickstarts.md`.