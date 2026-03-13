---
id: nexus-quickstarts
lang: ba
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus SDK quickstarts
description: Minimal steps for Rust/JS/Swift/Android/CLI SDKs to connect to Sora Nexus.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Тулы тиҙ старт йәшәй I18NI000000012X. Был портал
дөйөм айырып күрһәтә дөйөм алшарттар һәм пер-СДК командалары шулай эшләүселәр .
уларҙы тиҙ генә раҫлай ала.

## Дөйөм ҡоролма

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Скачать I18NT000000000000000000 конфигурация өйөм, ҡуйырға һәр SDK’s бәйлелек, һәм тәьмин итеү .
TLS сертификаттары тура килә релиз профиле (ҡара
`docs/source/sora_nexus_operator_onboarding.md`).

## Тут

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Һылтанмалар: `docs/source/sdk/rust.md`

## JavaScript / тип Скрипт

```bash
npm run demo:nexus
```

Сценарий I18NI000000015X инстанциялары менән өҫтәге env vars һәм баҫтырып сығара
һуңғы блок.

## Свифт

```bash
make swift-nexus-demo
```

Ҡулланыу I18NI0000000016X `IrohaSwift` тиклем `FindNetworkStatus` алыу.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
``` X

Йүгереп идара итеү-ҡоролма һынау I18NT000000001X стадияһы ос нөктәһенә тейҙе.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Төҙөкләндереүҙең

- TLS етешһеҙлектәре → раҫлау CA өйөмө I18NT000000002X татарбол сығарыу.
- I18NI000000019X → тапшырыу I18NI000000020X/I18NI000000021X бер тапҡыр күп һыҙатлы маршрутлаштырыу
  үтәлгән.
- I18NI000000022X → чек [I18NT000000003X операциялары] (../nexus/nexus-operations) өсөн
  инцидент процесы; идара итеү һыҙатты туҡтатҡандыр.

Тәрән контекст һәм SDK-махсус аңлатмалар өсөн ҡарағыҙ
`docs/source/nexus_sdk_quickstarts.md`.