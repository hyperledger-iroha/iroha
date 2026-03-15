---
lang: dz
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

མགྱོགས་མྱུར་འགོ་བཙུགས་འདི་ `docs/source/nexus_sdk_quickstarts.md` ལུ་སྡོད་དོ་ཡོདཔ་ཨིན། དྲ་རྒྱ་འདི་
བཅུད་བསྡུས་ཚུ་ བགོ་བཤའ་རྐྱབ་ཡོད་པའི་སྔོན་སྒྲིག་དང་ ཨེསི་ཌི་ཀེ་རེ་ལུ་ བརྡ་བཀོད་ཚུ་ དེ་འབདཝ་ལས་ གོང་འཕེལ་གཏང་མི་ཚུ་ཨིན།
ཁོང་གི་གཞི་སྒྲིག་འདི་མགྱོགས་པ་རང་བདེན་དཔྱད་འབད་ཚུགས།

## བརྗེ་སོར་འབད་ཡོད་པའི་གཞི་སྒྲིག་།

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus རིམ་སྒྲིག་བཱན་ཌལ་འདི་ཕབ་ལེན་འབད་ཞིནམ་ལས་ ཨེསི་ཌི་ཀེ་རེ་རེ་གི་བརྟེན་ས་ཚུ་གཞི་བཙུགས་འབད་ཞིནམ་ལས་ ངེས་གཏན་བཟོ།
ཊི་ཨེལ་ཨེསི་ལག་ཁྱེར་ཚུ་གིས་ གསར་བཏོན་གསལ་སྡུད་དང་མཐུན་སྒྲིག་འབད་ (se)
`docs/source/sora_nexus_operator_onboarding.md`).

## རསཊ།

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

དཔྱད་གཞི།: `docs/source/sdk/rust.md`

## ཇ་བ་སིཀིརིཔ / དབྱེ་བ་ཡིག་དཔར་བ།

```bash
npm run demo:nexus
```

ཡིག་ཚུགས་འདི་གིས་ `ToriiClient` འདི་ གོང་ལུ་ env vars དང་ཅིག་ཁར་ བརྡ་བཀོད་འབད་དེ་ དཔར་བསྐྲུན་འབདཝ་ཨིན།
སྡེབ་ཚན་གསར་པ་།

## ཧུམ་སྲོལ།

I18NF0000008X

I18NI000000016X I18NI000000017X ལས་ `FindNetworkStatus` ལག་ལེན་འཐབ་ཨིན།

## ཨེན་ཌྲོའིཌ།

I18NF0000009X

འཛིན་སྐྱོང་འཐབ་མི་ ཐབས་འཕྲུལ་བརྟག་དཔྱད་འདི་ Nexus གི་གནས་རིམ་ལུ་བརྡུང་ཡོདཔ་ཨིན།

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## དཀའ་ངལ་སེལ་བ།

- TLS འཐུས་ཤོར་→ Nexus གསར་བཏོན་འབད་མི་ཊར་བཱོལ་ལས་ CA བང་རིམ་འདི་ངེས་དཔྱད་འབད།
- I18NI0000000019X → `--lane-id`/I18NI000000021X ཚར་ཅིག་ སྣ་མང་ལམ་ལམ་འགྲུལ་བསྐྱོད་འབད་དོ།
  བསྟར་སྤྱོད་འབད་ཡོདཔ།
- `ERR_SETTLEMENT_PAUSED` → བརྟག་དཔྱད་ [I18NT0000003X བཀོལ་སྤྱོད་](I18NU000000011X) འདི་གི་དོན་ལུ་ཨིན།
  བྱུང་བའི་ལས་རིམ། གཞུང་སྐྱོང་གིས་ ལམ་འདི་ བཀག་བཞག་སྟེ་ཡོདཔ་འོང་།

གཏིང་ཟབ་པའི་སྐབས་དོན་དང་ SDK-དམིགས་བསལ་འགྲེལ་བཤད་ཚུ་བལྟ།
`docs/source/nexus_sdk_quickstarts.md`.