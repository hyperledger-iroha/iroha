---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 035600f179f4dd225778fae57c927b2a6c9a0f1c45ca949e3536b99283c2dde3
source_last_modified: "2026-01-28T17:11:30.697433+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK 快速入門

Rust 客戶端 API 位於 `iroha` 箱中，它公開了 `client::Client`
用於與 Torii 對話的類型。當您需要提交交易時使用它，
訂閱事件，或從 Rust 應用程序查詢狀態。

## 1. 添加箱子

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

工作區示例通過 `client` 功能解鎖客戶端模塊。如果你
使用已發布的板條箱，將 `path` 屬性替換為當前的
版本字符串。

## 2.配置客戶端

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

`ClientConfiguration` 鏡像 CLI 配置文件：它包括 Torii 和
遙測 URL、身份驗證材料、超時和批處理首選項。

## 3.提交交易

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

在後台，客戶端使用 Norito 之前對交易有效負載進行編碼
將其發佈到 Torii。如果提交成功，返回的hash可以用來
通過 `client.poll_transaction_status(hash)` 跟踪狀態。

## 4. 提交 DA blob

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

當您需要檢查或保留 Norito 有效負載而不將其發送到
Torii，調用`client.build_da_ingest_request(...)`獲取簽名請求
並將其渲染為 JSON/字節，鏡像 `iroha app da submit --no-submit`。

## 5.查詢數據

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

查詢遵循請求/響應模式：構造查詢類型
`iroha_data_model::query`，通過 `client.request` 發送，並迭代
結果。響應使用 Norito 支持的 JSON，因此傳輸格式是確定的。

## 6. 資源管理器二維碼快照

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

`ExplorerAccountQrSnapshot` 鏡像 `/v2/explorer/accounts/{id}/qr` JSON
表面：它包括規範的帳戶ID，用
規範 I105 字面量、網絡前綴/糾錯元數據、QR 尺寸以及
錢包/瀏覽器可以直接嵌入的內聯 SVG 有效負載。

## 7. 訂閱事件

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

客戶端公開 Torii 的 SSE 端點的異步流，包括管道
事件、數據事件和遙測源。

## 更多示例

- 端到端流在 `crates/iroha` 中的 `tests/` 下運行。搜索集成
 測試如`transaction_submission.rs`，場景更豐富。
- CLI (`iroha_cli`) 使用相同的客戶端模塊；瀏覽
 `crates/iroha_cli/src/` 查看身份驗證、批處理和重試的情況
 在生產工具中處理。
- 記住 Norito：客戶端永遠不會回退到 `serde_json`。當你
 擴展 SDK，依賴 `norito::json` JSON 端點幫助程序
 `norito::codec` 用於二進制有效負載。

## 相關 Norito 示例

- [Hajimari 入口點框架](../norito/examples/hajimari-entrypoint) — 編譯、運行和部署
 最小的 Kotodama 腳手架，反映了本快速入門中的設置階段。
- [註冊域名和鑄造資產](../norito/examples/register-and-mint) — 與
 上面顯示了 `Register` + `Mint` 流程，以便您可以從合約重播相同的操作。
- [賬戶之間轉移資產](../norito/examples/transfer-asset) — 演示
 `transfer_asset` 系統調用與 SDK 快速入門使用的帳戶 ID 相同。

使用這些構建塊，您可以將 Torii 集成到 Rust 服務或 CLI 中。
請參閱生成的文檔和數據模型包以獲取全套內容
指令、查詢和事件。