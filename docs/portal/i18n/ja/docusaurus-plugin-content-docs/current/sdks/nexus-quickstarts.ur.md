---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

クイックスタート `docs/source/nexus_sdk_quickstarts.md` موجود ہے۔ SDK の開発と開発وویلپرز اپنی سیٹ اپ جلدی جانچ سکیں۔

## شترکہ سیٹ اپ

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus 開発中のソフトウェア SDK 開発ソフトウェア SDK 開発ソフトウェアیقینی بنائیں کہ TLS سرٹیفکیٹس ریلیز پروفائل سے میل کھاتے ہیں (دیکھیے `docs/source/sora_nexus_operator_onboarding.md`)。

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

اسکرپٹ اوپر والی ماحولیات متغیرات کے ساتھ `ToriiClient` بناتا ہے اور تازہ ترین بلاک پرنٹありがとうございます

## スウィフト

```bash
make swift-nexus-demo
```

`IrohaSwift` ٩ے `Torii.Client` سے `FindNetworkStatus` حاصل کرتا ہے۔

## アンドロイド

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

ステージング エンドポイント ステージング エンドポイント ステージング エンドポイント ステージング エンドポイント

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## すごい

- TLS ناکامیاں -> Nexus ریلیز tarball سے CA بنڈل کی توثیق کریں۔
- `ERR_UNKNOWN_LANE` -> マルチレーンルーティング `--lane-id`/`--dataspace-id` دیں۔
- `ERR_SETTLEMENT_PAUSED` -> واقعہ عمل کے لیے [Nexus 操作](../nexus/nexus-operations) دیکھیں؛ ã‚¹ã‚¤ã‚¤ã‚¹ã‚¹ã‚¿

مزید سیاق و سباق اور SDK مخصوص وضاحتوں کے لیے `docs/source/nexus_sdk_quickstarts.md` دیکھیں۔