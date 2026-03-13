---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`docs/source/nexus_sdk_quickstarts.md` のガイドは完全に設定されています。ポータルの再開は、SDK の開発前に必要な前提条件を確認し、迅速な設定を確認するためのコマンドを実行します。

## 設定コミューン

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

構成 Nexus のパケットを充電し、SDK の依存関係をインストールし、TLS 対応プロファイルのリリース (`docs/source/sora_nexus_operator_onboarding.md`) の証明書を保証します。

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

スクリプト インスタンス `ToriiClient` 環境変数の環境変数と詳細ブロックの添付ファイル。

## スウィフト

```bash
make swift-nexus-demo
```

`Torii.Client` および `IrohaSwift` 注入回収器 `FindNetworkStatus` を利用します。

## アンドロイド

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

ステージング終了点 Nexus のテストを実行します。

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## パンナージを解除する

- Echecs TLS -> リリース Nexus の検証ファイル バンドル CA デュ タールボール。
- `ERR_UNKNOWN_LANE` -> 通行人 `--lane-id`/`--dataspace-id` une fois le Routage マルチレーン アップリケ。
- `ERR_SETTLEMENT_PAUSED` -> コンサルタ [Nexus 操作](../nexus/nexus-operations) プロセスのインシデントが発生しました。 la gouvernance a peut etre miss la LANE EN PAUSE。

SDK によるコンテキストと説明を追加します。`docs/source/nexus_sdk_quickstarts.md` を参照してください。