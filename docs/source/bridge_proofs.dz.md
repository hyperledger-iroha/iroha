---
lang: dz
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 465d8cf704022986b169ab93133517428f8cf2ffe01a498cbda458f4a5b2e69b
source_last_modified: "2026-07-11"
translation_last_reviewed: 2026-07-11
translator: machine-assisted
---

> ཤོག་ལེབ་འདི་ བསྡུས་པའི་སྐད་བསྒྱུར་བཅུད་དོན་ཙམ་ཨིནམ་ལས་
> ཆ་ཚང་གི་སྐད་བསྒྱུར་མེན། བཅའ་ཁྲིམས་ API དང་ བདེན་དཔང་གི་དོན་དག་
> དེ་ལས་གསར་བཏོན་གྱི་དགོས་མཁོ་ཚུ་གི་དོན་ལུ་
> [ཨིང་སྐད་ཀྱི་གཞི་རྟེན་ཤོག་ལེབ](bridge_proofs.md) ལག་ལེན་འཐབ།

# SCCP V1 ཟམ་པའི་བདེན་དཔང་ — བཅུད་དོན།

## གསར་བཏོན་དང་པའི་ཚད།

SCCP V1 འདི་ གསར་བཏོན་དང་པའི་དོན་ལུ་ཁ་བསྡམས་པའི་ལམ་ལུགས་ཨིན།
ཕྱིའི་འབྱུང་ཁུངས་ `ethereum-mainnet`, `bsc-mainnet` དང་
`tron-mainnet` རྐྱངམ་ཅིག་ལུ་རྒྱབ་སྐྱོར་ཡོད། SORA གི་འགྲོ་ཡུལ་གཅིག་པུ་
`sora-taira` ཨིན། Solana, TON, སྒེར་གྱི་ཡོངས་འབྲེལ་ ཡང་ན་ SORA
གི་འགྲོ་ཡུལ་གཞན་ཚུ་ལུ་རྒྱབ་སྐྱོར་མེདཔ་ལས་ ཉེན་སྲུང་ཐོག་ལས་ངོས་ལེན་མི་འབད།

གསར་བཏོན་འདི་ནང་ `SubmitBridgeProof` གིས་ དབྱེ་བ་ཅན་གྱི
`NativeProtocol` དང་ `SccpDestination` བདེན་དཔང་རྐྱངམ་ཅིག་ལེནམ་ཨིན།
སྤྱིར་བཏང་ `Ics` དང་ `TransparentZk` ཕུལ་ནི་མེདཔ་ལས་
དབང་ཚད་ཅན་གྱི on-chain verifier མ་འཐོབ་ཚུན་ཚོད་ངོས་ལེན་མི་འབད།

## དབྱེ་བ་ཅན་གྱི་ཐོ་དེབ་དང་ replay ཉེན་སྲུང་།

`SccpRegistryV1` འདི་ lane ལུ་བསྡམས་པའི་ དབྱེ་བ་ཅན་དང་
ཁ་སྐོང་རྐྱངམ་ཅིག་འབད་བཏུབ་པའི་ (append-only) ཐོ་དེབ་ཨིན།
lane རེ་རེ་ནང་ route revision 64 དང་ native trust anchor 4,096 ཚུན་ཚོད་
བཞག་ཚུགས། ལོ་རྒྱུས་ཀྱི་ཐོ་ཚུ་རང་བཞིན་གྱིས་མི་བཏོན། ཚད་ལུ་ལྷོདཔ་ད་
ཁ་སྐོང་ཤུལ་མམ་འདི་ state མ་བསྒྱུར་བར་ངོས་ལེན་མི་འབད།

Anchor interval འདི་ authentication ཡོད་པའི consensus ཡར་རྒྱས་ཀྱི་
coordinate གིས་ཚད་འཇལཝ་ཨིན། Ethereum གིས་ finalized beacon slot དང་
BSC/TRON གིས་ finalized native block height ལག་ལེན་འཐབ། Anchor རྙིངམ་
འདི་ successor checkpoint ཚུད་དེ་ནུས་ཅན་ཨིན། Anchor མཐའ་མམ་འདི་
མཇུག་ཁ་ཕྱེ་སྟེ་ཡོད། Terminal route གི finality cutoff འདི་ ལོ་རྒྱུས་ཀྱི་
anchor གི successor checkpoint དང་ཏག་ཏག་འདྲ་དགོ།

རྒྱུན་བརྟན་ inbound ཐོ་གིས་ event/source finality height དང་
`anchor_interval_height` གཉིས་ཆ་ར་བཞགཔ་ཨིན། lane དང་ anchor hash གིས་
ལྡེ་མིག་བཟོ་བའི high-water index གིས་ ཧེ་མ་ངོས་ལེན་འབད་མི་ coordinate
ལས་དམའ་བའི successor checkpoint གདམ་ཁ་རྐྱབ་མི་བཅུག། Snapshot hydration
གིས་ index འདི་རྒྱུན་བརྟན་ཐོ་ལས་ལོག་རྩིས་བཏོན་ཏེ་ ཏག་ཏག་འདྲ་དགོ།
མེད་པ་ རྙིངམ་ མེདཔ་བཏང་མི་ ཡང་ན་རྒྱབ་རྟེན་མེད་པའི index ངོས་ལེན་མི་འབད།

## ཐེངས་གཅིག་གི་བདེན་དཔྱད་དང་ ལཱ་གི་ཚད།

Destination དང་ native བདེན་དཔང་ཚུ་ ཐེངས་གཅིག་བཀྲལ་ ཐེངས་གཅིག་བསྡམས་
ཞིནམ་ལས་ གསང་རྩིས་ཀྱི་ལཱ་གོང་ཆེན་མ་འབད་བའི་ཧེ་མ་ deterministic work
reserve འབདཝ་ཨིན། Destination ལམ་གྱིས་ BN254 pairing-product དང་
ས་གནས་ཀྱི BLS finality ཐེངས་གཅིག་རེ་བདེན་དཔྱད་འབད། Native ལམ་ཚུ་གིས་
canonical shortest-prefix དགོ། BSC ལུ header 1,004 དང་ TRON ལུ 54 གི་ཚད་ཨིན།

`[zk.sccp]` གིས་ proof count/bytes, native headers/bytes, Ethereum light-client
updates, secp256k1 recoveries, BLS aggregate checks/key contributions དང་ BN254
pairing checks ཚུ་ལུ་ transaction དང་ block རེ་ལུ་ཀླད་ཀོར་མེན་པའི་ཚད་བཀལཝ་ཨིན།
ངོས་ལེན་གྱི་ཚད་ཚུ་ consensus-bound ཨིནམ་ལས་ validator ཆ་མཉམ་གྱི་
config file ནང་གནས་གོང་གཅིག་པ་དགོ། Environment-variable override མེད།

གསར་བཏོན་དང་པའི་སྔོན་སྒྲིག་ཚད་ཚུ།

| ལཱ་གི་ཚད | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| BN254 pairing-product checks | 1 | 4 |

Proof གཅིག་ནང་ canonical bytes 8 MiB ལས་མང་མི་ཆོག། བཀོག་བཞག་པ་
ཡང་ན་ངོས་ལེན་མ་འབད་བའི transaction གི་ reserved work འདི་ block ནང་མི་འཛུལ།

## Torii དང་ HTTP ཚད།

Torii གིས་ SCCP endpoint རེ་ལུ་ JSON body གི་ཚད་སོ་སོ་བཀལཝ་ཨིན།
ཚད་འདི་ body མ་ལྷག་པ་ memory མ་བགོ་བ་ དེ་ལས་གསང་རྩིས་བདེན་དཔྱད་མ་འབད་བའི་ཧེ་མ་བཀལཝ་ཨིན།
ཚད་ལས་བརྒལ་བའི `Content-Length` ཡང་ན་ chunked body འདི་ HTTP `413` གིས་ངོས་ལེན་མི་འབད།
Client གིས་ decoded HTTP response ཡང་ཚད་ཅན་གྱི་ནང་ལྷགཔ་ཨིནམ་ལས་
`Content-Length` མེད་པ་ཡང་ན་རྫུན་མ་གིས་ཚད་བརྒལ་མི་ཚུགས།

JSON, base64 དང་ Norito ཨིན་པུཊ་ཚུ་ canonical དགོ། Unknown fields,
duplicate keys, network/route/anchor མ་མཐུན་པ་ replay ལཱ་གི་ཚད་བརྒལ་བ་
ཡང་ན་བདེན་དཔྱད་མ་འགྲུབ་པ་ཚུ་ state ཆ་ཤས་ཅིག་ཡང་མ་བསྒྱུར་བར་ངོས་ལེན་མི་འབད།
