<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ja
direction: ltr
source: docs/source/nexus_sdk_quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3a080e17f3a1ce9ded139921a5dffe84bfe7a136b7c7804272cc22d53a683d0
source_last_modified: "2025-11-19T15:43:49.064234+00:00"
translation_last_reviewed: 2026-01-01
---

# Nexus SDK クイックスタート

**ロードマップリンク:** NX-14 - Nexus ドキュメントとオペレーター向けランブック  
**ステータス:** 2026-03-24 草案  
**対象:** Sora Nexus (Iroha 3) ネットワークに接続する SDK チームとパートナー開発者。

このガイドは、各ファーストパーティ SDK を Nexus の公開ネットワークに向けるために
必要な最小手順をまとめます。`docs/source/sdk/*` の詳細 SDK マニュアルと、運用
ランブック (`docs/source/nexus_operations.md`) を補完します。

## 共通の前提条件

- テストする SDK に合わせて、`rust-toolchain.toml` と `package.json`/`Package.swift` に
  固定されているツールチェーンのバージョンをインストールします。
- Nexus の設定バンドルを取得します (`docs/source/sora_nexus_operator_onboarding.md` を参照)。
  Torii エンドポイント、TLS 証明書、決済レーンのカタログを入手します。
- 次の環境変数をエクスポートします (環境に合わせて調整):

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<hex-peer-public-key>"
```

これらの値は下記の SDK 固有の設定ブロックに利用されます。

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

- ビルド/テスト: `cargo test -p iroha_client -- --nocapture`
- デモスクリプト: `cargo run --bin nexus_quickstart --features sdk`
- ドキュメント: `docs/source/sdk/rust.md`

### データ可用性ヘルパー (DA-8)

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

- ドキュメント: `docs/source/sdk/rust.md`

### Explorer QR ヘルパー (ADDR-6b)

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

返される `ExplorerAccountQrSnapshot` には、正規のアカウント ID、要求されたリテラル、
誤り訂正設定、ウォレット/エクスプローラ共有フローで使われるインライン SVG ペイロードが
含まれます。

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

- 依存関係のインストール: `npm ci`
- サンプル実行: `NEXUS_TORII_URL=... npm run demo:nexus`
- ドキュメント: `docs/source/sdk/js/quickstart.md` (このドキュメントに合わせてリネーム予定)

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

- ビルド/テスト: `swift test`
- デモハーネス: `make swift-nexus-demo`
- ドキュメント: `docs/source/sdk/swift/index.md`

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

- ビルド/テスト: `./gradlew :iroha-android:connectedDebugAndroidTest`
- デモアプリ: `examples/android/retail-wallet` (`.env` に Nexus 値を更新)
- ドキュメント: `docs/source/sdk/android/index.md`

## CLI (`iroha_cli`)

```bash
iroha_cli nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}" \
  --trusted-peer "${NEXUS_TRUSTED_PUBKEY}" \
  --key "${HOME}/.iroha/keys/nexus_ed25519.key"
```

このコマンドは `FindNetworkStatus` クエリを送信して結果を表示します。書き込みパスでは、
レーンカタログに一致する `--lane-id`/`--dataspace-id` 引数を渡します。

## テストマトリクス

| 対象 | コマンド | 期待結果 |
|---------|---------|----------|
| Rust | `cargo test -p iroha_client -- --ignored nexus_quickstart` | Nexus のステージングエンドポイントにアクセスし、ブロック高を表示する。 |
| JS/TS | `npm run test:nexus` | Torii と pipeline の URL が動作することを確認する Jest テスト。 |
| Swift | `swift test --filter NexusQuickstartTests` | iOS シミュレータがステータスを取得する。 |
| Android | `./gradlew :iroha-android:nexusQuickstartTest` | 管理対象デバイスがステージングにアクセスする。 |
| CLI | `iroha_cli nexus quickstart --dry-run` | ネットワーク呼び出し前に設定を検証する。 |

## トラブルシューティング

- **TLS/CA エラー** - Nexus リリースに同梱された CA バンドルを確認し、`toriiUrl` が HTTPS を
  使っていることを確認してください。
- **`ERR_SETTLEMENT_PAUSED`** - レーン単位のガバナンス停止; インシデント手順は
  `docs/source/nexus_operations.md` を確認してください。
- **`ERR_UNKNOWN_LANE`** - マルチレーン admission が強制される場合、明示的に
  `lane_id`/`dataspace_id` を渡すよう SDK を更新してください。

GitHub issues で `NX-14` タグを付けて報告し、使用した SDK とコマンドを記載してください。
