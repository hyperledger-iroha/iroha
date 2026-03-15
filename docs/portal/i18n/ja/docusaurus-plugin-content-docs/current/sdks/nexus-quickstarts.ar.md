---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`docs/source/nexus_sdk_quickstarts.md`。 SDK を使用して、SDK を使用する必要があります。ありがとうございます。

## ああ、

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus، وثبت تبعيات كل SDK، وتاكد من تطابق شهادات TLS مع ملف और देखें

## 錆びる

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

番号: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

`ToriiClient` を確認してください。

## スウィフト

```bash
make swift-nexus-demo
```

`Torii.Client`、`IrohaSwift`、`FindNetworkStatus`。

## アンドロイド

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

ステージングは最高です。

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## ستكشاف الاخطاء واصلاحها

- TLS -> テスト CA テスト tarball テスト Nexus。
- `ERR_UNKNOWN_LANE` -> عندما يتم فرض التوجيه متعدد المسارات.
- `ERR_SETTLEMENT_PAUSED` -> راجع [Nexus 操作](../nexus/nexus-operations) قد تكون الحوكمة اوقفت المسار.

SDK `docs/source/nexus_sdk_quickstarts.md` を参照してください。