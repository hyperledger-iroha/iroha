---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-29T18:16:35.177959+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-reconciliation
title: SoraFS Capacity Reconciliation
description: Nightly workflow for matching capacity fee ledgers to XOR transfer exports.
translator: machine-google-reviewed
---

རོ་ཌི་མེཔ་རྣམ་གྲངས་ **SF-2c** དངུལ་ཁང་གིས་ ཤོང་ཚད་ཀྱི་འཐུས་རྩིས་ཤོག་འདི་ བདེན་ཁུངས་བཀལ་དགོ་པའི་ བཀའ་རྒྱ་འབདཝ་ཨིན།
མཚན་རེ་ལུ་ལག་ལེན་འཐབ་ཡོད་མི་ ཨེགསི་ཨོ་ཨར་ སྤོ་བཤུད་ཚུ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན། ལག་ལེན་འཐབ།
`scripts/telemetry/capacity_reconcile.py` རོགས་རམ་འབད་མི་ གྲོགས་རམ་པ།
`/v1/sorafs/capacity/state` ལག་ལེན་འཐབ་ཡོད་པའི་ སྤོ་བཤུད་ཀྱི་སྡེ་ཚན་དང་ རྒྱབ་འགལ་འབད་ནི།
emit Prometheus དྲན་སྐུལ་གྱི་དོན་ལུ་ ཚིག་ཡིག་ཡིག་སྣོད་ཀྱི་མེ་ཊིགསི།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།
- ལྕོགས་གྲུབ་གནས་སྟངས་ཀྱི་པར་བཏབ་ (`fee_ledger` ingent) I18NT000000001X ལས་ཕྱིར་འདྲེན་འབད་ཡོདཔ།
- Ledger གཅིག་པའི་སྒོ་སྒྲིག་ (JSON ཡང་ན་ NDJSON དང་ `provider_id_hex`, with, with,
  I18NI000000007X = གཞིས་ཆགས་/ཉེས་ཆད་དང་ I18NI0000008X).
- ཁྱོད་ལུ་ཉེན་བརྡ་ཚུ་དགོ་པ་ཅིན་ node_ཕྱིར་འདྲེན་འབད་མི་ ཚིག་ཡིག་ཡིག་སྣོད་བསྡུ་སྒྲིག་འབད་མི་ལུ་འགྲུལ་ལམ་།

## གཡོག་བཀོལ།
I18NF0000002X

- ཕྱིར་ཐོན་ཨང་རྟགས་: མཐུན་སྒྲིག་གཙང་མ་ཅིག་གུ་ I18NI0000009X, གཞིས་ཆགས་/ཉེས་ཆད་ཚུ་བརླག་སྟོར་ཞུགས་པའི་སྐབས་ `1`
  ཡང་ན་ དངུལ་སྤྲོད་ཚད་ལས་བརྒལ་བའི་ ཨིན་པུཊ་ཚུ་གུ་ `2` ཨིན།
- ༢༠༠༨ ལུ་ ཇེ་ཨེསི་ཨོ་ཨེན་ བཅུད་བསྡུས་ + ཧ་ཤེད་ཚུ་ མཉམ་སྦྲགས་འབད།
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- I18NI000000013X ཡིག་སྣོད་བསྡུ་ལེན་པ་ནང་ ཡིག་སྣོད་ཚུ་ཡོད་པའི་སྐབས་ དྲན་སྐུལ་འདི་ཨིན།
  `SoraFSCapacityReconciliationMismatch` (མཐོང་།
  I18NI000000015X) གང་ལྟར་ཡང་མེ་འབར་བ།
  གླ་ཆ་མང་དྲགས་སྦེ་ ཡང་ན་ རེ་བ་མེད་པའི་ བྱིན་མི་སྤོ་བཤུད་ཚུ་ བརྟག་དཔྱད་འབད་ཡོདཔ་ཨིན།

## ཐོན་འབྲས་ཚུ།
- གཞིས་ཆགས་དང་ཉེས་ཆད་ཚུ་གི་དོན་ལུ་ ཁྱད་པར་ཡོད་པའི་ གནས་རིམ་ཚུ།
- བསྡོམས་རྩིས་སྦེ་ཕྱིར་འདྲེན་འབད་ཡོདཔ།
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## རེ་བ་བསྐྱེད་པའི་ཁྱབ་ཚད་དང་བཟོད་བསྲན།
- མཐུན་སྒྲིག་འདི་ ཏན་ཏན་སྦེ་ ངེས་བདེན་ཨིན་: རེ་བ་བསྐྱེད་མི་དང་ གཞིས་ཆགས་/ཉེས་ཆད་ནེ་ནོ་ཚུ་ བཟོད་བསྲན་ཀླད་ཀོར་དང་ མཐུན་སྒྲིག་འབད་དགོ། ཀླད་ཀོར་མེན་པའི་ཁྱད་པར་གང་རུང་ཤོག་ལེབ་བཀོལ་སྤྱོད་པ་ཚུ་དགོ།
- CI གིས་ ཉིནམ་༣༠ གི་རིང་ ཤོང་ཚད་ཀྱི་འཐུས་ (བརྟག་དཔྱད་ I18NI0000021X) གི་དོན་ལུ་ `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` ལུ་ བླུག་དོ་ཡོདཔ་ཨིན། གོང་ཚད་དང་ ཡང་ན་ བསིལ་དྲོད་ཀྱི་ ཡིག་བརྡའི་རིགས་ཚུ་ བསྒྱུར་བཅོས་འབད་བའི་སྐབས་རྐྱངམ་གཅིག་ ཟས་བཅུད་གསརཔ་ གསརཔ་བཟོ།
- དཀར་ཆག་གསལ་སྡུད་ནང་ (I18NI0000023X, `strike_threshold=u32::MAX`) ཉེས་ཆད་ཚུ་ ཀླད་ཀོར་ནང་སྡོད་དགོ། ཐོན་སྐྱེད་འདི་གིས་ ལག་ལེན་འཐབ་ཐངས་/ཡར་འཕར་/PoR གི་ཐིང་གཞི་ཚུ་ བརྡལ་བཤིག་གཏང་པའི་སྐབས་རྐྱངམ་ཅིག་ ཉེས་ཆད་ཚུ་ བཏོན་དགོཔ་དང་ རིམ་སྒྲིག་འབད་ཡོད་པའི་ བསིལ་དྲོད་ལུ་ གུས་ཞབས་འབད་དགོ།