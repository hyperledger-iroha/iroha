---
lang: dz
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami ཨའི་རོ་ཧ་༣ གསལ་སྡུད།

Kagami ཚུ་ Iroha གི་དོན་ལུ་ སྔོན་སྒྲིག་འབདཝ་ཨིནམ་ལས་ བཀོལ་སྤྱོད་པ་ཚུ་གིས་ གཏན་འབེབས་བཟོ་ཚུགས།
རིགས་མཚན་འདི་ ཡོངས་འབྲེལ་རེ་ལུ་ མཛུབ་མོ་མེད་པར་ མངོན་གསལ་འབདཝ་ཨིན།

- གསལ་སྡུད་: `iroha3-dev` (རྒྱུན་རིམ་ `iroha3-dev.local`, བསྡུ་སྒྲིག་ k=1, r=1, VRF འདི་ NPoS སེལ་འཐུ་འབད་བའི་སྐབས་ རིམ་སྒྲིག་ཨའི་ཌི་ལས་འབྱུང་ཡོདཔ་ཨིན།) `iroha3-taira` (chain Kagami, བསྡུ་སྒྲིག་ཀེ་=༣, དགོས་མཁོ་ཡི། NPoS སེལ་འཐུ་འབད་བའི་སྐབས་ `--vrf-seed-hex` X ལུ་ `iroha3-nexus` (རིམ་སྒྲིག་ `iroha3-nexus`, བསྡུ་སྒྲིག་པ་ k=5 r=3, NPoS སེལ་འཐུ་འབད་བའི་སྐབས་ `--vrf-seed-hex` དགོཔ་ཨིན།)
- མོས་མཐུན་: སོ་ར་གསལ་སྡུད་དྲ་རྒྱ་ (Nexus + dataspaces) ལུ་ NPoS དང་ གོ་རིམ་མེད་པའི་གནས་རིམ་གྱི་ མཐུད་མཚམས་ཚུ་དགོཔ་ཨིན། གནང་བ་ཡོད་པའི་ Iroha3 བཀྲམ་སྤེལ་ཚུ་ སོ་ར་གསལ་སྡུད་མེད་པར་ གཡོག་བཀོལ་དགོ།
- མི་རབས་: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. `--consensus-mode npos` གི་དོན་ལུ་ Nexus ལག་ལེན་འཐབ། `--vrf-seed-hex` འདི་ NPoS གི་དོན་ལུ་རྐྱངམ་ཅིག་ ནུས་ཅན་ (taira/Nexus གི་དོན་ལུ་དགོས་མཁོ་ཡོདཔ་ཨིན།)། Kagami གིས་ Iroha3 གི་ཐིག་གུ་ DA/RBC གིས་ བཅུད་བསྡུས་ཅིག་བཟོ་སྟེ་ བཅུད་བསྡུས་ཅིག་བཟོཝ་ཨིན།
- བདེན་དཔྱད་: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` གསལ་སྡུད་ཀྱི་རེ་བ་ཚུ་ ལོག་གཏངམ་ཨིན། (རིམ་སྒྲིག་ཨའི་ཌི་, ཌི་ཨེ་/ཨར་བི་སི། བཀྲམ་སྤེལ་ `--vrf-seed-hex` བརྟག་དཔྱད་/འབྲེལ་མཐུན་གྱི་དོན་ལུ་ NPoS མངོན་གསལ་འབད་བའི་སྐབས་རྐྱངམ་ཅིག་ བཀྲམ་སྤེལ་འབདཝ་ཨིན།
- དཔེ་ཚད་ཀྱི་བང་རིམ་: སྔོན་ལས་བཟོ་མི་ བང་རིམ་ཚུ་ `defaults/kagami/iroha3-{dev,taira,nexus}/` གི་འོག་ལུ་སྡོད་དོ་ཡོདཔ་ཨིན། `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` དང་མཉམ་དུ་བསྐྱར་བཟོ་འབད།
- Mochi: `mochi`/`mochi-genesis` དང་ `--genesis-profile <profile>` དང་ `--vrf-seed-hex <hex>` (PooS only) གིས་ Kagami ལུ་ དེ་བཟུམ་མའི་ཐོག་ལས་ གསལ་སྡུད་ལུ་ Kagami འདི་ གསལ་སྡུད་ལུ་ studout/stderr. ལག་ལེན་འཐབ་ཡོདཔ།

བཱན་ཌལ་ཚུ་གིས་ BLS PoPs ཚུ་ ཊོ་པོ་ལོ་ཇི་ཐོ་བཀོད་ཚུ་དང་གཅིག་ཁར་ བཙུགས་ཏེ་ཡོདཔ་ལས་ `kagami verify` མཐར་འཁྱོལ་བྱུངམ་ཨིན།
སྒྲོམ་ནང་ལས་ཕྱི་ཁར་; ས་གནས་ཀྱི་དོན་ལུ་དགོ་པའི་ རིམ་སྒྲིག་ཚུ་ནང་ བློ་གཏད་ཅན་གྱི་ མཉམ་རོགས་/འདྲེན་ལམ་ཚུ་ བདེ་སྒྲིག་འབད།
དུ་བ་བརྒྱུགས་པ་ཡི།