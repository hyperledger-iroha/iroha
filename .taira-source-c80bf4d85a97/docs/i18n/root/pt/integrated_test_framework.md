---
lang: pt
direction: ltr
source: integrated_test_framework.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff9e1108802fdd57703749069f87270c4195f4037a32aa65c28cde9a67b63e98
source_last_modified: "2025-11-02T04:40:28.026560+00:00"
translation_last_reviewed: 2026-01-21
---

# Estrutura de testes de integração para Hyperledger Iroha (rede de 7 nós)

## Introdução

O Hyperledger Iroha v2 fornece um conjunto rico de Instruções Especiais do Iroha (ISI) para gerenciamento de domínios, ativos, permissões e pares. Este documento especifica uma estrutura de testes de integração para uma rede de 7 pares para validar correção, consenso e consistência entre pares em condições normais e com falhas (tolerando até 2 pares com falhas).

Essas diretrizes refletem a migração recente do esquema de gênese para blocos com múltiplas transações.

Nota sobre a API: nesta base de código, Torii é uma API HTTP/WebSocket (Axum). Os testes devem usar endpoints HTTP (comumente portas 8080+). Nenhum serviço RPC adicional escuta em 50051.

## Objetivos

- Orquestração automatizada de 7 pares: iniciar e gerenciar pares programaticamente para CI rápido (principal) ou via Docker Compose (opcional).
- Configuração de gênese: iniciar a partir de uma gênese Norito comum com contas/chaves determinísticas e permissões necessárias.
- Exercitar ISI: cobrir ISI de forma sistemática via transações e verificar o estado resultante.
- Consistência entre pares: consultar vários pares após cada etapa para garantir estado de ledger idêntico.
- Verificações de tolerância a falhas: com até 2 pares offline ou particionados, os pares restantes continuam; os pares que retornam alcançam o estado sem divergências.

## Modos de orquestração

- Harness em Rust (recomendado): `crates/iroha_test_network` inicia processos `irohad` localmente, aloca portas, grava configs por execução e gênese Norito `.nrt`, monitora prontidão e alturas de bloco, e fornece utilitários de desligamento/reinício.
- Docker Compose (opcional): `crates/iroha_swarm` gera um arquivo Compose para N pares. Use quando uma rede conteinerizada ou orquestração externa for desejada.

## Configuração da rede de testes

**Bloco de gênese:**

- Fonte: `defaults/genesis.json` que agora agrupa instruções em um array `transactions`. Os testes podem anexar instruções com `NetworkBuilder::with_genesis_instruction` e iniciar uma nova transação via `.next_genesis_transaction()`. O bloco resultante é serializado em Norito `.nrt`.
- Topologia: armazenada na primeira transação (`transactions[0].topology`) e inclui todos os 7 pares (chave pública + endereço), para que cada par conheça a rede desde o início.
- Contas/Permissões: preferir contas padronizadas de `crates/iroha_test_samples` (`ALICE_ID`, `BOB_ID`, `SAMPLE_GENESIS_ACCOUNT_KEYPAIR`) com concessões explícitas para cenários de teste, por ex. `CanManagePeers`, `CanManageRoles`, `CanMintAssetWithDefinition`.
- Estratégia de injeção: com o harness, a gênese normalmente é fornecida a um par (o “genesis submitter”); outros pares se atualizam via sincronização de blocos. Com Compose, aponte todos os pares para o mesmo caminho de gênese.

**Rede e portas:**

- API HTTP Torii: `API_ADDRESS` (Axum). Para Compose, mapeie `8080`–`8086` para 7 pares no host. O harness aloca portas de loopback automaticamente.
- P2P: o endereço peer-to-peer interno é configurado em `trusted_peers` e difundido por gossip. O harness define `trusted_peers` automaticamente por execução.

**Armazenamento de dados:**

- Kura: armazenamento de blocos do Iroha v2 (não requer contêiner RocksDB/Postgres). Configure via `[kura]` (por ex., `store_dir`). Desative snapshots para testes via `[snapshot]`.

**Fluxo do harness:**

1) Construir rede: `NetworkBuilder::new().with_peers(7)`; opcionalmente `.with_pipeline_time(...)` e `.with_config_layer(...)` para overrides; escolher combustível IVM via `IvmFuelConfig`.
2) Iniciar pares: `.start()` ou `.start_blocking()`; grava camadas de configuração, define `trusted_peers`, injeta a gênese para um par e aguarda prontidão.
3) Prontidão: `Network::ensure_blocks(height)` ou `once_blocks_sync(...)` garante que alturas de bloco não vazias atinjam as expectativas entre pares. Alternativamente, consulte `Client::get_status()`.

**Fluxo Docker Compose (opcional):**

1) Gerar compose: Use `iroha_swarm::Swarm` para emitir um arquivo compose com N pares. Mapeie portas de API para o host e defina variáveis de ambiente (CHAIN, chaves, TRUSTED_PEERS, GENESIS).
2) Iniciar: `docker compose up`.
3) Prontidão: sonde endpoints HTTP Torii até que o status esteja saudável e a altura de bloco >= 1.

## Implementação do harness de testes (Rust)

**Cliente e transações:**

- Use `iroha::client::Client` (HTTP/WebSocket) para submeter transações e consultas ao Torii.
- Construa transações a partir de sequências ISI `InstructionBox` ou bytecode IVM; assine com valores determinísticos de `KeyPair` de `iroha_test_samples`.
- Chamadas úteis: `submit_blocking`, `submit_all_blocking`, `query(...).execute()/execute_all()`, `get_status()`, e streams de blocos e eventos via WebSocket.

**Consistência entre pares:**

- Após cada operação, consulte cada par em execução (`Network::peers()` -> `peer.client()`) e compare resultados (por ex., saldos, definições, listas de pares). Isso garante verificações de consistência além da verificação em um único par.

**Injeção de falhas (sem contêineres):**

- Use os utilitários de relay/proxy em `integration_tests/tests/extra_functional/unstable_network.rs` para reescrever `trusted_peers` para proxies TCP e suspender links seletivamente. Isso permite partições, perdas e reconexões direcionadas.

**Pré-requisitos do IVM:**

- Alguns testes exigem amostras IVM pré-construídas; garanta que `crates/ivm/target/prebuilt/build_config.toml` exista. Quando ausente, os testes podem ser ignorados (os testes de integração atuais fazem essa checagem).

## Esboço de cenário de 7 nós

1) Inicie uma rede de 7 pares (harness ou Compose) e aguarde o commit de gênese em todos os pares.
2) Execute uma suíte de ISI:
   - Registrar domínios/contas/ativos; conceder/revogar permissões; cunhar/queimar/transferir ativos; definir/remover chave‑valor; registrar triggers; atualizar o executor.
3) Após cada etapa lógica, execute consultas entre pares para verificar o estado idêntico.
4) Pare 1–2 pares, continue enviando transações com os 5 pares restantes; assegure progresso e consistência entre os pares em execução.
5) Reinicie os pares parados e verifique a recuperação e a igualdade entre pares.

## Uso em CI

- Build: `cargo build --workspace`
- Pré-construir amostras IVM (conforme necessário pelos testes)
- Teste: `cargo test --workspace`
- Lint mais rigoroso (opcional): `cargo clippy --workspace --all-targets -- -D warnings`

## Conclusão

Este framework utiliza o harness Rust do repositório (`iroha_test_network`) e o cliente HTTP (`iroha::client::Client`) para validar uma rede Iroha v2 de 7 pares. Ele enfatiza consistência entre pares, cenários de falha realistas e configuração/teardown reproduzíveis adequados para CI. Docker Compose via `iroha_swarm` está disponível quando a conteinerização é preferível.

## Explorer

- Qualquer explorador de blockchain que fale Torii HTTP/WebSocket pode se conectar a cada par de forma independente. Cada par expõe um endpoint Torii (host:porta) adequado para status, blocos, consultas e eventos.
- Com o harness Rust: após construir uma rede você pode derivar as URLs de todos os pares:

  ```rust
  use iroha_test_network::NetworkBuilder;

  let network = NetworkBuilder::new().with_peers(7).build();
  let urls = network.torii_urls();
  // ex. ["http://127.0.0.1:8080", ..., "http://127.0.0.1:8086"]
  ```

  Helpers por par também estão disponíveis: `peer.api_address()` e `peer.torii_url()`.

- Com Docker Compose (`iroha_swarm`): gere um compose de 7 pares e mapeie `8080..8086` para o host; aponte seu explorador para cada um desses endereços. Se seu explorador suportar vários endpoints, configure todos os 7; caso contrário, execute uma instância por par.
