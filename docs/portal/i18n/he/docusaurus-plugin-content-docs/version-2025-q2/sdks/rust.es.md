---
lang: es
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e208999275d0b445ca40c10e21de3339cc8e2e6fa1a08592496a128c1392a19
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 035600f179f4dd225778fae57c927b2a6c9a0f1c45ca949e3536b99283c2dde3
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

# Rust SDK Quickstart

The Rust client API lives in the `iroha` crate, which exposes a `client::Client`
type for talking to Torii. Use it when you need to submit transactions,
subscribe to events, or query state from a Rust application.

## 1. Add the crate

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

The workspace example unlocks the client module via the `client` feature. If you
consume the published crate, replace the `path` attribute with the current
version string.

## 2. Configure the client

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

`ClientConfiguration` mirrors the CLI configuration file: it includes Torii and
telemetry URLs, authentication material, timeouts, and batching preferences.

## 3. Submit a transaction

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

Under the hood the client uses Norito to encode the transaction payload before
posting it to Torii. If submission succeeds, the returned hash can be used to
track status via `client.poll_transaction_status(hash)`.

## 4. Submit DA blobs

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

When you need to inspect or persist the Norito payload without sending it to
Torii, call `client.build_da_ingest_request(...)` to obtain the signed request
and render it as JSON/bytes, mirroring `iroha app da submit --no-submit`.

## 5. Query data

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

Queries follow the request/response pattern: construct a query type from
`iroha_data_model::query`, send it via `client.request`, and iterate over the
results. Responses use Norito-backed JSON, so the wire format is deterministic.

## 6. Explorer QR snapshots

```rust
use iroha::client::{
    AddressFormat, Client, ClientConfiguration, ExplorerAccountQrOptions,
};

fn download_qr() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let snapshot = client.get_explorer_account_qr(
        "ih58...",
        Some(ExplorerAccountQrOptions {
            address_format: Some(AddressFormat::Compressed),
        }),
    )?;
    println!("Canonical literal: {}", snapshot.literal);
    println!("SVG payload: {}", snapshot.svg);
    Ok(())
}
```

`ExplorerAccountQrSnapshot` mirrors the `/v1/explorer/accounts/{id}/qr` JSON
surface: it includes the canonical account id, the literal rendered with the
requested format, network prefix/error-correction metadata, QR dimensions, and
the inline SVG payload that wallets/explorers can embed directly. Omit
`ExplorerAccountQrOptions` to default to the preferred IH58 output or set
`address_format: Some(AddressFormat::Compressed)` to retrieve the second-best
`sora…` variant used by ADDR-6b.

## 7. Subscribe to events

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

The client exposes async streams for Torii’s SSE endpoints, including pipeline
events, data events, and telemetry feeds.

## More examples

- End-to-end flows live under `tests/` in `crates/iroha`. Search for integration
  tests such as `transaction_submission.rs` for richer scenarios.
- The CLI (`iroha_cli`) uses the same client module; browse
  `crates/iroha_cli/src/` to see how authentication, batching, and retries are
  handled in production tooling.
- Keep Norito in mind: the client never falls back to `serde_json`. When you
  extend the SDK, rely on `norito::json` helpers for JSON endpoints and
  `norito::codec` for binary payloads.

## Related Norito examples

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — compile, run, and deploy
  the minimal Kotodama scaffold that mirrors the setup phase in this quickstart.
- [Register domain and mint assets](../norito/examples/register-and-mint) — aligns with the
  `Register` + `Mint` flow shown above so you can replay the same operations from a contract.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — demonstrates the
  `transfer_asset` syscall with the same account IDs the SDK quickstarts use.

With these building blocks you can integrate Torii into Rust services or CLIs.
Refer to the generated documentation and data-model crates for the full set of
instructions, queries, and events.
