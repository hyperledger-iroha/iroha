---
id: account-address-status
lang: dz
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ཀེ་ནོ་ནིག་ཨེ་ཌི་ཌི་ཨར་-༢ བུནཌལ་ (I18NI0000008X) འཛིན་བཟུང་ཚུ།
I105 (འདོད་པ་) བསྡམ་བཞག་ཡོདཔ་ (`sora`, གཉིས་པ་ གཉིས་པ་/ཕྱེད་ཀ་/རྒྱ་ཚད་) མཚན་རྟགས་མང་པོའི་ནང་དང་ ངན་པའི་སྒྲིག་བཀོད་ཚུ།
རེ་རེ་བཞིན་ SDK + I18NT0000004X ཁ་ཐོག་འདི་ JSON གཅིག་པ་ལུ་བརྟེན་ཏེ་ ང་བཅས་ཀྱིས་ ཀོ་ཌེཀ་གང་རུང་ཅིག་ བརྟག་དཔྱད་འབད་ཚུགས།
བཟོ་སྐྲུན་མ་འབད་བའི་ཧེ་མ་ drift . ཤོག་ལེབ་འདི་གིས་ ནང་འཁོད་གནས་རིམ་མདོར་བསྡུས་འདི་ གསལ་སྟོན་འབདཝ་ཨིན།
(I18NI000000010X རྩ་བའི་མཛོད་ཁང་ནང་) སོ་སོ།
ལྷག་མི་ཚུ་གིས་ མོ་ནོ་-རི་པོ་བརྒྱུད་དེ་ མ་འཕྲི་བར་ལཱ་གི་རྒྱུན་རིམ་འདི་ གཞི་བསྟུན་འབད་ཚུགས།

## བརྩེགས་འདི་བསྐྱར་བཟོ་ཡང་ན་བདེན་དཔྱད་འབད།

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

སྐུགས་:

- I18NI000000011X — དུས་ཚོད་བརྟག་དཔྱད་ཀྱི་དོན་ལུ་ ཇེ་ཨེསི་ཨེན་འདི་ stdout ལུ་བཏོན་དགོ།
- `--out <path>` — འགྲུལ་ལམ་སོ་སོ་ཅིག་ལུ་བྲིས་ (དཔེར་ན་ བསྒྱུར་བཅོས་བསྒྱུར་བཅོས་འབད་བའི་སྐབས་)།
- `--verify` — ལཱ་གི་འདྲ་བཤུས་འདི་ གསརཔ་སྦེ་བཟོ་ཡོད་པའི་ནང་དོན་ལུ་ ག་བསྡུར་འབད།
  `--stdout` དང་མཉམ་སྡེབ་འབད།)

སི་ཨའི་ལཱ་གི་རྒྱུན་རིམ་ **ཨེཌ་རེསི་ཝེག་ཊར་ཌིརཕཊི་** གིས་ `cargo xtask address-vectors --verify` གཡོག་བཀོལཝ་ཨིན།
བསྐྱར་ཞིབ་འབད་མི་ཚུ་ལུ་ ཉེན་བརྡ་འབད་ནི་ལུ་ སྒྲིག་ཆས་དང་ གློག་ཤུགས་འཕྲུལ་ཆས་ ཡང་ན་ ཡིག་ཆ་ཚུ་ ག་དུས་འབད་རུང་ འགྱུར་བཅོས་འབདཝ་ཨིན།

## ག་གིས་ སྒྲིག་ཆས་འདི་ ཟ་སྤྱོད་འབདཝ་སྨོ?

| ཁ་ཐོག་ | བདེན་དཔྱད་ |
|--------------------------------------------------------------------------------
| རསཊི་གནས་སྡུད་-དཔེ་ཚད་ | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| I18NT00000005 (སར་བར་) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| ཇ་བ་ཨིསི་ཀིརིཔཊི་ཨེསི་ཌི་ཀེ་ | `javascript/iroha_js/test/address.test.js` |
| Swift ཨེསི་ཌི་ཀེ་ | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

མདའ་རྟགས་སྐོར་ཐེངས་རེ་རེ་བཞིན་ ཀེནོ་ནིག་བཱའིཊིསི་ + I105 + བསྡམ་བཞག་ཡོདཔ་ (`sora`, གཉིས་པ་དྲག་ཤོས་) ཨིན་ཀོ་ཌིང་དང་།
Norito-style འཛོལ་བའི་གསང་ཡིག་ཚུ་ ངན་པའི་གནད་དོན་ཚུ་གི་དོན་ལུ་ རིམ་སྒྲིག་དང་གཅིག་ཁར་ གྱལ་རིམ་སྦེ་ ཞིབ་དཔྱད་འབདཝ་ཨིན།

## རང་འགུལ་དགོཔ་ཨིན་ན?

ལག་ཆས་བཏོན་མི་འདི་གིས་ ཡིག་ཚུགས་སྒྲིག་ཆས་འདི་ གྲོགས་རམ་པ་དང་གཅིག་ཁར་ གསར་བསྐྲུན་འབད་ཚུགས།
I18NI0000002X, གིས་ ཀེ་ནོ་ནིཀ་འདི་ ལེན་ཡོདཔ་ ཡང་ན་ བདེན་དཔྱད་འབདཝ་ཨིན།
འདྲ་བཤུས་མེད་པའི་ bundle : གོ་རིམ་:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

གྲོགས་རམ་པ་འདི་གིས་ `--source` འདི་ ཆ་གནས་འབདཝ་ཨིནམ་དང་ ཡང་ན་ `IROHA_ACCOUNT_FIXTURE_URL` འདི་ཨིན།
མཐའ་འཁོར་འགྱུར་ཅན་དེ་འབདཝ་ལས་ SDK CI ལཱ་ཚུ་ ཁོང་རའི་དགའ་གདམ་ཅན་གྱི་མེ་ལོང་ལུ་སྟོན་ཚུགས།
I18NI000000025X འདི་བཀྲམ་སྤེལ་འབད་བའི་སྐབས་ གྲོགས་རམ་འབད་མི་གིས་བྲིས་ཡོདཔ་ཨིན།
I18NI000000026X དང་ ཀེན་ནོ་ཀལ་གཉིས་དང་མཉམ་དུ།
SHA-256 བཞུ་ (I18NI0000027X) དེ་ དེ་ དེ་ I18NT000000001X ཚིག་ཡིག་ ཡིག་འཕྲིན་ ཨིན།
བསྡུ་སྒྲིག་དང་ Grafana བརྡ་བཀོད་ `account_address_fixture_status` གིས་ བདེན་ཁུངས་བཀལ་ཚུགས།
ཁ་ཐོག་ག་ར་མཉམ་འབྱུང་ནང་ལུསཔ་ཨིན། དམིགས་གཏད་སྙན་ཞུ་ `0` ག་དུས་ཡིན་ནམ། དོན་ལུ
སྣ་མང་ཁ་ཐོག་རང་བཞིན་གྱིས་ བཀབ་ཆ་ `ci/account_fixture_metrics.sh` ལག་ལེན་འཐབ་ཨིན།
(ངོས་ལེན་ཚུ་ བསྐྱར་ལོག་ `--target label=path[::source]`) དེ་འབདཝ་ལས་ ཁ་པར་སྡེ་ཚན་ཚུ་གིས་ དཔར་བསྐྲུན་འབད་ཚུགས།
གཅིག་མཉམ་བསྡོམས་འབད་ཡོད་པའི་ `.prom` ཡིག་སྣོད་འདི་ མཐུད་མཚམས་ཕྱིར་འདྲེན་འབད་མི་ ཚིག་ཡིག་ཡིག་སྣོད་བསྡུ་སྒྲིག་འབད་མི་གི་དོན་ལུ་ཨིན།

## མདོར་བསྡུས་ཆ་ཚང་དགོས་སམ།

ADDR-2 ཆ་ཚང་མཐུན་སྒྲིག་གནས་རིམ་ (ཇོ་བདག་, ལྟ་རྟོག་འཆར་གཞི། ཁ་ཕྱེ་བའི་བྱ་སྤྱོད།)
I18NI000000033X ནང་སྡོད་ཡོད།
ཁ་བྱང་བཟོ་བཀོད་ RFC (`docs/account_structure.md`) དང་མཉམ་དུ། ཤོག་ལེབ་འདི་ སྦེ་ལག་ལེན་འཐབ།
མགྱོགས་དྲགས་བཀོལ་སྤྱོད་དྲན་སྐུལ་; རི་པོ་ཌོག་ཚུ་ལུ་ ལམ་སྟོན་གཏིང་ཟབ་སྦེ་ གཞི་བསྟུན་འབད།