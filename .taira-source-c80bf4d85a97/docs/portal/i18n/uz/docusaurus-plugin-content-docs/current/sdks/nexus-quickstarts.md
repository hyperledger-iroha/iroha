---
id: nexus-quickstarts
lang: uz
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus SDK quickstarts
description: Minimal steps for Rust/JS/Swift/Android/CLI SDKs to connect to Sora Nexus.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

To'liq tezkor ishga tushirish `docs/source/nexus_sdk_quickstarts.md` da ishlaydi. Ushbu portal
Xulosa ishlab chiquvchilar uchun umumiy shartlar va har bir SDK buyruqlarini ta'kidlaydi
ularning sozlamalarini tezda tekshirishi mumkin.

## Umumiy sozlash

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus konfiguratsiya to'plamini yuklab oling, har bir SDK bog'liqligini o'rnating va ishonch hosil qiling
TLS sertifikatlari nashr profiliga mos keladi (qarang
`docs/source/sora_nexus_operator_onboarding.md`).

## Zang

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Refs: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

Skript yuqoridagi env varslari bilan `ToriiClient` ni yaratadi va chop etadi
oxirgi blok.

## Tezkor

```bash
make swift-nexus-demo
```

`FindNetworkStatus` ni olish uchun `IrohaSwift` dan `Torii.Client` dan foydalanadi.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexus bosqichma-bosqich yakuniy nuqtasiga tegib, boshqariladigan qurilma testini ishga tushiradi.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Nosozliklarni bartaraf etish

- TLS nosozliklari → Nexus reliz tarballidan CA to'plamini tasdiqlang.
- `ERR_UNKNOWN_LANE` → `--lane-id`/`--dataspace-id` bir marta ko'p qatorli marshrutga o'ting
  amalga oshiriladi.
- `ERR_SETTLEMENT_PAUSED` → [Nexus operatsiyalarini](../nexus/nexus-operations) tekshiring
  hodisa jarayoni; boshqaruv yo'lni to'xtatib qo'ygan bo'lishi mumkin.

Chuqurroq kontekst va SDK-ga xos tushuntirishlar uchun qarang
`docs/source/nexus_sdk_quickstarts.md`.