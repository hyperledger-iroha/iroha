---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08d231445c0eb56985d360594393a1fd0fec06b53fdcf8defbe0b2439191ee2f
source_last_modified: "2026-01-22T14:45:01.264878+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-quickstarts
title: Nexus SDK quickstarts
description: Minimal steps for Rust/JS/Swift/Android/CLI SDKs to connect to Sora Nexus.
translator: machine-google-reviewed
---

完整的快速入門位於 `docs/source/nexus_sdk_quickstarts.md`。這個門戶
摘要強調了共享的先決條件和每個 SDK 命令，以便開發人員
可以快速驗證他們的設置。

## 共享設置

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

下載 Nexus 配置包，安裝每個 SDK 的依賴項，並確保
TLS 證書與發布配置文件匹配（請參閱
`docs/source/sora_nexus_operator_onboarding.md`）。

## 鐵鏽

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

參考號：`docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

該腳本使用上面的環境變量實例化 `ToriiClient` 並打印
最新塊。

## 斯威夫特

```bash
make swift-nexus-demo
```

使用 `Torii.Client` 從 `IrohaSwift` 獲取 `FindNetworkStatus`。

## 安卓

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

運行命中 Nexus 暫存端點的託管設備測試。

## 命令行界面

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## 故障排除

- TLS 失敗 → 確認 Nexus 版本 tarball 中的 CA 捆綁包。
- `ERR_UNKNOWN_LANE` → 通過 `--lane-id`/`--dataspace-id` 一次多通道路由
  被強制執行。
- `ERR_SETTLEMENT_PAUSED` → 檢查 [Nexus 操作](../nexus/nexus-operations)
  事件過程；治理可能已經暫停了車道。

有關更深入的上下文和特定於 SDK 的說明，請參閱
`docs/source/nexus_sdk_quickstarts.md`。