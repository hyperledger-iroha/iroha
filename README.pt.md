---
lang: pt
direction: ltr
source: README.md
status: complete
translator: manual
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:00:00+00:00"
translation_last_reviewed: 2026-02-07
---

# Hyperledger Iroha

[![Licença](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha é uma plataforma de blockchain determinística para implantações permissionadas e de consórcio. Ela oferece gestão de contas e ativos, permissões on-chain e contratos inteligentes por meio da Iroha Virtual Machine (IVM).

> O estado do workspace e as mudanças recentes são registrados em [`status.md`](./status.md).

## Linhas de release

Este repositório publica duas linhas de implantação a partir da mesma base de código:

- **Iroha 2**: redes permissionadas/de consórcio auto-hospedadas.
- **Iroha 3 (SORA Nexus)**: linha orientada ao Nexus usando os mesmos crates centrais.

As duas linhas compartilham os mesmos componentes principais, incluindo serialização Norito, consenso Sumeragi e o toolchain Kotodama -> IVM.

## Estrutura do repositório

- [`crates/`](./crates): crates Rust principais (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito` etc.).
- [`integration_tests/`](./integration_tests): testes de integração e de rede entre componentes.
- [`IrohaSwift/`](./IrohaSwift): pacote SDK Swift.
- [`java/iroha_android/`](./java/iroha_android): pacote SDK Android.
- [`docs/`](./docs): documentação para usuários, operações e desenvolvimento.

## Início rápido

### Pré-requisitos

- [Rust estável](https://www.rust-lang.org/tools/install)
- Opcional: Docker + Docker Compose para execuções locais com múltiplos peers

### Build e testes (workspace)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Notas:

- O build completo do workspace pode levar cerca de 20 minutos.
- Os testes completos do workspace podem levar várias horas.
- O workspace tem como alvo `std` (builds WASM/no-std não são suportados).

### Comandos de teste direcionados

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### Comandos de teste dos SDKs

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## Executar uma rede local

Inicie a rede Docker Compose fornecida:

```bash
docker compose -f defaults/docker-compose.yml up
```

Use o CLI com a configuração de cliente padrão:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Para passos de implantação nativa do daemon, veja [`crates/irohad/README.md`](./crates/irohad/README.md).

## API e observabilidade

O Torii expõe APIs Norito e JSON. Endpoints operacionais comuns:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Veja a referência completa de endpoints em:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Crates principais

- [`crates/iroha`](./crates/iroha): biblioteca cliente.
- [`crates/irohad`](./crates/irohad): binários do daemon peer.
- [`crates/iroha_cli`](./crates/iroha_cli): CLI de referência.
- [`crates/iroha_core`](./crates/iroha_core): motor de execução e núcleo do ledger.
- [`crates/iroha_config`](./crates/iroha_config): modelo tipado de configuração.
- [`crates/iroha_data_model`](./crates/iroha_data_model): modelo de dados canônico.
- [`crates/iroha_crypto`](./crates/iroha_crypto): primitivas criptográficas.
- [`crates/norito`](./crates/norito): codec de serialização determinística.
- [`crates/ivm`](./crates/ivm): Iroha Virtual Machine.
- [`crates/iroha_kagami`](./crates/iroha_kagami): ferramentas de chaves/gênesis/configuração.

## Mapa da documentação

- Índice principal: [`docs/README.md`](./docs/README.md)
- Gênesis: [`docs/genesis.md`](./docs/genesis.md)
- Consenso (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Pipeline de transações: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- Internos P2P: [`docs/source/p2p.md`](./docs/source/p2p.md)
- Syscalls do IVM: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Gramática Kotodama: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Formato wire do Norito: [`norito.md`](./norito.md)
- Acompanhamento do trabalho atual: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Traduções

Visão geral em japonês: [`README.ja.md`](./README.ja.md)

Outras visões gerais:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Fluxo de tradução: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Contribuição e ajuda

- Guia de contribuição: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Canais de comunidade/suporte: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Licença

O Iroha é licenciado sob Apache-2.0. Veja [`LICENSE`](./LICENSE).

A documentação é licenciada sob CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/
