---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

ད་ལྟོ་ ཟུར་གསོག་+ཁང་གླ་གི་སྲིད་བྱུས་ (ལམ་སབ་ཁྲ་གི་རྣམ་གྲངས་ **SFM‐6**) གིས་ `sorafs reserve` འདི་ བཏངམ་ཨིན།
CLI གྲོགས་རམ་འབད་མི་ཚུ་དང་ I18NI0000010X སྐད་སྒྱུར་པ་ཚུ་ཨིན།
དངུལ་ཁང་གི་རྒྱུག་འགྲན་ཚུ་གིས་ ཁང་གླ་/གསོག་འཇོག་སྤོ་བཤུད་ཚུ་ བཏོན་གཏང་ཚུགས། ཤོག་ངོས་འདིས་མེ་ལོང་།
I18NI000000011X ནང་ངེས་འཛིན་འབད་ཡོད་པའི་ལཱ་གི་རྒྱུན་རིམ་དང་ འགྲེལ་བཤད་རྐྱབ།
དཔལ་འབྱོར་རིག་པ་དང་ Grafana ལུ་ སྤོ་བཤུད་ཀྱི་ཕིཌ་གསརཔ་འདི་ ག་དེ་སྦེ་ གློག་ཐག་བཏང་དགོཔ་ཨིན་ན་དང་ དཔལ་འབྱོར་རིག་པ་དང་།
གཞུང་སྐྱོང་བསྐྱར་ཞིབ་པ་ཚུ་གིས་ བྱུང་འཛིན་འཁོར་རིམ་ག་ར་ལུ་ རྩིས་ཞིབ་འབད་ཚུགས།

## མཇུག་ལས་མཇུག་བསྡུའི་ལས་རིམ།

1. **Quote + ལེཊི་ཇར་པར་བརྙན་**།
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account i105... \
    --treasury-account i105... \
    --reserve-account i105... \
    --asset-definition xor#sora \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   ལེག་ཇར་གྱིས་ I18NI000000012X སྡེབ་ཚན་ (net rese sepe, secain, speer, speer,
   མ་ཤོང་། མཐོ་ཚད་ཌེལ་ཊ་ འོག་གི་བྲི་བ་བུ་ལིན་) དང་ `Transfer` དང་།
   ISIs གིས་ དངུལ་ཁང་དང་ གསོག་འཇོག་རྩིས་ཁྲ་ཚུ་གི་བར་ན་ XOR སྤོ་བཤུད་འབད་དགོཔ་ཐོན་ནུག།

2.*བཞུ་ + Prometheus/NDJSON ཐོན་འབྲས་**
   I18NF0000008X
   ཟས་བཅུད་ཀྱི་གྲོགས་རམ་པ་འདི་གིས་ མའི་ཀོརོ་ཨེགསི་ཨོ་ཨར་ བསྡོམས་རྩིས་ ཨེགསི་ཨོ་ཨར་ ནང་ལུ་ སྤྱིར་བཏང་སྦེ་ ཨེགསི་ཨོ་ཨར་ ནང་ལུ་ བསྡོམས་རྩིས་འབདཝ་ཨིན།
   པར་བརྙན་ཚུ་ རྗེས་སུ་འབྲི་ནི་ལུ་ མཐུན་སྒྲིག་འབདཝ་ཨིནམ་དང་ **transfer ཕིཌི་** མེ་ཊིགསི་ཚུ་ བཏོནམ་ཨིན།
   `sorafs_reserve_ledger_transfer_xor` དང་།
   `sorafs_reserve_ledger_instruction_total`. ལེཌི་ཇར་སྣ་མང་དགོ་པའི་སྐབས།
   ལས་སྦྱོར་ (དཔེར་ན་ བྱིན་མི་ཚུ་གི་སྡེ་ཚན་) ) `--ledger`/`--label` ཆ་ཚུ་ བསྐྱར་ལོག་འབདཝ་ཨིན།
   གྲོགས་རམ་པ་འདི་གིས་ NDJSON/Prometheus ཡིག་སྣོད་གཅིག་འབྲི་སྟེ་ བཞུ་ཚད་ཆ་མཉམ་ཡོདཔ་ཨིན།
   dashboods གིས་ འཁོར་རིམ་ཆ་མཉམ་རང་ མེ་ཏོག་མེད་པར་ བཏོནམ་ཨིན། `--out-prom`
   ཡིག་སྣོད་དམིགས་གཏད་ཚུ་གིས་ མཐུད་མཚམས་ཕྱིར་འདྲེན་འབད་མི་ཚིག་ཡིག་ཡིག་སྣོད་བསྡུ་སྒྲིག་འབད་མི་— I18NI000000019X ཡིག་སྣོད་འདི་ནང་ལུ་བཀོག་བཞག་།
   ཕྱིར་ཚོང་པ་གིས་ སྣོད་ཐོ་བལྟ་ ཡང་ན་ ཊེ་ལི་མི་ཊི་བཱ་ཀེཊ་ལུ་ སྐྱེལ་བཙུགས་འབད་ཡོདཔ་ཨིན།
   གསོག་འཇོག་འབད་ཡོད་པའི་ ཌེཤ་བོརཌ་ལཱ་གིས་ བཀོལ་སྤྱོད་འབདཝ་ཨིན་— I18NI000000020X འདི་ ཅོག་འཐདཔ་འབདཝ་ཨིན།
   གནད་སྡུད་མདོང་ལམ་ནང་ papploads.

3. **དཔེ་སྐྲུན་ཁང་། + སྒྲུབ་བྱེད་**
   - `artifacts/sorafs_reserve/ledger/<provider>/` དང་འབྲེལ་མཐུད་འོག་ལུ་ བཞུ་ཚུགས།
     ཁྱོད་ཀྱི་བདུན་ཕྲག་དཔལ་འབྱོར་སྙན་ཞུ་ལས་ རྟགས་བཅུད་བསྡུས་པ།
   - JSON ཟས་བཅུད་འཚིག་ཡོད་མི་འདི་ ཁང་གླ་མེ་གིས་འཚིགས་ཏེ་ (དེ་འབདཝ་ལས་ རྩིས་ཞིབ་པ་ཚུ་གིས་ བསྐྱར་རྩེད་འབད་ཚུགས།
     math) དང་ གཞུང་སྐྱོང་སྒྲུབ་བྱེད་ཀྱི་ ཐུམ་སྒྲིལ་ནང་ ཅེག་སམ་ཚུ་ ཚུདཔ་ཨིན།
   - བཞུ་ཁུ་གིས་ མགོ་ཡར་ ཡང་ན་ འོག་རིམ་གྱི་ བརྡ་རྟགས་ཅིག་ བརྡ་སྟོན་པ་ཅིན་ ཉེན་བརྡ་ལུ་ གཞི་བསྟུན་འབད།
     ID (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) དང་ ISIs སྤོ་བཤུད་གང་འདྲ་ཞིག་ཡིན་ནམ།
     གློག་རིམ་

## མེ་ཊིགསི་ → བཀྲམ་ཤོག་ → ཉེན་བརྡ།

| འབྱུང་ཁུངས་མེ་ཊིག | I18NT0000004X པེ་ནཱལ་ | དྲན་སྐུལ་/ སྲིད་བྱུས་ཧུཀ་ | དྲན་ཐོ། |
|------------------------------------------------------------------- --------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA ཁང་གླ་བཀྲམ་ (XOR/hour)” ནང་ I18NI0000027X | བདུན་ཕྲག་རེའི་ བང་མཛོད་ཀྱི་ བཞུ་བཅོས་ལུ་ ལྟོ་བྱིན་དགོ། `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) ནང་ལུ་ གསོག་འཇོག་འབད་སའི་ ཁྱབ་སྤེལ་ནང་ spikes ཚུ་ཨིན། |
| `torii_da_rent_gib_months_total` | “ཤུགས་ཚད་ལག་ལེན་ (GiB-ཟླཝ་)” (Dashboard) | ཨེགསི་ཨོ་ཨར་ སྤོ་བཤུད་ཀྱི་ བྱུང་འཛིན་གསོག་འཇོག་མཐུན་སྒྲིག་ཚུ་ བདེན་ཁུངས་བསྐྱལ་ནིའི་དོན་ལུ་ ལེད་ཇར་ བཞུ་བཅོས་དང་གཅིག་ཁར་ ཆ་སྒྲིག་འབད། |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor`, “Snapshot (XOR)” + གནས་ཚད་ཤོག་བྱང་ཚུ་ I18NI000000034X ནང་ གསོག་འཇོག་འབད། | `SoraFSReserveLedgerTopUpRequired` I18NI000000036X སྐབས་མེ་འབར་བ་དང་། I18NI000000037X `meets_underwriting=0` སྐབས་མེ་འབར་བ། |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind” “Latest Transfer Breakdown” དང་ I18NI0000041X ནང་ ཁྱབ་ཁོངས་ཤོག་བྱང་། | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing`, དང་ I18NI000000044X ལུ་ སྤོ་བཤུད་ཀྱི་ཕིཌི་འདི་ མེད་པའི་སྐབས་ ཡང་ན་ ཀླད་ཀོར་འབད་ཡོདཔ་སྦེ་ ཉེན་བརྡ་འབདཝ་ཨིན། གནད་དོན་གཅིག་ནང་ ཁྱབ་ཚད་ཤོག་བྱང་ཚུ་ ༠% ལུ་ལྷོདཔ་ཨིན། |

ཁང་གླའི་འཁོར་རིམ་འདི་མཇུག་བསྡུ་བའི་སྐབས་ I18NT000000002X/NDJSON པར་ལེན་ཚུ་གསར་བསྐྲུན་འབད་ཞིནམ་ལས་ ངེས་གཏན་བཟོ།
Grafana པེ་ནཱལ་ཚུ་གིས་ `label` གསརཔ་འདི་ འཐུ་ཞིནམ་ལས་ གསལ་གཞི་ཚུ་ + མཉམ་སྦྲགས་འབདཝ་ཨིན།
ཉེན་བརྡ་འབད་མི་ ID ཚུ་ གཞུང་སྐྱོང་ཁང་ཚན་གྱི་སྦུང་ཚན་ལུ་ཨིན། འདི་གིས་ CLI དམིགས་ཚད་འདི་ བདེན་ཁུངས་བཀལཝ་ཨིན།
telemetry དང་གཞུང་གིས་ *same** སྤོ་བཤུད་ཀྱི་ཕིཌ་དང་ ལས་རིགས་ཆ་མཉམ་ལུ་ རྩ་བ་བཏོནམ་ཨིན།
ལམ་སྟོན་གྱི་དཔལ་འབྱོར་གྱི་ བརྡ་དོན་བཀོད་སྒྲིག་ཚུ་ ཟུར་གསོག་+ཁང་གླ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
རང་འགོད། ཁྱབ་ཁོངས་ཤོག་བྱང་ཚུ་ བརྒྱ་ཆ་༡༠༠ (ཡང་ན་ ༡.༠) དང་ དྲན་བསྐུལ་གསརཔ་ཚུ་ལྷག་དགོ།
ཁང་གླ་འདི་ བཤུབ་དགོཔ་དང་ མཐོ་རིམ་སྤོ་བཤུད་ཚུ་ བཞུ་ཁུ་ནང་ལུ་ཡོདཔ་ཨིན།