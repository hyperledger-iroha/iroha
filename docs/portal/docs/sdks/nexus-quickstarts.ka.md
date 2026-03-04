---
lang: ka
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

სრული სწრაფი დაწყება მოქმედებს `docs/source/nexus_sdk_quickstarts.md`-ზე. ეს პორტალი
რეზიუმე ხაზს უსვამს დეველოპერების საერთო წინაპირობებს და თითო SDK ბრძანებებს
შეუძლია მათი დაყენების სწრაფად გადამოწმება.

## გაზიარებული დაყენება

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

ჩამოტვირთეთ Nexus კონფიგურაციის ნაკრები, დააინსტალირეთ თითოეული SDK-ის დამოკიდებულებები და დარწმუნდით
TLS სერთიფიკატები ემთხვევა გამოშვების პროფილს (იხ
`docs/source/sora_nexus_operator_onboarding.md`).

## ჟანგი

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

ნომრები: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

სკრიპტი ახდენს `ToriiClient` ინსტანციას ზემოთ env vars-ით და ბეჭდავს
უახლესი ბლოკი.

## სვიფტი

```bash
make swift-nexus-demo
```

იყენებს `Torii.Client`-დან `IrohaSwift`-დან `FindNetworkStatus`-ის მისაღებად.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

აწარმოებს მართული მოწყობილობის ტესტს Nexus დადგმის საბოლოო წერტილზე.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## პრობლემების მოგვარება

- TLS წარუმატებლობა → დაადასტურეთ CA პაკეტი Nexus გამოშვების ტარბოლიდან.
- `ERR_UNKNOWN_LANE` → გაიარეთ `--lane-id`/`--dataspace-id` ერთხელ მრავალ ზოლიანი მარშრუტით
  აღსრულებულია.
- `ERR_SETTLEMENT_PAUSED` → შეამოწმეთ [Nexus ოპერაციები](../nexus/nexus-operations)
  ინციდენტის პროცესი; მმართველობამ შესაძლოა შეაჩერა ჩიხი.

უფრო ღრმა კონტექსტისა და SDK-ს სპეციფიკური ახსნა-განმარტებისთვის იხ
`docs/source/nexus_sdk_quickstarts.md`.