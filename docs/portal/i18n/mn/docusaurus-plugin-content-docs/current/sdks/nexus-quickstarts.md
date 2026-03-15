---
id: nexus-quickstarts
lang: mn
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus SDK quickstarts
description: Minimal steps for Rust/JS/Swift/Android/CLI SDKs to connect to Sora Nexus.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Бүрэн хурдан эхлүүлэх нь `docs/source/nexus_sdk_quickstarts.md` дээр амьдардаг. Энэ портал
хураангуй хуваалцсан урьдчилсан нөхцөл болон нэг SDK тушаалуудыг онцлон хөгжүүлэгчид
тохиргоог хурдан шалгаж болно.

## Хуваалцсан тохиргоо

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus тохиргооны багцыг татаж аваад SDK тус бүрийн хамаарлыг суулгаж, баталгаажуулна уу.
TLS гэрчилгээ нь хувилбарын профайлтай таарч байна (харна уу
`docs/source/sora_nexus_operator_onboarding.md`).

## Зэв

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

Скрипт нь `ToriiClient`-г дээрх env vars-уудаар үүсгэн хэвлэнэ.
хамгийн сүүлийн блок.

## Хурдан

```bash
make swift-nexus-demo
```

`FindNetworkStatus`-г татахын тулд `IrohaSwift`-аас `Torii.Client`-г ашигладаг.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexus шатлалын төгсгөлийн цэгийг давах удирдлагатай төхөөрөмжийн тестийг ажиллуулдаг.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Алдааг олж засварлах

- TLS алдаа → Nexus хувилбараас CA багцыг баталгаажуулна уу.
- `ERR_UNKNOWN_LANE` → `--lane-id`/`--dataspace-id` нэг удаа олон эгнээний чиглүүлэлт хийх
  хэрэгждэг.
- `ERR_SETTLEMENT_PAUSED` → [Nexus үйлдлийн](../nexus/nexus-operations)-г шалгана уу.
  ослын үйл явц; засаглал энэ эгнээг түр зогсоосон байж магадгүй.

Илүү гүнзгий контекст болон SDK-д зориулсан тайлбарыг үзнэ үү
`docs/source/nexus_sdk_quickstarts.md`.