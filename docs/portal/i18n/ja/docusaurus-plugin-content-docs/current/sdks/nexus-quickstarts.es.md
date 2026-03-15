---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

La guia completa esta en `docs/source/nexus_sdk_quickstarts.md`。ポータルの再起動により、コンパートメントの前提条件やコマンド、SDK の設定を迅速に検証できます。

## 構成のコンパルティダ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus の構成をダウンロードし、依存関係のある SDK をインストールし、TLS の証明書のリリースを確認します (バージョン `docs/source/sora_nexus_operator_onboarding.md`)。

## 錆びる

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

スクリプトインスタンス `ToriiClient` は、主要な究極ブロックの変数を含みます。

## スウィフト

```bash
make swift-nexus-demo
```

米国 `Torii.Client` と `IrohaSwift` は `FindNetworkStatus` を取得します。

## アンドロイド

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexus のステージングのエンドポイントを管理するための処理を実行します。

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## 問題の解決策

- Fallas TLS -> Nexus のリリースの tarball バンドル CA を確認します。
- `ERR_UNKNOWN_LANE` -> pasa `--lane-id`/`--dataspace-id` cuando el enrutamiento multi-lane sea obligatorio。
- `ERR_SETTLEMENT_PAUSED` -> [Nexus 操作](../nexus/nexus-operations) 事故処理手順の改訂。ラ・ゴベルナンザ・プド・パウサール・ラ・レーン。

SDK に関するコンテキストの説明については、`docs/source/nexus_sdk_quickstarts.md` を参照してください。