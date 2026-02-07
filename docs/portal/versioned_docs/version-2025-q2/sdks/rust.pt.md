---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T15:38:30.655816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Início rápido do Rust SDK

A API do cliente Rust reside na caixa `iroha`, que expõe um `client::Client`
digite para falar com Torii. Use-o quando precisar enviar transações,
assinar eventos ou consultar o estado de um aplicativo Rust.

## 1. Adicione a caixa

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

O exemplo do espaço de trabalho desbloqueia o módulo cliente por meio do recurso `client`. Se você
consumir a caixa publicada, substitua o atributo `path` pelo atual
sequência de versão.

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

`ClientConfiguration` espelha o arquivo de configuração CLI: inclui Torii e
URLs de telemetria, material de autenticação, tempos limite e preferências de lote.

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

Nos bastidores, o cliente usa Norito para codificar a carga útil da transação antes
publicando-o em Torii. Se o envio for bem-sucedido, o hash retornado poderá ser usado para
rastrear status via `client.poll_transaction_status(hash)`.

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

Quando você precisar inspecionar ou persistir a carga útil Norito sem enviá-la para
Torii, ligue para `client.build_da_ingest_request(...)` para obter a solicitação assinada
e renderizá-lo como JSON/bytes, espelhando `iroha app da submit --no-submit`.

## 5. Consultar dados

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

As consultas seguem o padrão de solicitação/resposta: construa um tipo de consulta a partir de
`iroha_data_model::query`, envie-o via `client.request` e itere sobre o
resultados. As respostas usam JSON baseado em Norito, portanto, o formato de ligação é determinístico.

## 6. Inscreva-se em eventos

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

O cliente expõe fluxos assíncronos para endpoints SSE do Torii, incluindo pipeline
eventos, eventos de dados e feeds de telemetria.

## Mais exemplos

- Os fluxos ponta a ponta vivem em `tests/` em `crates/iroha`. Busca por integração
  testes como `transaction_submission.rs` para cenários mais ricos.
- A CLI (`iroha_cli`) utiliza o mesmo módulo cliente; navegar
  `crates/iroha_cli/src/` para ver como são a autenticação, o lote e as novas tentativas
  manipulados em ferramentas de produção.
- Lembre-se de Norito: o cliente nunca volta para `serde_json`. Quando você
  estender o SDK, contar com auxiliares `norito::json` para endpoints JSON e
  `norito::codec` para cargas binárias.

Com esses blocos de construção você pode integrar Torii em serviços Rust ou CLIs.
Consulte a documentação gerada e as caixas do modelo de dados para obter o conjunto completo de
instruções, consultas e eventos.