<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: pt
direction: ltr
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

# Guia rapido do SDK Nexus

**Link do roadmap:** NX-14 - documentacao de Nexus e runbooks de operadores  
**Status:** Rascunho 2026-03-24  
**Audiencia:** Times de SDK e desenvolvedores parceiros conectando-se a rede Sora Nexus (Iroha 3).

Este guia captura os passos minimos necessarios para apontar cada SDK first-party para a rede
publica Nexus. Ele complementa os manuais detalhados de SDK em `docs/source/sdk/*` e o runbook de
operadores (`docs/source/nexus_operations.md`).

## Prerequisitos compartilhados

- Instale as versoes de toolchain fixadas em `rust-toolchain.toml` e `package.json`/`Package.swift`
  para o SDK que voce esta testando.
- Baixe o bundle de configuracao do Nexus (veja `docs/source/sora_nexus_operator_onboarding.md`)
  para obter os endpoints de Torii, certificados TLS e o catalogo de lanes de settlement.
- Exporte as seguintes variaveis de ambiente (ajuste para o seu ambiente):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

Esses valores alimentam os blocos de configuracao especificos do SDK abaixo.

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

- Build/tests: `cargo test -p iroha_client -- --nocapture`
- Script de demo: `cargo run --bin nexus_quickstart --features sdk`
- Docs: `docs/source/sdk/rust.md`

### Helper de disponibilidade de dados (DA-8)

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

### Helper de QR do Explorer (ADDR-6b)

```rust
use iroha_client::client::{
    Client,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "<i105-account-id>",
    )?;
    println!("i105 literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

O `ExplorerAccountQrSnapshot` retornado inclui o id de conta canonico, o literal solicitado, as
configuracoes de correcao de erros e o payload SVG inline usado pelos fluxos de compartilhamento
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

- Instalar deps: `npm ci`
- Executar exemplo: `NEXUS_TORII_URL=... npm run demo:nexus`
- Docs: `docs/source/sdk/js/quickstart.md` (renomeacao pendente para alinhar com este doc)

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
- App de demo: `examples/android/retail-wallet` (atualize `.env` com valores Nexus)
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

O comando envia uma consulta `FindNetworkStatus` e imprime o resultado. Para caminhos de escrita,
passe argumentos `--lane-id`/`--dataspace-id` que correspondam ao catalogo de lanes.

## Matriz de testes

| Superficie | Comando | Esperado |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | Atinge o endpoint de staging do Nexus, imprime a altura do bloco. |
| JS/TS | `npm run test:nexus` | Teste Jest que garante que as URLs do Torii + pipeline funcionam. |
| Swift | `swift test --filter NexusQuickstartTests` | Simulador iOS busca o status. |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | Dispositivo gerenciado acessa staging. |
| CLI | `iroha_cli app nexus quickstart --dry-run` | Valida a config antes de enviar chamadas de rede. |

## Solucao de problemas

- **Erros TLS/CA** - verifique o bundle de CA enviado com o release Nexus e confirme que `toriiUrl`
  usa HTTPS.
- **`ERR_SETTLEMENT_PAUSED`** - pausa de governanca em nivel de lane; ver
  `docs/source/nexus_operations.md` para o processo de incidente.
- **`ERR_UNKNOWN_LANE`** - atualize seu SDK para passar `lane_id`/`dataspace_id` explicitos quando a
  admissao multi-lane estiver aplicada.

Reporte bugs via issues do GitHub com a tag `NX-14` e mencione o SDK + comando usado para que os
mantenedores possam reproduzir rapidamente.
