---
lang: am
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

ሙሉ ፈጣን ጅምር በ `docs/source/nexus_sdk_quickstarts.md` ይኖራል። ይህ ፖርታል
ማጠቃለያ የጋራ ቅድመ-ሁኔታዎችን እና በየኤስዲኬ ገንቢዎችን ያደምቃል
አወቃቀራቸውን በፍጥነት ማረጋገጥ ይችላል።

## የተጋራ ማዋቀር

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

የNexus ውቅር ቅርቅቡን ያውርዱ፣ እያንዳንዱን የኤስዲኬ ጥገኛ ይጫኑ እና ያረጋግጡ
የቲኤልኤስ የምስክር ወረቀቶች ከመልቀቂያው መገለጫ ጋር ይዛመዳሉ (ይመልከቱ
`docs/source/sora_nexus_operator_onboarding.md`).

## ዝገት።

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

ማጣቀሻዎች: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

ስክሪፕቱ `ToriiClient` ከላይ ካለው env vars ጋር ያፋጥናል እና ያትማል።
የቅርብ ጊዜ እገዳ.

## ፈጣን

```bash
make swift-nexus-demo
```

`FindNetworkStatus` ለማምጣት `Torii.Client` ከ `IrohaSwift` ይጠቀማል።

## አንድሮይድ

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

የሚተዳደረውን መሳሪያ ሙከራ የI18NT0000001X የዝግጅት መጨረሻ ነጥብን በመምታት ያካሂዳል።

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## መላ መፈለግ

- TLS አለመሳካቶች → የCA ጥቅልን ከI18NT0000002X ልቀት ታርቦል ያረጋግጡ።
- `ERR_UNKNOWN_LANE` → ማለፍ `--lane-id`/I18NI0000021X አንዴ ባለ ብዙ መስመር ማዘዋወር
  ተፈጻሚ ነው።
- `ERR_SETTLEMENT_PAUSED` → ያረጋግጡ [Nexus ክወናዎች](../nexus/nexus-operations)
  የአደጋ ሂደት; አስተዳደር መስመሩን ባለበት አቁሞ ሊሆን ይችላል።

ጠለቅ ያለ አውድ እና ኤስዲኬ-ተኮር ማብራሪያዎችን ይመልከቱ
`docs/source/nexus_sdk_quickstarts.md`.