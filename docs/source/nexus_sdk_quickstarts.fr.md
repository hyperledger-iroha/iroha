<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: fr
direction: ltr
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

# Guide de demarrage rapide des SDK Nexus

**Lien roadmap :** NX-14 - documentation Nexus et runbooks operateurs  
**Statut :** Brouillon 2026-03-24  
**Audience :** Equipes SDK et developpeurs partenaires se connectant au reseau Sora Nexus (Iroha 3).

Ce guide capture les etapes minimales requises pour pointer chaque SDK first-party vers le reseau
public Nexus. Il complete les manuels SDK detailles sous `docs/source/sdk/*` et le runbook operateur
(`docs/source/nexus_operations.md`).

## Prerequis partages

- Installez les versions de toolchain epinglees dans `rust-toolchain.toml` et
  `package.json`/`Package.swift` pour le SDK que vous testez.
- Telechargez le bundle de config Nexus (voir `docs/source/sora_nexus_operator_onboarding.md`) pour
  obtenir les endpoints Torii, certificats TLS et le catalogue de lanes de settlement.
- Exportez les variables d'environnement suivantes (adaptez a votre environnement) :

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

Ces valeurs alimentent les blocs de configuration specifiques aux SDK ci-dessous.

## SDK Rust (`iroha_client`)

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

- Compilation/tests: `cargo test -p iroha_client -- --nocapture`
- Script de demo: `cargo run --bin nexus_quickstart --features sdk`
- Docs: `docs/source/sdk/rust.md`

### Helper de disponibilite des donnees (DA-8)

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

### Helper QR Explorer (ADDR-6b)

```rust
use iroha_client::client::{
    Client,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "i105...",
    )?;
    println!("i105 literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

Le `ExplorerAccountQrSnapshot` renvoye inclut l'id de compte canonique, le literal demande, les
reglages de correction d'erreurs et la charge utile SVG inline utilisee par les flux de partage
wallet/explorer.

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

- Installer deps: `npm ci`
- Lancer l'exemple: `NEXUS_TORII_URL=... npm run demo:nexus`
- Docs: `docs/source/sdk/js/quickstart.md` (renommage prevu pour s'aligner avec ce doc)

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

- Compilation/tests: `swift test`
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

- Compilation/tests: `./gradlew :iroha-android:connectedDebugAndroidTest`
- Application demo: `examples/android/retail-wallet` (mettre a jour `.env` avec les valeurs Nexus)
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

La commande soumet une requete `FindNetworkStatus` et imprime le resultat. Pour les chemins
d'ecriture, passez les arguments `--lane-id`/`--dataspace-id` correspondant au catalogue de lanes.

## Matrice de tests

| Surface | Commande | Attendu |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | Atteint l'endpoint de staging Nexus, imprime la hauteur de bloc. |
| JS/TS | `npm run test:nexus` | Test Jest qui verifie que les URLs Torii + pipeline fonctionnent. |
| Swift | `swift test --filter NexusQuickstartTests` | Le simulateur iOS recupere l'etat. |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | Appareil gere atteint staging. |
| CLI | `iroha_cli app nexus quickstart --dry-run` | Valide la config avant d'envoyer des appels reseau. |

## Depannage

- **Erreurs TLS/CA** - verifier le bundle CA livre avec le release Nexus et confirmer que `toriiUrl`
  utilise HTTPS.
- **`ERR_SETTLEMENT_PAUSED`** - pause de gouvernance au niveau lane; voir
  `docs/source/nexus_operations.md` pour le processus d'incident.
- **`ERR_UNKNOWN_LANE`** - mettre a jour votre SDK pour passer explicitement `lane_id`/`dataspace_id`
  une fois l'admission multi-lane appliquee.

Signalez les bugs via des issues GitHub etiquetees `NX-14` et mentionnez le SDK + la commande
utilisee pour que les maintainers puissent reproduire rapidement.
