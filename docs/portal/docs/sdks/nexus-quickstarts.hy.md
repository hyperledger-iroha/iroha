---
lang: hy
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

Ամբողջական արագ մեկնարկը գործում է `docs/source/nexus_sdk_quickstarts.md`-ում: Այս պորտալը
ամփոփումը ընդգծում է ընդհանուր նախադրյալները և յուրաքանչյուր SDK հրամանները, որպեսզի մշակողները
կարող է արագ ստուգել դրանց կարգավորումը:

## Համօգտագործվող կարգավորում

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Ներբեռնեք Nexus կազմաձևման փաթեթը, տեղադրեք յուրաքանչյուր SDK-ի կախվածությունը և ապահովեք
TLS վկայագրերը համապատասխանում են թողարկման պրոֆիլին (տես
`docs/source/sora_nexus_operator_onboarding.md`):

## Ժանգ

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Հղումներ՝ `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

Սցենարը ներկայացնում է `ToriiClient`-ը վերևում գտնվող env vars-ով և տպում է
վերջին բլոկը.

## Սվիֆթ

```bash
make swift-nexus-demo
```

Օգտագործում է `Torii.Client` `IrohaSwift`-ից՝ `FindNetworkStatus` բերելու համար:

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Գործարկում է կառավարվող սարքի թեստը՝ հարվածելով Nexus փուլային վերջնակետին:

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Անսարքությունների վերացում

- TLS ձախողումներ → հաստատեք CA փաթեթը Nexus թողարկման tarball-ից:
- `ERR_UNKNOWN_LANE` → անցեք `--lane-id`/`--dataspace-id` մեկ անգամ բազմագոտի երթուղի
  հարկադրված է։
- `ERR_SETTLEMENT_PAUSED` → ստուգեք [Nexus գործողություններ] (../nexus/nexus-operations)
  միջադեպի գործընթաց; կառավարումը կարող է դադարեցրել երթուղին:

Ավելի խորը համատեքստի և SDK-ին հատուկ բացատրությունների համար տե՛ս
`docs/source/nexus_sdk_quickstarts.md`.