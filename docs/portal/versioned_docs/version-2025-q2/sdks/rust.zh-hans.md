---
lang: zh-hans
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T14:35:36.896251+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK 快速入门

Rust 客户端 API 位于 `iroha` 箱中，它公开了 `client::Client`
用于与 Torii 对话的类型。当您需要提交交易时使用它，
订阅事件，或从 Rust 应用程序查询状态。

## 1. 添加箱子

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

工作区示例通过 `client` 功能解锁客户端模块。如果你
使用已发布的板条箱，将 `path` 属性替换为当前的
版本字符串。

## 2.配置客户端

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

`ClientConfiguration` 镜像 CLI 配置文件：它包括 Torii 和
遥测 URL、身份验证材料、超时和批处理首选项。

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

在后台，客户端使用 Norito 之前对交易有效负载进行编码
将其发布到 Torii。如果提交成功，返回的hash可以用来
通过 `client.poll_transaction_status(hash)` 跟踪状态。

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

当您需要检查或保留 Norito 有效负载而不将其发送到
Torii，调用`client.build_da_ingest_request(...)`获取签名请求
并将其渲染为 JSON/字节，镜像 `iroha app da submit --no-submit`。

## 5.查询数据

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

查询遵循请求/响应模式：构造查询类型
`iroha_data_model::query`，通过 `client.request` 发送，并迭代
结果。响应使用 Norito 支持的 JSON，因此传输格式是确定的。

## 6. 订阅事件

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

客户端公开 Torii 的 SSE 端点的异步流，包括管道
事件、数据事件和遥测源。

## 更多示例

- 端到端流位于 `tests/` 下的 `crates/iroha` 中。搜索集成
  测试如`transaction_submission.rs`，场景更丰富。
- CLI (`iroha_cli`) 使用相同的客户端模块；浏览
  `crates/iroha_cli/src/` 查看身份验证、批处理和重试的情况
  在生产工具中处理。
- 记住 Norito：客户端永远不会回退到 `serde_json`。当你
  扩展 SDK，依赖 `norito::json` 帮助程序来获取 JSON 端点和
  `norito::codec` 用于二进制有效负载。

使用这些构建块，您可以将 Torii 集成到 Rust 服务或 CLI 中。
请参阅生成的文档和数据模型包以获取全套内容
指令、查询和事件。