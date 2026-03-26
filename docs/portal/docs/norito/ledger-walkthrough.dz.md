---
lang: dz
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c61035c0e4b0fd478f08beeef34d7ae41415f55b09dc93dfda9490efe94fb91
source_last_modified: "2026-01-22T16:26:46.505734+00:00"
translation_last_reviewed: 2026-02-07
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
slug: /norito/ledger-walkthrough
translator: machine-google-reviewed
---

འདི་གིས་ [Norito མགྱོགས་དྲགས་འགོ་བཙུགས་](./quickstart.md) སྟོན་ཐོག་ལས་ ལྷན་ཐབས་འབདཝ་ཨིན།
`iroha` CLI དང་མཉམ་དུ་ ལག་ལེ་ཇར་གྱི་མངའ་སྡེ་ གང་འདྲ་བྱས་ནས་བརྟག་དཔྱད་བྱེད་དགོས། ཁྱོད་ཀྱིས་ཐོ་བཀོད་འབད་འོང་།
རྒྱུ་དངོས་ངེས་ཚིག་གསརཔ་, ཆ་ཚན་ལ་ལོ་ཅིག་ སྔོན་སྒྲིག་བཀོལ་སྤྱོད་རྩིས་ཐོ་ནང་ བཏོན་གཏང་། སྤོ་བཤུད་འབད། སྤོ་བཤུད་འབད།
ལྷག་ལུས་ཀྱི་ཆ་ཤས་ཅིག་ རྩིས་ཐོ་གཞན་ཅིག་ལུ་དང་ གྲུབ་འབྲས་ཐོན་མི་ཚོང་འབྲེལ་ཚུ་བདེན་སྦྱོར་འབད།
།དང་འཛིན་པ་དང་། གོ་རིམ་རེ་རེ་བཞིན་ མེ་ལོང་རེ་རེ་བཞིན་ རསཊ་/པའེ་ཐོན་/ཇ་བ་ཨིསི་ཀིརིཔ་ནང་ བཀབ་པའི་ བཞུར་རྒྱུན་ཚུ་ འབྱུངམ་ཨིན།
SDK མགྱོགས་དྲགས་འགོ་འཛིན། ཁྱོད་ཀྱིས་ CLI དང་ SDK གི་སྤྱོད་ལམ་གྱི་བར་ན་ ཆ་སྙོམས་འདི་ ངེས་གཏན་བཟོ་ཚུགས།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

- [./quickstart.md) བརྒྱུད་དེ་ པི་རེ་པིར་ཡོངས་འབྲེལ་འདི་ བུཊི་འབད་ནིའི་དོན་ལུ་ [ཁིག་སི་ཊརཊི་](I18NU0000017X) ལུ་བལྟ།
  I18NI0000002X.
- I18NI000000023X (The CLI) བཟོ་བསྐྲུན་ཡང་ན་ཕབ་ལེན་འབད་དེ་ ཁྱོད་ཀྱིས་ ལྷོད་ཚུགས།
  Peer ལག་ལེན་འཐབ་སྟེ་ `defaults/client.toml`.
- གདམ་ཁ་ཅན་གྱི་གྲོགས་རམ་པ་: `jq` (JSON ལན་ཚུ་བཟོ་བཅོས་འབད་ནི) དང་ པི་གི་དོན་ལུ་ པི་ཨོ་ཨེསི་ཨའི་ཨེགསི་ཤེལ་ཅིག།
  འོག་ལུ་ལག་ལེན་འཐབ་མི་མཐའ་འཁོར་གནས་སྟངས་ཀྱི་ཆ་ཤས་ཚུ་ཨིན།

ལམ་སྟོན་གྱི་སྐབས་ལུ་ `$ADMIN_ACCOUNT` དང་ I18NI000000027X འདི་ ཚབ་བཙུགས་དགོ།
རྩིས་ཐོ་ཨའི་ཌི་ ཁྱོད་ཀྱིས་ལག་ལེན་འཐབ་ནི་གི་འཆར་གཞི་ཡོད། སྔོན་སྒྲིག་བུནཌལ་དེ་གིས་ཧེ་མ་ལས་རྩིས་ཐོ་གཉིས་ཚུད་ཡོདཔ་ཨིན།
བརྡ་སྟོན་ལྡེ་མིག་ལས་བྱུང་བ།

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

གནས་གོང་ཚུ་ རྩིས་ཐོ་འགོ་དང་པ་ཚུ་ ཐོ་བཀོད་འབད་དེ་ ངེས་དཔྱད་འབད།

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. རིགས་རིགས་ཀྱི་དཔྱད་གཞི།

ལག་དེབ་འདི་འཚོལ་ཞིབ་འབད་དེ་འགོ་བཙུགས། CLI འདི་དམིགས་གཏད་ཨིན།

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

བརྡ་བཀོད་འདི་ཚུ་གིས་ Norito-backed ལན་ཚུ་ལུ་བློ་གཏད་དེ་ དེ་འབདཝ་ལས་ ཚགས་མ་དང་ ཤོག་ལེབ་འདི་ ༡.
གཏན་འབེབས་དང་ SDKs གིས་ཐོབ་མི་དང་མཐུན་སྒྲིག་འབད།

## 2. བདོག་དད་ངེས་ཚིག་ཐོ་འགོད།

`wonderland` ནང་ལུ་ `coffee` ཟེར་མི་ རྒྱུ་དངོས་གསརཔ་ཅིག་གསར་བསྐྲུན་འབད།
མངའ༌ཁོངས:

I18NF0000008X

CLI གིས་ བཙུགས་ཡོད་པའི་ ཚོང་འབྲེལ་ཧེཤ་ (དཔེར་ན་ ,
`0x5f…`). འདི་སྲུངས་བཞག་འབད་ དེ་འབད་ ཁྱོད་ཀྱིས་ ཤུལ་ལས་གནས་རིམ་འདི་འདྲི་དཔྱད་འབད་ཚུགས།

## 3. བཀོལ་སྤྱོད་རྩིས་ཐོ་ནང་ལུ་ མིན་ཊི་ཡུ་ནིཊི་ཚུ།

རྒྱུ་དངོས་འབོར་ཚད་ `(asset definition, account)` ཆ་གཅིག་གི་འོག་ལུ་སྡོད་དོ་ཡོདཔ་ཨིན། མིན་ཊི་ ༢༥༠།
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` གི་ཡུ་ནིཊི་ཚུ་ I18NI000000033X: ལུ།

I18NF0000009X

ད་རུང་ སི་ཨེལ་ཨའི་ཨའུཊི་པུཊི་ལས་ ཚོང་འབྲེལ་ཧེཤ་ (I18NI0000034X) འདི་ བཟུང་དགོ། ལུ
ལྷག་ལུས་འདི་ཞིབ་དཔྱད་འབད་, གཡོག་བཀོལ།:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ཡང་ན་ རྒྱུ་དངོས་གསརཔ་འདི་རྐྱངམ་ཅིག་དམིགས་གཏད་བསྐྱེད་ནི།

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. ཚད་སྙོམ་པའི་ཆ་ཤས་འདི་རྩིས་ཐོའི་གཞན་དུ་སྤོ་བ།

བཀོལ་སྤྱོད་རྩིས་ཐོ་ལས་ I18NI000000035X ལུ་ ཡུ་ནིཊི་༥༠ སྤོ་བཤུད་འབད།

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

བརྗེ་སོར་ཧེཤ་འདི་ `$TRANSFER_HASH` སྦེ་སྲུང་བཞག་འབད། གཉིས་ཀའི་བདག་དབང་འདྲི་བ།
ལྷག་ལུས་གསརཔ་ཚུ་བདེན་དཔྱད་འབད་ནིའི་རྩིས་ཁྲ་ཚུ།

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. ལེ་ཌི་ཇར་གྱི་སྒྲུབ་བྱེད་བདེན་སྦྱོར་བྱེད་པ།

ཚོང་འབྲེལ་གཉིས་ཆ་རང་ འབད་ཡོདཔ་ངེས་གཏན་བཟོ་ནི་ལུ་ གསོག་འཇོག་འབད་ཡོད་པའི་ ཧ་ཤེ་ཚུ་ལག་ལེན་འཐབ།

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

ཁྱོད་ཀྱིས་ འཕྲལ་གྱི་སྡེབ་ཚན་ཚུ་ཡང་ སྡེབ་ཚན་ག་ཅི་གིས་ གནས་སོར་འདི་ཚུད་ཡོདཔ་ཨིན་ན་ བལྟ་ནི་ལུ་ རྒྱུན་སྤེལ་འབད་ཚུགས།

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

གོང་འཁོད་བརྡ་བཀོད་རེ་རེ་གིས་ ཨེསི་ཌི་ཀེ་ཨེསི་ཚུ་བཟུམ་སྦེ་ I18NT0000002X གི་སྤྲོད་ལེན་ཚུ་ལག་ལེན་འཐབ་ཨིན། ཁྱོད་ཀྱིས་འདྲ་དཔེ་བཟོ་བ་ཅིན།
འདི་ གསང་ཡིག་བརྒྱུད་དེ་ (འོག་གི་ཨེསི་ཌི་ཀེ་ མགྱོགས་དྲགས་ཚུ་བལྟ་) ཧ་ཤེ་དང་ ལྷག་ལུས་ཚུ་ འགྱོ་འོང་།
ཁྱོད་ཀྱིས་ཡོངས་འབྲེལ་གཅིག་དང་སྔོན་སྒྲིག་ཚུ་དམིགས་གཏད་བསྐྱེད་ཚུན་ཚོད་ གྲལ་ཐིག་ཡར་འགྱོ།

## SDK ཆ་སྙོམས་འབྲེལ་མཐུད།

- [Rust SDK མགྱོགས་མྱུར་](../sdks/rust) — ཐོ་བཀོད་ཀྱི་བཀོད་རྒྱ་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།
  ཕུལ་བའི་ཚོང་འབྲེལ་དང་ Rust ལས་ འོས་བསྡུའི་གནས་ཚད།
- [Python SDK མགྱོགས་འགོ་བཙུགས་](I18NU000000019X) — ཐོ་འགོད་/མི་ཊི་གཅིག་པ་སྟོནམ་ཨིན།
  བཀོལ་སྤྱོད་དང་ Norito-backed JSON གྲོགས་རམ་པ།
- [ཇ་བ་ཨིསི་ཀིརིཔ་ཨེསི་ཌི་ཀེ་ མགྱོགས་དྲགས་](../sdks/javascript) — I18NT000000004X ཞུ་བ་ཚུ་ ཁྱབ་སྟེ་ཡོདཔ་ཨིན།
  གཞུང་སྐྱོང་རོགས་སྐྱོར་འབད་མི་དང་ ཡིག་དཔར་རྐྱབ་ཡོད་པའི་འདྲི་དཔྱད་བཀབ་ཆ་ཚུ།

དང་པ་རང་ སི་ཨེལ་ཨའི་ འགྲུལ་བསྐྱོད་འདི་གཡོག་བཀོལ་ཞིནམ་ལས་ ཁྱོད་རའི་དགའ་གདམ་ཨེསི་ཌི་ཀེ་དང་གཅིག་ཁར་ མཐོང་སྣང་འདི་བསྐྱར་ལོག་འབད།
ཁ་ཐོག་གཉིས་ཆ་ར་གིས་ ཚོང་འབྲེལ་གྱི་ཧ་ཤི་དང་ ལྷག་ལུས་ དེ་ལས་ འདྲི་དཔྱད་ཚུ་ལུ་ ཆ་འཇོག་འབད་ནི་ལུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ཨིན།
ཐོན་འབྲས་ཚུ།