---
lang: kk
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

Толық жылдам іске қосу `docs/source/nexus_sdk_quickstarts.md` мекенжайында өмір сүреді. Бұл портал
жиынтық ортақ алғышарттарды және әр SDK пәрмендерін әзірлеушілер үшін бөлектейді
орнатуды жылдам тексере алады.

## Ортақ орнату

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus конфигурация жинағын жүктеп алыңыз, әрбір SDK тәуелділіктерін орнатыңыз және қамтамасыз етіңіз.
TLS сертификаттары шығарылым профиліне сәйкес келеді (қараңыз
`docs/source/sora_nexus_operator_onboarding.md`).

## Тот

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Анықтамалар: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

Сценарий `ToriiClient` нұсқасын жоғарыдағы env нұсқаларымен жасайды және басып шығарады
соңғы блок.

## Жылдам

```bash
make swift-nexus-demo
```

`FindNetworkStatus` алу үшін `IrohaSwift` бастап `Torii.Client` пайдаланады.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexus кезеңінің соңғы нүктесіне сәйкес басқарылатын құрылғы сынағын іске қосады.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Ақаулықтарды жою

- TLS қателері → Nexus шығарылым тарболынан CA бумасын растаңыз.
- `ERR_UNKNOWN_LANE` → `--lane-id`/`--dataspace-id` бір рет көп жолақты маршрутизациядан өту
  орындалады.
- `ERR_SETTLEMENT_PAUSED` → [Nexus операциялары](../nexus/nexus-operations) үшін тексеріңіз
  оқиға процесі; басқару жолақты уақытша тоқтатқан болуы мүмкін.

Тереңірек мәтінмән және SDK-арнайы түсініктемелер үшін қараңыз
`docs/source/nexus_sdk_quickstarts.md`.