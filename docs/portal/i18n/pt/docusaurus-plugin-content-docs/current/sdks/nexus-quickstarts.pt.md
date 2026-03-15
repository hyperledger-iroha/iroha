---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

O guia completo está em `docs/source/nexus_sdk_quickstarts.md`. Este resumo do portal destaca os pré-requisitos compartilhados e os comandos por SDK para que os desenvolvedores verifiquem sua configuração rapidamente.

## Configuração compartilhada

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Baixe o pacote de configuração do Nexus, instale as dependências de cada SDK e garanta que os certificados TLS correspondam ao perfil de lançamento (veja `docs/source/sora_nexus_operator_onboarding.md`).

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

O script instância `ToriiClient` com as variáveis de ambiente acima e imprime o bloco mais recente.

## Rápido

```bash
make swift-nexus-demo
```

Usa `Torii.Client` do `IrohaSwift` para buscar `FindNetworkStatus`.

##Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Executa o teste de dispositivo gerenciado apontando para o endpoint de staging do Nexus.

##CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Solução de problemas

- Falhas TLS -> confirme o pacote CA do tarball de lançamento do Nexus.
- `ERR_UNKNOWN_LANE` -> passe `--lane-id`/`--dataspace-id` quando o roteamento multi-lane para imposto.
- `ERR_SETTLEMENT_PAUSED` -> verifique [Nexus operações](../nexus/nexus-operations) para o processo de incidente; a governança pode ter pausado a pista.

Para mais contexto e explicações por SDK veja `docs/source/nexus_sdk_quickstarts.md`.