---
lang: dz
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 93505cbda553c6d73c4850776545a87723b03a0d922610e6e7786a3f379b8fae
source_last_modified: "2026-07-11T23:16:35+00:00"
translation_last_reviewed: 2026-07-11
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# ཟམ་གྱི་མཐའ་མཇུག་བདེན་ཁུངས།

ཡིག་ཆ་འདི་གིས་ ཐོག་མའི་ཐོན་རིམ་གྱི་ bridge finality རྣམ་གཞག་གཏན་འབེབས་འབདཝ་ཨིན།
བདེན་ཁུངས་ཀྱིས་ Sumeragi v2 གིས་བཟོ་ཞིནམ་ལས་རྒྱུན་བརྟན་སྦེ་བསགས་བཞག་ཡོད་པའི་ finality
evidence ངེས་ཏིག་འབགཔ་ཨིན། Proof envelope གི་ schema version འདི་ `1` དང་ ནང་ན་ཡོད་པའི་
consensus protocol version འདི་ `2` ཨིན། Sumeragi v1 certificate projection, decoder དང་
fallback ལམ་མེད།

## བདེན་ཁུངས་ཀྱི་རྣམ་གཞག་ངེས་ཏིག

Norito ཡང་ན་ Norito JSON གིས་ཨང་བཀོད་འབད་མི་ `BridgeFinalityProof` ལུ་ས་སྒོ་གསུམ་རྐྱངམ་ཅིག་ཡོད།

```text
{ version, block_header, finality_artifact }
```

- `version` འདི་ `1` ངེས་པར་དགོ།
- `block_header` འདི་ ཞུ་བ་འབད་མི་མཐོ་ཚད་ཀྱི་ canonical `BlockHeader` ཨིན།
- `finality_artifact` འདི་ block དེ་གི་དོན་ལུ་བསགས་བཞག་ཡོད་པའི་ `V2FinalityArtifact` ངེས་ཏིག་ཨིན།
  འདི་གིས་ height-context roster གི་གོ་རིམ་བཞིན་ validator རེ་རེའི་ BLS-normal PoP
  (`validator_set_pops`) རྒྱུན་བརྟན་སྦེ་ནང་ན་བཞགཔ་ཨིན།

Artifact ནང་ `HeightContext` ཡོངས་རྫོགས་དང་མི་འགྱུར་བ་ `BlockSubject` ངེས་ཏིག་ block hash,
CommitQC དང་ roster-aligned PoP ཚུ་ཡོད། Height context གིས་ chain, epoch, roster,
`DualQuorum`, DA layout, leader seed དང་ consensus data གཞན་ཚུ་གཏན་འཇགས་བཟོཝ་ཨིན། Epoch
མཇུག་བསྡུ་མི་ parent block གི་ context ནང་ optional `next_epoch_snapshot` ཡང་ཡོད། ས་སྒོ་འདི་
context id གི་ཆ་ཤས་ཨིནམ་ལས་ parent CommitQC གིས་ child roster ལུ་དབང་ཚད་མ་སྤྲོད་པའི་ཧེ་མ་
ངོ་སྤྲོད་འབདཝ་ཨིན། Finalized snapshot གིས་ ཤུལ་མའི་ epoch parameters དང་གཅིག་ཁར་
`epoch_end_height` དང་ ཤུལ་མའི་ roster-aligned `validator_set_pops` ཡང་ངོ་སྤྲོད་འབདཝ་ཨིན།

## རྒྱུན་བརྟན་བསགས་བཞག་དང་བདེན་དཔྱད།

Sumeragi v2 apply path གིས་ artifact བདེན་དཔྱད་འབད་དེ་ མི་འགྱུར་བའི་ Kura sidecar སྦེ་བསགས་བཞགཔ་ཨིན།
Proof builder གིས་ canonical block དང་དེའི་ sidecar ལྷགཔ་ཨིནམ་མ་གཏོགས་ historical PoP ཡང་ན་
certificate འདི་ འགྱུར་བ་ཅན་གྱི་ད་ལྟའི་ world state ལས་ལོག་མི་བཟོ། Sidecar མེད་པ་ མེདཔ་ཐལ་བ་
འགལ་བ་ཡང་ན་བདེན་དཔྱད་མི་ཚུགས་པ་ཨིན་པ་ཅིན་ fail closed ཨིན། ཐོབ་ཚུགས་མི་འདི་ in-memory
history window ཉེ་འདབས་ལུ་མི་ཚད།

Stateless verifier གིས་ version, chain, height, header hash, context, subject དང་ CommitQC ངེས་ཏིག་
མཐུན་སྒྲིག་འབད་དེ་ artifact ནང་གི་ PoP ཆ་མཉམ་བདེན་དཔྱད་འབདཝ་ཨིན། Signer index ཚུ་ཡར་འཕར་
གོ་རིམ་ནང་དང་ཚད་ནང་འཁོད་དགོ། CommitQC གིས་ validator count དང་ voting power quorum གཉིས་ཆ་
ཚང་དགོཔ་དང་ Sumeragi v2 vote preimage ངེས་ཏིག་གུ་ BLS aggregate signature ཆ་གནས་ཅན་དགོ།

## ཡིད་ཆེས་ཀྱི་ anchor དང་ successor བདེན་དཔྱད།

བདེན་ཁུངས་རྐྱང་པ་གིས་ རང་གིས་འབག་མི་ roster འོག་གི་ནང་འཁོད་མཐུན་སྒྲིག་རྐྱངམ་ཅིག་སྟོནམ་ཨིན།
`BridgeFinalityVerifier` གིས་ བདེན་ཁུངས་དང་པ་མ་ལེན་པའི་ཧེ་མ་ གསལ་ཏོག་ཏོ་སྦེ་ཡིད་ཆེས་ཡོད་པའི་
`HeightContextId` དགོ། དེ་ལས་ཐད་ཀར་རྗེས་མའི་མཐོ་ཚད་རྐྱངམ་ཅིག་ལེན་ཏེ་ child context གི་ parent
CommitQC འདི་ ཧེ་མའི་ frozen roster དང་ PoP གིས་བདེན་དཔྱད་འབདཝ་ཨིན། Epoch ནང་འཁོད་ལུ་ child
artifact གིས་ ཧེ་མའི་ artifact PoP ཚུ་འདྲ་བཤུས་འབདཝ་ཨིན། Boundary ལུ་ epoch, roster, quorum,
seed དང་ PoP ཚུ་ parent CommitQC གིས་ངོ་སྤྲོད་འབད་མི་ `next_epoch_snapshot` དང་ དེའི་
`epoch_end_height` ཚུད་ མཐུན་དགོ།
རྙིངམ་ བརྒལ་བ་ ཡང་ན་མ་འབྲེལ་བའི་མཐོ་ཚད་ཚུ་ཆ་མེད་ཨིན།

SCCP གིས་ `BridgeFinalityProof` གཅིག་པ་ལག་ལེན་འཐབ། Message གིས་བྱིན་མི་ roster འོག་གི་ signature
རྐྱངམ་ཅིག་ལུ་ཡིད་ཆེས་མི་བཏུབ། Governance གིས་གཏན་འཇགས་བཟོ་མི་ checkpoint context/artifact ལས་
message artifact ཚུན་ immediate successor རེ་རེ་བདེན་དཔྱད་འབད་དགོ།

## Bundle དང་ API

`BridgeFinalityBundle` འདི་ངེས་ཏིག་ `{ commitment, finality_proof }` ཨིན། Commitment འདི་
`{ chain_id, height_context_id, block_height, block_hash, mmr_root?,
mmr_leaf_index?, mmr_peaks? }` ཨིན། Optional MMR ས་སྒོ་ཚུ་ commitments རྐྱངམ་ཅིག་ཨིནམ་དང་ finality
ཡང་ན་ inclusion proof མེན།

- `GET /v1/bridge/finality/{height}` གིས་ `BridgeFinalityProof` སླར་ལོག་འབདཝ་ཨིན།
- `GET /v1/bridge/finality/bundle/{height}` གིས་ `BridgeFinalityBundle` སླར་ལོག་འབདཝ་ཨིན།

Block ཡང་ན་ v2 artifact རྒྱུན་བརྟན་ངེས་ཏིག་མེད་པ་ ཡང་ན་ཆ་མེད་ཨིན་པ་ཅིན་ endpoint གཉིས་ཆ་ fail closed
ཨིན། ངོ་མ་ཤེས་པའི་ས་སྒོ་ རྒྱབ་སྐྱོར་མེད་པའི་ version དང་ retired proof shape ཚུ་ཆ་མེད་བཟོ་དགོ།
