---
lang: dz
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS མ་པ་སི་ཨེལ་ཨའི་ མཇུག་ལས་མཇུག་-མཐའ་མཚམས།

དཔེ་འདི་གིས་ ཡིག་ཆ་ཚུ་ SoraFS ལུ་ལག་ལེན་འཐབ་སྟེ་ དཔར་བསྐྲུན་འབད་ཐོག་ལས་ འགྱོཝ་ཨིན།
`sorafs_manifest_stub` དང་མཉམ་དུ་ གཏན་འབེབས་ཆ་ཤས་སྒྲིག་བཀོད་དང་མཉམ་དུ།
SoraFS བཟོ་བཀོད་ RFC ནང་གསལ་བཀོད་འབད་ཡོདཔ། རྒྱུན་འབབ་འདི་གིས་ རྙིང་མའི་མི་རབས་འདི་ཁྱབ་ཨིན།
རེ་བ་བརྟག་དཔྱད་དང་ འཐོབ་ཚུགས།
སྡེ་ཚན་ཚུ་གིས་ CI ནང་ གོམ་པ་གཅིག་སྦེ་ བཙུགས་ཚུགས།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

- ལཱ་གི་ས་སྒོ་འདི་ རིགས་མཚུངས་བཟོ་བཅོས་འབད་དེ་ ལག་ཆས་ཚུ་ གྲ་སྒྲིག་ཡོདཔ་ཨིན། (`cargo`, `rustc`).
- `fixtures/sorafs_chunker` ལས་ ཕིགསི་ཅར་ཚུ་འཐོབ་ཚུགསཔ་ལས་ རེ་བ་གནས་གོང་ཚུ་ ཐོབ་ཚུགས།
  section (ཐོན་སྐྱེད་གཡོག་བཀོལ་ནིའི་དོན་ལུ་ གནས་སྤོའི་ལག་དེབ་ཐོ་བཀོད་ལས་ གནས་གོང་ཚུ་འཐེན།
  ཅ་རྙིང་དང་འབྲེལ་བ་ཡོདཔ།)
- དཔེ་སྐྲུན་འབད་ནི་ལུ་ དཔེ་ཚད་པེ་ལོཌི་སྣོད་ཐོ་ (དཔེ་འདི་གིས་ `docs/book` ལག་ལེན་འཐབ་ཨིན།)

## གོ་རིམ་༡ པ་ — གསལ་སྟོན་དང་ སི་ཨར་ མཚན་རྟགས་ དེ་ལས་ འཐོབ་ཚུགས་པའི་འཆར་གཞི་བཟོ་ནི།

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

བཀའ་བཀོད།

- `ChunkProfile::DEFAULT` བརྒྱུད་དེ་ པེ་ལོཌ་འདི་བརྐྱབ།
- CARv2 གཏན་མཛོད་ཅིག་དང་ ཅནཀ་-ཕེཆ་འཆར་གཞི་ཅིག་ བཏོནམ་ཨིན།
- `ManifestV1` དྲན་ཐོ་ཅིག་བཟོ་བསྐྲུན་འབདཝ་ཨིན་ གསལ་སྟོན་གྱི་མཚན་རྟགས་ (བྱིན་པ་ཅིན་) དང་།
  ཡིག་ཤུབས་འདི་བྲིས།
- བཱའིཊིསི་འཕྱེལ་འགྱོ་བ་ཅིན་ རེ་བ་སྐྱེད་ཀྱི་དར་ཆ་ཚུ་བསྟར་སྤྱོད་འབདཝ་ལས་ གཡོག་བཀོལ་མི་འདི་ འཐུས་ཤོར་འབྱུང་འོང་།

## གོ་རིམ་ ༢ པ་ — ཅངཀ་ཚོང་ཁང་ + པོ་ཨར་ བསྐྱར་སྦྱོང་དང་གཅིག་ཁར་ ཐོན་འབྲས་ཚུ་བདེན་སྦྱོར་འབད།

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

འདི་གིས་ གཏན་འབེབས་བཟོ་ནིའི་ཆ་ཤས་ཚོང་ཁང་བརྒྱུད་དེ་ CAR འདི་ བསྐྱར་རྩེད་འབདཝ་ཨིན།
དཔེ་ཚད་ཤིང་གི་བདེན་ཁུངས་དང་ གསལ་སྟོན་སྙན་ཞུ་འདི་ འོས་འབབ་ཅན་བཟོཝ་ཨིན།
གཞུང་སྐྱོང་བསྐྱར་ཞིབ།

## གོ་རིམ་ ༣ པ་ — སྣ་མང་མཁོ་སྤྲོད་པ་ ལོག་ཐོབ་བཀོད།

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

སི་ཨའི་མཐའ་འཁོར་ཚུ་གི་དོན་ལུ་ བྱིན་མི་རེ་ལུ་ པེ་ལོཌ་འགྲུལ་ལམ་སོ་སོ་སྦེ་བྱིན་ (དཔེར་ན་ སྦྱར་བརྩེགས་འབད་ཡོདཔ།
ལུས་སྦྱོང་འབད་ནི་གི་དོན་ལུ་ དུས་ཚོད་ཀྱི་དུས་ཚོད་དང་ འཐུས་ཤོར་འཛིན་སྐྱོང་འཐབ་ནི།

## གོ་རིམ་༤ པ། — ཐོ་བཀོད་ལེབ་འཛིན།

པར་སྐྲུན་འདི་ `docs/source/sorafs/migration_ledger.md` ནང་ལུ་བཀོདཔ་ཨིན།

- Manifest CID, CAR ཟས་འཇུ་དང་ ཚོགས་སྡེའི་མིང་རྟགས་ ཧ།
གནས་སྟངས་ (`Draft`, `Staging`, `Pinned`).
- CI ལུ་མཐུད་འབྲེལ་ཡང་ན་ གཞུང་སྐྱོང་ཤོག་བྱང་།

## གོ་རིམ་༥ — གཞུང་སྐྱོང་བརྒྱུད་དེ་ པིན་ (ཐོ་བཀོད་འབད་བའི་སྐབས་ འཚོ་བའི་སྐབས།)

པིན་ཐོ་བཀོད་འདི་བཙུགས་ཚརཝ་ད་ (གནས་སྤོ་འགྱོ་བའི་ལམ་ཐིག་ནང་ མའི་ལི་སི་ཊོན་ཨེམ་༢)།
གསལ་སྟོན་འདི་ CLI བརྒྱུད་དེ་བཙུགས།

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

གྲོས་འཆར་ངོས་འཛིན་དང་རྗེས་མའི་ཆ་འཇོག་ཚོང་འབྲེལ་ཧ་སི་ཚུ་ཨིན་དགོ།
རྩིས་ཞིབ་འབད་ནི་གི་དོན་ལུ་ གནས་སྤོ་འགྱོ་མི་ ལག་དེབ་ཐོ་བཀོད་ནང་ བཏོན་ཡོདཔ་ཨིན།

## གཙང་སྦྲ།

`target/sorafs/` གི་འོག་ལུ་ཡོད་པའི་ ཅ་རྙིང་ཚུ་ གཏན་མཛོད་དང་ ཡང་ན་ གནས་རིམ་གྱི་ མཐུད་མཚམས་ཚུ་ནང་ སྐྱེལ་བཙུགས་འབད་ཚུགས།
གསལ་སྟོན་དང་ མཚན་རྟགས་ CAR དེ་ལས་ འཆར་གཞི་ཚུ་གཅིག་ཁར་བསྡོམས་ཏེ་ མར་ཕྱོགས་ལུ་བཞག་དགོ།
བཀོལ་སྤྱོད་པ་དང་ཨེསི་ཌི་ཀེ་སྡེ་ཚན་ཚུ་གིས་ བཀྲམ་སྤེལ་འདི་ གཏན་འབེབས་བཟོ་ཐོག་ལས་ བདེན་དཔྱད་འབད་ཚུགས།