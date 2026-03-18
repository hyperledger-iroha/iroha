---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2151bc561df9599865e2a5aa5d159171c1aa6f4830bfa51bd32d726f0c70a6f
source_last_modified: "2025-11-11T10:23:01.761192+00:00"
translation_last_reviewed: 2026-01-30
---

# Quickstart do SDK Rust

A API cliente em Rust vive no crate `iroha`, que expõe o tipo `client::Client` para conversar com Torii. Use-o quando precisar enviar transações, assinar eventos ou consultar estado a partir de uma aplicação Rust.

## 1. Adicione o crate

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

O exemplo de workspace libera o módulo cliente via a feature `client`. Se você consumir o crate publicado, substitua o atributo `path` pela versão atual.

## 2. Configure o cliente

```rust title="src/main.rs"
use iroha::client::{Client, ClientConfiguration};

fn main() -> eyre::Result<()> {
 let cfg = ClientConfiguration {
 torii_url: "http://127.0.0.1:8080".parse()?,
 telemetry_url: Some("http://127.0.0.1:8080".parse()?),
 // account_id, key_pair and other options can be populated here or via helper builders
 ..ClientConfiguration::default()
 };

 let client = Client::new(cfg)?;
 println!("Node status: {:?}", client.get_status()?);
 Ok(())
}
```

`ClientConfiguration` espelha o arquivo de configuração da CLI: inclui URLs de Torii e telemetria, material de autenticação, timeouts e preferências de batching.

## 3. Envie uma transação

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::{
 isi::prelude::*,
 prelude::{AccountId, ChainId, DomainId, Name},
};
use iroha_crypto::{KeyPair, PublicKey};

fn submit_example() -> eyre::Result<()> {
 let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
 let account_id = AccountId::new(
 Name::from_str("alice")?,
 DomainId::from_str("wonderland")?,
 );

 let key_pair = KeyPair::generate_ed25519(); // replace with a persistent key in real apps

 let cfg = ClientConfiguration {
 chain: chain_id.clone(),
 account: account_id.clone(),
 key_pair: key_pair.clone(),
 ..ClientConfiguration::test()
 };

 let client = Client::new(cfg)?;

 let instruction = Register {
 object: Domain::new(Name::from_str("research")?, None),
 };

 let tx = client.build_transaction([instruction]);
 let signed = tx.sign(&key_pair)?;
 let hash = client.submit_transaction(&signed)?;
 println!("Submitted transaction: {hash}");
 Ok(())
}
```

Por baixo dos panos o cliente usa Norito para codificar o payload antes de enviar para Torii. Se o envio for bem-sucedido, o hash retornado pode ser usado para acompanhar o status via `client.poll_transaction_status(hash)`.

## 4. Envie blobs DA

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha::da::DaIngestParams;
use iroha_data_model::{da::types::ExtraMetadata, nexus::LaneId};

fn submit_da_blob() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let mut params = DaIngestParams::default();
 params.lane_id = LaneId::new(7);
 params.epoch = 42;
 let payload = std::fs::read("payload.car")?;
 let metadata = ExtraMetadata::default();
 let result = client.submit_da_blob(payload, &params, metadata, None)?;
 println!(
 "status={} duplicate={} bytes={}",
 result.status, result.duplicate, result.payload_len
 );
 Ok(())
}
```

Quando precisar inspecionar ou persistir o payload Norito sem enviar a Torii, chame `client.build_da_ingest_request(...)` para obter a requisição assinada e renderizá-la como JSON/bytes, espelhando `iroha app da submit --no-submit`.

## 5. Consulte dados

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::query::prelude::*;

fn list_domains() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let response = client.request(&FindAllDomains::new())?;
 for domain in response {
 println!("{}", domain.name());
 }
 Ok(())
}
```

As consultas seguem o padrão request/response: construa um tipo de consulta a partir de `iroha_data_model::query`, envie via `client.request` e itere sobre os resultados. As respostas usam JSON apoiado por Norito, então o formato no wire é determinístico.

## 6. Snapshots QR do Explorer

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "i105...",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` espelha o JSON `/v1/explorer/accounts/{id}/qr`: inclui o account id canônico, o literal canônico I105, metadados de prefixo/correção de erro, dimensões do QR e o payload SVG inline.

## 7. Assine eventos

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::events::pipeline::PipelineEventFilterBox;
use futures_lite::stream::StreamExt;

async fn listen_for_blocks() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let mut stream = client
 .listen_for_events([PipelineEventFilterBox::any()])
 .await?;

 while let Some(event) = stream.next().await {
 println!("Received event: {:?}", event?);
 }
 Ok(())
}
```

O cliente expõe streams async para os endpoints SSE de Torii, incluindo eventos de pipeline, eventos de dados e feeds de telemetria.

## Mais exemplos

- Fluxos end-to-end vivem em `tests/` em `crates/iroha`. Procure testes de integração como `transaction_submission.rs` para cenários mais ricos.
- A CLI (`iroha_cli`) usa o mesmo módulo cliente; veja `crates/iroha_cli/src/` para entender autenticação, batching e retries em tooling de produção.
- Tenha Norito em mente: o cliente nunca recorre a `serde_json`. Ao estender o SDK, use helpers `norito::json` para endpoints JSON e `norito::codec` para payloads binários.

## Exemplos Norito relacionados

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — compila, executa e implanta o scaffold mínimo de Kotodama que espelha a fase de setup deste quickstart.
- [Register domain and mint assets](../norito/examples/register-and-mint) — alinha-se com o fluxo `Register` + `Mint` mostrado acima para repetir as mesmas operações a partir de um contrato.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — demonstra o syscall `transfer_asset` com as mesmas IDs de conta usadas pelos quickstarts do SDK.

Com esses blocos, você pode integrar Torii em serviços ou CLIs em Rust. Consulte a documentação gerada e os crates de data model para o conjunto completo de instruções, consultas e eventos.
