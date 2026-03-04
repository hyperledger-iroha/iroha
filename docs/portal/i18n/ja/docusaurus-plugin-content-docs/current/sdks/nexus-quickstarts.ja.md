---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sdks/nexus-quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eeea9edc91ca4b2183f0361b2db9bed545ce427be5860ec04584362ca08b47b9
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

完全なクイックスタートは `docs/source/nexus_sdk_quickstarts.md` にあります。このポータルの要約は、共通の前提条件とSDK別のコマンドを示し、開発者が設定を素早く確認できるようにします。

## 共通セットアップ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexusの設定バンドルをダウンロードし、各SDKの依存関係をインストールし、TLS証明書がリリースプロファイルと一致していることを確認してください（`docs/source/sora_nexus_operator_onboarding.md` を参照）。

## Rust

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

参照: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

このスクリプトは上記の環境変数で `ToriiClient` を初期化し、最新のブロックを出力します。

## Swift

```bash
make swift-nexus-demo
```

`IrohaSwift` の `Torii.Client` を使って `FindNetworkStatus` を取得します。

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexusのステージングエンドポイントに向けたマネージドデバイステストを実行します。

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## トラブルシューティング

- TLSの失敗 -> Nexusリリースのtarballに含まれるCAバンドルを確認。
- `ERR_UNKNOWN_LANE` -> マルチレーンルーティングが強制になったら `--lane-id`/`--dataspace-id` を指定。
- `ERR_SETTLEMENT_PAUSED` -> [Nexus operations](../nexus/nexus-operations) のインシデント手順を確認。ガバナンスがレーンを停止している可能性があります。

詳細な背景とSDK別の説明は `docs/source/nexus_sdk_quickstarts.md` を参照してください。
