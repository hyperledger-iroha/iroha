<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ru
direction: ltr
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

# Быстрый старт SDK Nexus

**Ссылка на дорожную карту:** NX-14 — документация Nexus и ранбуки операторов  
**Статус:** Черновик 2026-03-24  
**Аудитория:** команды SDK и партнерские разработчики, подключающиеся к сети Sora Nexus (Iroha 3).

Этот гид фиксирует минимальные шаги, чтобы направить каждый first-party SDK на публичную сеть
Nexus. Он дополняет подробные руководства SDK в `docs/source/sdk/*` и операторский ранбук
(`docs/source/nexus_operations.md`).

## Общие предпосылки

- Установите версии toolchain, закрепленные в `rust-toolchain.toml` и `package.json`/`Package.swift`
  для тестируемого SDK.
- Скачайте конфигурационный пакет Nexus (см. `docs/source/sora_nexus_operator_onboarding.md`),
  чтобы получить эндпоинты Torii, TLS-сертификаты и каталог settlement lane.
- Экспортируйте следующие переменные окружения (адаптируйте под свою среду):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

Эти значения используются в блоках конфигурации SDK ниже.

## SDK на Rust (`iroha_client`)

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

- Сборка/тесты: `cargo test -p iroha_client -- --nocapture`
- Демо-скрипт: `cargo run --bin nexus_quickstart --features sdk`
- Документация: `docs/source/sdk/rust.md`

### Хелпер доступности данных (DA-8)

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

- Документация: `docs/source/sdk/rust.md`

### Хелпер QR для Explorer (ADDR-6b)

```rust
use iroha_client::client::{
    Client,
};

fn share_wallet_qr(client: &Client) -> eyre::Result<()> {
    let snapshot = client.get_explorer_account_qr(
        "soraカタカナ...",
    )?;
    println!("i105 literal: {}", snapshot.literal);
    std::fs::write("alice_qr.svg", snapshot.svg)?;
    Ok(())
}
```

Возвращаемый `ExplorerAccountQrSnapshot` включает канонический id аккаунта, запрошенный literal,
настройки коррекции ошибок и inline SVG payload, используемый в потоках шаринга wallet/explorer.

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

- Установка зависимостей: `npm ci`
- Запуск примера: `NEXUS_TORII_URL=... npm run demo:nexus`
- Документация: `docs/source/sdk/js/quickstart.md` (переименование ожидается для согласования с этим доком)

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

- Сборка/тесты: `swift test`
- Демо-харнесс: `make swift-nexus-demo`
- Документация: `docs/source/sdk/swift/index.md`

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

- Сборка/тесты: `./gradlew :iroha-android:connectedDebugAndroidTest`
- Демо-приложение: `examples/android/retail-wallet` (обновите `.env` значениями Nexus)
- Документация: `docs/source/sdk/android/index.md`

## CLI (`iroha_cli`)

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}" \
  --trusted-peer "${NEXUS_TRUSTED_PUBKEY}" \
  --key "${HOME}/.iroha/keys/nexus_ed25519.key"
```

Команда отправляет запрос `FindNetworkStatus` и печатает результат. Для путей записи передайте
аргументы `--lane-id`/`--dataspace-id`, соответствующие каталогу lanes.

## Матрица тестирования

| Компонент | Команда | Ожидаемо |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | Вызывает endpoint staging Nexus и печатает высоту блока. |
| JS/TS | `npm run test:nexus` | Jest-тест, подтверждающий, что URL Torii и pipeline работают. |
| Swift | `swift test --filter NexusQuickstartTests` | iOS симулятор получает статус. |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | Управляемое устройство обращается к staging. |
| CLI | `iroha_cli app nexus quickstart --dry-run` | Проверяет конфигурацию перед сетевыми вызовами. |

## Устранение неполадок

- **Ошибки TLS/CA** — проверьте CA bundle, поставляемый с релизом Nexus, и убедитесь, что `toriiUrl`
  использует HTTPS.
- **`ERR_SETTLEMENT_PAUSED`** — пауза управления на уровне lane; см.
  `docs/source/nexus_operations.md` для процесса инцидента.
- **`ERR_UNKNOWN_LANE`** — обновите SDK, чтобы явно передавать `lane_id`/`dataspace_id`, когда
  admission multi-lane будет введен.

Сообщайте о багах через GitHub issues с меткой `NX-14` и указывайте SDK и команду, чтобы
сопровождающие могли быстро воспроизвести.
