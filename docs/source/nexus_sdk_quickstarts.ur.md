<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ur
direction: rtl
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_sdk_quickstarts.md -->

# Nexus SDK فوری آغاز

**روڈ میپ لنک:** NX-14 - Nexus دستاویزات اور آپریٹر runbooks  
**اسٹیٹس:** مسودہ 2026-03-24  
**سامعین:** SDK ٹیمیں اور پارٹنر ڈویلپرز جو Sora Nexus (Iroha 3) نیٹ ورک سے جڑتے ہیں۔

یہ گائیڈ ہر first-party SDK کو Nexus پبلک نیٹ ورک کی طرف پوائنٹ کرنے کے لئے کم سے کم اقدامات
بیان کرتا ہے۔ یہ `docs/source/sdk/*` کے تفصیلی SDK مینولز اور آپریٹر runbook
(`docs/source/nexus_operations.md`) کی تکمیل کرتا ہے۔

## مشترکہ پیشگی تقاضے

- جس SDK کی جانچ کر رہے ہیں اس کے لئے `rust-toolchain.toml` اور `package.json`/`Package.swift` میں
  pinned toolchain ورژنز انسٹال کریں۔
- Nexus config bundle ڈاؤن لوڈ کریں (`docs/source/sora_nexus_operator_onboarding.md` دیکھیں) تاکہ
  Torii endpoints، TLS certificates اور settlement lane catalog حاصل ہوں۔
- درج ذیل environment variables ایکسپورٹ کریں (اپنے ماحول کے مطابق ایڈجسٹ کریں):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

یہ قدریں نیچے دیے گئے SDK مخصوص configuration blocks میں استعمال ہوتی ہیں۔

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

- بلڈ/ٹیسٹ: `cargo test -p iroha_client -- --nocapture`
- ڈیمو اسکرپٹ: `cargo run --bin nexus_quickstart --features sdk`
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
    AddressFormat, Client, ExplorerAccountQrOptions,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "ih58...",
        Some(ExplorerAccountQrOptions {
            address_format: Some(AddressFormat::Compressed),
        }),
    )?;
    println!("IH58 (preferred)/snx1 (second-best) literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

واپس آنے والا `ExplorerAccountQrSnapshot` canonical account id، requested literal، error-correction
settings، اور wallet/explorer share flows میں استعمال ہونے والا inline SVG payload شامل کرتا ہے۔

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

- deps انسٹال کریں: `npm ci`
- مثال چلائیں: `NEXUS_TORII_URL=... npm run demo:nexus`
- Docs: `docs/source/sdk/js/quickstart.md` (اس doc کے مطابق rename متوقع ہے)

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

- بلڈ/ٹیسٹ: `swift test`
- ڈیمو ہارنس: `make swift-nexus-demo`
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

- بلڈ/ٹیسٹ: `./gradlew :iroha-android:connectedDebugAndroidTest`
- ڈیمو ایپ: `examples/android/retail-wallet` (`.env` میں Nexus ویلیوز اپ ڈیٹ کریں)
- Docs: `docs/source/sdk/android/index.md`

## CLI (`iroha_cli`)

```bash
iroha_cli nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}" \
  --trusted-peer "${NEXUS_TRUSTED_PUBKEY}" \
  --key "${HOME}/.iroha/keys/nexus_ed25519.key"
```

یہ کمانڈ `FindNetworkStatus` کوئری سبمٹ کرتی ہے اور نتیجہ پرنٹ کرتی ہے۔ لکھنے والے راستوں کے
لئے `--lane-id`/`--dataspace-id` آرگومینٹس پاس کریں جو lane catalog سے میچ کرتے ہوں۔

## ٹیسٹنگ میٹرکس

| سطح | کمانڈ | متوقع |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | Nexus staging endpoint کو ہٹ کرتا ہے اور بلاک ہائٹ پرنٹ کرتا ہے۔ |
| JS/TS | `npm run test:nexus` | Jest ٹیسٹ جو Torii اور pipeline URLs کے کام کرنے کی تصدیق کرتا ہے۔ |
| Swift | `swift test --filter NexusQuickstartTests` | iOS سمیولیٹر اسٹیٹس حاصل کرتا ہے۔ |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | مینیجڈ ڈیوائس staging تک رسائی حاصل کرتا ہے۔ |
| CLI | `iroha_cli nexus quickstart --dry-run` | نیٹ ورک کالز سے پہلے config ویلیڈیٹ کرتا ہے۔ |

## ٹربل شوٹنگ

- **TLS/CA errors** - Nexus ریلیز کے ساتھ دی گئی CA bundle کی تصدیق کریں اور یقینی بنائیں کہ
  `toriiUrl` HTTPS استعمال کر رہا ہے۔
- **`ERR_SETTLEMENT_PAUSED`** - lane-level governance pause; incident process کے لئے
  `docs/source/nexus_operations.md` دیکھیں۔
- **`ERR_UNKNOWN_LANE`** - جب multi-lane admission نافذ ہو تو SDK اپ ڈیٹ کریں تاکہ `lane_id`/
  `dataspace_id` واضح طور پر پاس ہوں۔

GitHub issues میں `NX-14` ٹَیگ کے ساتھ bug رپورٹ کریں اور استعمال کیا گیا SDK اور کمانڈ ضرور لکھیں
تاکہ maintainers جلدی reproduce کر سکیں۔

</div>
