---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`docs/source/nexus_sdk_quickstarts.md` の情報は完了しました。ポータル デスタカ OS の前提条件を確認し、SDK パラメタの OS コマンドを迅速に設定検証します。

## コンパルティリハーダを構成する

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus の設定を行い、SDK の依存関係としてインストールし、OS 証明書の TLS 対応リリース (veja `docs/source/sora_nexus_operator_onboarding.md`) を確認します。

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

O スクリプト インスタンス `ToriiClient` は、最近の環境や世界のさまざまな環境に対応しています。

## スウィフト

```bash
make swift-nexus-demo
```

米国 `Torii.Client` は `IrohaSwift` パラバスカー `FindNetworkStatus` を実行します。

## アンドロイド

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

エンドポイントのステージング Nexus を実行します。

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## 問題の解決策

- Falhas TLS -> バンドル CA が tarball をリリースし、Nexus を確認します。
- `ERR_UNKNOWN_LANE` -> passe `--lane-id`/`--dataspace-id` Quando o Roteamento インポスト用マルチレーン。
- `ERR_SETTLEMENT_PAUSED` -> 検証 [Nexus 操作](../nexus/nexus-operations) インシデント処理パラメタ。ガバナンカ ポデ テル パウサド ア レーン。

SDK veja `docs/source/nexus_sdk_quickstarts.md` のコンテキストと説明が表示されます。