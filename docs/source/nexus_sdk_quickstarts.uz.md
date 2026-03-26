---
lang: uz
direction: ltr
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06e588194ae61f27626b9ab13e56546d4a3084f5033a256701622208d47745a8
source_last_modified: "2026-01-28T17:11:30.696801+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus SDK Quickstarts

**Roadmap link:** NX-14 — Nexus documentation & operator runbooks  
**Status:** Drafted 2026-03-24  
**Audience:** SDK teams and partner developers connecting to the Sora Nexus
(Iroha 3) network.

This guide captures the minimum steps required to point each first-party SDK at
the Nexus public network. It complements the detailed SDK manuals under
`docs/source/sdk/*` and the operator runbook (`docs/source/nexus_operations.md`).

## Shared prerequisites

- Install the toolchain versions pinned in `rust-toolchain.toml` and
  `package.json`/`Package.swift` for the SDK you are testing.
- Download the Nexus config bundle (see `docs/source/sora_nexus_operator_onboarding.md`)
  to obtain the Torii endpoints, TLS certificates, and settlement lane catalog.
- Export the following environment variables (adjust for your environment):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

These values feed into the SDK-specific configuration blocks below.

## Rust SDK (`iroha_client`)

```rust
use iroha_client::client::{Client, ClientConfiguration};
use iroha_core::crypto::KeyPair;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keypair = KeyPair::from_hex("ed0120...")?;
    let config = ClientConfiguration::with_url(NEXUS_TORII_URL.parse()?)
        .with_chain_id(NEXUS_CHAIN_ID.parse()?)
        .with_pipeline_url(NEXUS_PIPELINE_URL.parse()?)
        .with_tls("nexus_ca.pem")
        .with_trusted_peers(vec![NEXUS_TRUSTED_PUBKEY.parse()?]);
    let client = Client::new(config, keypair)?;
    let status = client.get_network_status()?;
    println!("Latest block: {}", status.latest_block_height);
    Ok(())
}
```

- Build/test: `cargo test -p iroha_client -- --nocapture`
- Demo script: `cargo run --bin nexus_quickstart --features sdk`
- Docs: `docs/source/sdk/rust.md`

### Data availability helper (DA-8)

```rust
use iroha_client::client::{Client, ClientConfiguration};
use iroha_client::da::DaIngestParams;
use iroha_data_model::da::types::ExtraMetadata;
use eyre::Result;

fn persist_da_payload(client: &Client, payload: Vec<u8>, storage_ticket: &str) -> Result<()> {
    client
        .submit_da_blob(payload, &DaIngestParams::default(), ExtraMetadata::default(), None)?;
    let persisted = client.fetch_da_manifest_to_dir(storage_ticket, "artifacts/da")?;
    println!(
        "manifest saved to {} / chunk plan {}",
        persisted.manifest_raw.display(),
        persisted.chunk_plan.display()
    );
    Ok(())
}
```

- Docs: `docs/source/sdk/rust.md`

### Explorer QR helper (ADDR-6b)

```rust
use iroha_client::client::{
    Client,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "<katakana-i105-account-id>",
    )?;
    println!("i105 literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

The returned `ExplorerAccountQrSnapshot` includes the canonical Katakana i105 account id,
canonical Katakana i105 literal, error-correction settings, and the inline SVG payload used by
wallet/explorer share flows.

## JavaScript / TypeScript (`@iroha/iroha-js`)

```ts
import { ToriiClient } from '@iroha/iroha-js';

const client = new ToriiClient({
  baseUrl: process.env.NEXUS_TORII_URL!,
  pipelineUrl: process.env.NEXUS_PIPELINE_URL!,
  chainId: process.env.NEXUS_CHAIN_ID!,
  trustedPeers: [process.env.NEXUS_TRUSTED_PUBKEY!],
});

(async () => {
  const status = await client.fetchStatus();
  console.log(`Latest block: ${status.latestBlock.height}`);
})();
```

- Install deps: `npm ci`
- Run sample: `NEXUS_TORII_URL=... npm run demo:nexus`
- Docs: `docs/source/sdk/js/quickstart.md` (pending rename to align with this doc)

## Swift (`IrohaSwift`)

```swift
import IrohaSwift

let config = Torii.Configuration(
    toriiURL: URL(string: ProcessInfo.processInfo.environment["NEXUS_TORII_URL"]!)!,
    pipelineURL: URL(string: ProcessInfo.processInfo.environment["NEXUS_PIPELINE_URL"]!)!,
    chainId: ProcessInfo.processInfo.environment["NEXUS_CHAIN_ID"]!,
    trustedPeers: [ProcessInfo.processInfo.environment["NEXUS_TRUSTED_PUBKEY"]!],
    tls: .cafile(path: "nexus_ca.pem")
)
let client = try Torii.Client(configuration: config, keyPair: KeyPair.randomEd25519())
let status = try client.getNetworkStatus().get()
print("Latest block \(status.latestBlock.height)")
```

- Build/tests: `swift test`
- Demo harness: `make swift-nexus-demo`
- Docs: `docs/source/sdk/swift/index.md`

## Android (`iroha-android`)

```kotlin
val config = ClientConfig(
    toriiUrl = NEXUS_TORII_URL,
    pipelineUrl = NEXUS_PIPELINE_URL,
    chainId = NEXUS_CHAIN_ID,
    trustedPeers = listOf(NEXUS_TRUSTED_PUBKEY),
    tls = ClientConfig.Tls.CertFile("nexus_ca.pem")
)
val client = NoritoRpcClient(config, defaultDispatcher)
runBlocking {
    val status = client.fetchStatus()
    println("Latest block ${status.latestBlock.height}")
}
```

- Build/tests: `./gradlew :iroha-android:connectedDebugAndroidTest`
- Demo app: `examples/android/retail-wallet` (update `.env` with Nexus values)
- Docs: `docs/source/sdk/android/index.md`

## CLI (`iroha_cli`)

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}" \
  --trusted-peer "${NEXUS_TRUSTED_PUBKEY}" \
  --key "${HOME}/.iroha/keys/nexus_ed25519.key"
```

The command submits a `FindNetworkStatus` query and prints the result. For write
paths, pass `--lane-id`/`--dataspace-id` arguments matching the lane catalog.

## Testing matrix

| Surface | Command | Expected |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | Hits Nexus staging endpoint, prints block height. |
| JS/TS | `npm run test:nexus` | Jest test ensuring Torii + pipeline URLs work. |
| Swift | `swift test --filter NexusQuickstartTests` | iOS simulator fetches status. |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | Managed device hits staging. |
| CLI | `iroha_cli app nexus quickstart --dry-run` | Validates config before sending network calls. |

## Troubleshooting

- **TLS/CA errors** — verify the CA bundle shipped with the Nexus release and
  confirm the `toriiUrl` uses HTTPS.
- **`ERR_SETTLEMENT_PAUSED`** — lane-level governance pause; check
  `docs/source/nexus_operations.md` for the incident process.
- **`ERR_UNKNOWN_LANE`** — update your SDK to pass explicit `lane_id`/`dataspace_id`
  once multi-lane admission is enforced.

Report bugs via GitHub issues tagged `NX-14` and mention the SDK + command used
so maintainers can reproduce quickly.
