---
lang: zh-hans
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

完整的快速入门位于 `docs/source/nexus_sdk_quickstarts.md`。这个门户
摘要强调了共享的先决条件和每个 SDK 命令，以便开发人员
可以快速验证他们的设置。

## 共享设置

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

下载 Nexus 配置包，安装每个 SDK 的依赖项，并确保
TLS 证书与发布配置文件匹配（请参阅
`docs/source/sora_nexus_operator_onboarding.md`）。

## 铁锈

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

参考号：`docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

该脚本使用上面的环境变量实例化 `ToriiClient` 并打印
最新块。

## 斯威夫特

```bash
make swift-nexus-demo
```

使用 `Torii.Client` 从 `IrohaSwift` 获取 `FindNetworkStatus`。

## 安卓

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

运行命中 Nexus 暂存端点的托管设备测试。

## 命令行界面

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## 故障排除

- TLS 失败 → 确认 Nexus 版本 tarball 中的 CA 捆绑包。
- `ERR_UNKNOWN_LANE` → 通过 `--lane-id`/`--dataspace-id` 一次多通道路由
  被强制执行。
- `ERR_SETTLEMENT_PAUSED` → 检查 [Nexus 操作](../nexus/nexus-operations)
  事件过程；治理可能已经暂停了车道。

有关更深入的上下文和特定于 SDK 的说明，请参阅
`docs/source/nexus_sdk_quickstarts.md`。