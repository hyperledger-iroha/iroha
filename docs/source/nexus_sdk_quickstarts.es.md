<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: es
direction: ltr
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

# Guia rapida del SDK de Nexus

**Enlace del roadmap:** NX-14 - documentacion de Nexus y runbooks de operadores  
**Estado:** Borrador 2026-03-24  
**Audiencia:** Equipos de SDK y desarrolladores socios que se conectan a la red Sora Nexus (Iroha 3).

Esta guia captura los pasos minimos requeridos para apuntar cada SDK de primera parte a la red
publica de Nexus. Complementa los manuales detallados de SDK bajo `docs/source/sdk/*` y el runbook
de operadores (`docs/source/nexus_operations.md`).

## Prerequisitos compartidos

- Instala las versiones de toolchain fijadas en `rust-toolchain.toml` y `package.json`/`Package.swift`
  para el SDK que estes probando.
- Descarga el bundle de configuracion de Nexus (ver `docs/source/sora_nexus_operator_onboarding.md`)
  para obtener los endpoints de Torii, certificados TLS y el catalogo de lanes de settlement.
- Exporta las siguientes variables de entorno (ajusta para tu entorno):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

Estos valores alimentan los bloques de configuracion especificos del SDK mas abajo.

## SDK de Rust (`iroha_client`)

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

- Compilar/pruebas: `cargo test -p iroha_client -- --nocapture`
- Script de demo: `cargo run --bin nexus_quickstart --features sdk`
- Docs: `docs/source/sdk/rust.md`

### Helper de disponibilidad de datos (DA-8)

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

### Helper de QR del explorador (ADDR-6b)

```rust
use iroha_client::client::{
    AddressFormat, Client, ExplorerAccountQrOptions,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "alice@wonderland",
        Some(ExplorerAccountQrOptions {
            address_format: Some(AddressFormat::Compressed),
        }),
    )?;
    println!("IH58 (preferred)/snx1 (second-best) literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

El `ExplorerAccountQrSnapshot` devuelto incluye el id de cuenta canonico, el literal solicitado,
los ajustes de correccion de errores y el payload SVG inline usado por los flujos de comparticion
de wallet/explorer.

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

- Instalar deps: `npm ci`
- Ejecutar ejemplo: `NEXUS_TORII_URL=... npm run demo:nexus`
- Docs: `docs/source/sdk/js/quickstart.md` (pendiente de renombre para alinearse con este doc)

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

- Compilar/pruebas: `swift test`
- Harness de demo: `make swift-nexus-demo`
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

- Compilar/pruebas: `./gradlew :iroha-android:connectedDebugAndroidTest`
- App de demo: `examples/android/retail-wallet` (actualiza `.env` con valores de Nexus)
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

El comando envia una consulta `FindNetworkStatus` y imprime el resultado. Para rutas de escritura,
pasa argumentos `--lane-id`/`--dataspace-id` que coincidan con el catalogo de lanes.

## Matriz de pruebas

| Superficie | Comando | Esperado |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | Golpea el endpoint de staging de Nexus, imprime la altura del bloque. |
| JS/TS | `npm run test:nexus` | Test de Jest que asegura que las URLs de Torii + pipeline funcionan. |
| Swift | `swift test --filter NexusQuickstartTests` | El simulador iOS obtiene el estado. |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | Dispositivo administrado accede a staging. |
| CLI | `iroha_cli nexus quickstart --dry-run` | Valida la config antes de enviar llamadas de red. |

## Resolucion de problemas

- **Errores TLS/CA** - verifica el bundle de CA enviado con el release de Nexus y confirma que
  `toriiUrl` usa HTTPS.
- **`ERR_SETTLEMENT_PAUSED`** - pausa de gobernanza a nivel de lane; revisa
  `docs/source/nexus_operations.md` para el proceso de incidente.
- **`ERR_UNKNOWN_LANE`** - actualiza tu SDK para pasar `lane_id`/`dataspace_id` explicitos cuando la
  admision multi-lane este aplicada.

Reporta bugs via issues de GitHub con etiqueta `NX-14` y menciona el SDK + comando usado para que
los mantenedores puedan reproducir rapido.
