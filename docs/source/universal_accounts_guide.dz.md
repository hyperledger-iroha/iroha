<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ཡོངས་ཁྱབ་རྩིས་ཁྲའི་ལམ་སྟོན།

ལམ་སྟོན་འདི་གིས་ ཡུ་ཨེ་ཨའི་ཌི་ (ཡོངས་ཁྱབ་རྩིས་ཐོ་ཨའི་ཌི་) བཀྲམ་སྤེལ་དགོས་མཁོ་ཚུ་ ༡ ལས་ བཙག་འཐུ་འབདཝ་ཨིན།
the Nexus ལམ་གྱི་སབ་ཁྲ་དང་ དེ་ཚུ་ བཀོལ་སྤྱོད་པ་ + ཨེསི་ཌི་ཀེ་ གཙོ་བོར་བཏོན་མི་ འགྲུལ་བསྐྱོད་ཅིག་ནང་ ཐུམ་སྒྲིལ་འབདཝ་ཨིན།
འདི་གིས་ ཡུ་ཨེ་ཨའི་ཌི་འབྱུང་ཁུངས་ ཡིག་ཆ་/གསལ་སྟོན་ཞིབ་དཔྱད་ ཚད་འཛིན་ཊེམ་པེལེཊ་ཚུ་ ཁྱབ་ཚུགསཔ་ཨིན།
དང་ `iroha app space-directory མངོན་གསལ་རེ་རེ་དང་མཉམ་དུ་འགྲོ་དགོས་པའི་སྒྲུབ་བྱེད།
publish` run (roadmap reference: `ལམ་གྱི་ས་ཁྲ།md:2209`).

## 1. UAID མགྱོགས་གཞི་གཞི་བསྟུན།- ཡུ་ཨེ་ཨའི་ཌི་ཚུ་ `uaid:<hex>` ཡིག་འབྲུ་ཚུ་ཨིནམ་ད་ `<hex>` འདི་ Blake2b-256 དྲའི་ཇེསཊི་ཨིན་
  LSB འདི་ `1` ལུ་གཞི་སྒྲིག་འབད་ཡོདཔ་ཨིན། ཀེ་ནོ་ནིཀ་དབྱེ་བ་འདི་ ༡༩༩༠ ལུ་སྡོདཔ་ཨིན།
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- རྩིས་ཐོའི་དྲན་ཐོ་ (`Account` དང་ `AccountDetails`) ཚུ་གིས་ ད་ལྟོ་གདམ་ཁ་ཅན་གྱི་ `uaid` འབག་འོང་།
  field དེ་འབདཝ་ལས་ གློག་རིམ་ཚུ་གིས་ ངོས་འཛིན་འབད་མི་འདི་ བེསི་པོཀ་ཧ་ཤིང་མེད་པར་ ལྷབ་ཚུགས།
- གསང་བའི་ལས་འགན་ངོས་འཛིན་འབད་མི་སྲིད་བྱུས་ཚུ་གིས་ གང་བྱུང་སྤྱིར་བཏང་བཟོ་ཡོད་པའི་ཨིན་པུཊི་ཚུ་ བསྡམ་ཚུགས།
  (ཁ་པར་ཨང་གྲངས་དང་གློག་འཕྲིན་ རྩིས་ཐོའི་ཨང་གྲངས་ མཉམ་འབྲེལ་ཡིག་རྒྱུན་ཚུ་) `opaque:` IDs ལུ་
  ཡུ་ཨེ་ཨའི་ཌི་མིང་གནས་ཅིག་གི་འོག་ལུ། རིམ་སྒྲིག་ཐོག་གི་ཆ་ཤས་ཚུ་ `IdentifierPolicy`, ཨིན།
  `IdentifierClaimRecord`, དང་ `opaque_id -> uaid` ཟུར་ཐོ།
- ས་སྟོང་སྣོད་ཐོ་གིས་ ཡུ་ཨེ་ཨའི་ཌི་རེ་རེ་མཐུད་མི་ `World::uaid_dataspaces` སབ་ཁྲ་ཅིག་ རྒྱུན་སྐྱོང་འཐབ་ཨིན།
  ཤུགས་ལྡན་གསལ་སྟོན་ཚུ་གིས་གཞི་བསྟུན་འབད་མི་ གནད་སྡུད་ས་སྟོང་རྩིས་ཐོ་ཚུ་ལུ་ཨིན། Torii དེ་ལོག་སྟེ་ལག་ལེན་འཐབ་ཨིན།
  `/portfolio` དང་ `/uaids/*` APIs གི་དོན་ལུ་སབ་ཁྲ།
- `POST /v1/accounts/onboard` གིས་ 2019 གི་དོན་ལུ་ སྔོན་སྒྲིག་བར་སྟོང་སྣོད་ཐོ་གསལ་སྟོན་ཅིག་དཔར་བསྐྲུན་འབདཝ་ཨིན།
  འཛམ་གླིང་གནད་སྡུད་ས་སྟོང་འདི་ ག་ཡང་མེད་པའི་སྐབས་ དེ་འབདཝ་ལས་ ཡུ་ཨེ་ཨའི་ཌི་འདི་ དེ་འཕྲོ་ལས་ བསྡམ་བཞགཔ་ཨིན།
  བཀོད་སྒྲིག་དབང་འཛིན་ཚུ་གིས་ `CanPublishSpaceDirectoryManifest{dataspace=0}` བཟུང་དགོ།
- ཨེསི་ཌི་ཀེ་ཨེསི་ཆ་མཉམ་གྱིས་ ཡུ་ཨེ་ཨའི་ཌི་ཡིག་འབྲུ་ཚུ་ ཁྲིམས་མཐུན་བཟོ་ནི་ལུ་ གྲོགས་རམ་འབད་མི་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།
  ཨེན་ཌོརཌ་ཨེསི་ཌི་ཀེ་ནང་ `UaidLiteral`)། གྲོགས་རམ་འབད་མི་ཚུ་གིས་ ༦༤-ཧེགསི་དྲའི་ཇེསཊི་སྔོ་མ་ཚུ་ངོས་ལེན་འབདཝ་ཨིན།
  (LSB=1) ཡང་ན་ `uaid:<hex>` ཡིག་འབྲུ་ཚུ་དང་ གཅིག་མཚུངས་ Norito ཀོ་ཌེཀ་ཚུ་ལོག་སྟེ་ལག་ལེན་འཐབ།
  digest གིས་ སྐད་ཡིག་ཚུ་གི་བར་ན་ འཕྱེལ་འགྱོ་མི་ཚུགས།

## ༡.༡ གསང་བའི་ངོས་འཛིན་སྲིད་བྱུས་ཚུ།

ཡུ་ཨེ་ཨའི་ཌི་ཚུ་ ད་ལྟོ་ངོ་རྟགས་བང་རིམ་གཉིས་པའི་དོན་ལུ་ གཞི་རྟེན་ཨིན།- ཡོངས་ཁྱབ་`IdentifierPolicyId` (`<kind>#<business_rule>`) གིས་ ངེས་འཛིན་འབདཝ་ཨིན།
  མིང་གནས་ མི་མང་ཁས་བླངས་མེ་ཊ་ཌེ་ཊ་ ཐབས་ཤེས་བདེན་དཔྱད་ལྡེ་མིག་ དེ་ལས་
  ཀེ་ནོ་ནིཀ་ཨིན་པུཊི་སྤྱིར་བཏང་བཟོ་ནི་ཐབས་ལམ་ (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress`, ཡང་ན་ `AccountNumber`).
- ཐོབ་བརྗོད་ཅིག་གིས་ འབྱུང་ཁུངས་`opaque:` ངོས་འཛིན་འབད་མི་གཅིག་ ཡུ་ཨེ་ཨའི་ཌི་གཅིག་དང་ གཅིག་ལུ་ ཏན་ཏན་སྦེ་ བསྡམ་བཞགཔ་ཨིན།
  སྲིད་བྱུས་དེའི་འོག་ལུ་ canonical `AccountId` དེ་འབདཝ་ད་ རིམ་སྒྲིག་འདི་གིས་ རྐྱངམ་ཅིག་ངོས་ལེན་འབདཝ་ཨིན།
  མཚན་རྟགས་བཀོད་ཡོད་པའི་ `IdentifierResolutionReceipt` དང་ཅིག་ཁར་ཡོད་པའི་སྐབས་ ཐོབ་བརྗོད་བཀོད།
- ཐག་གཅོད་འདི་ `resolve -> transfer` འཕྲོ་མཐུད་ཅིག་སྦེ་ལུསཔ་ཨིན། Torii གིས་ དྭངས་གསལ་ཅན་འདི་སེལ་འཐུ་འབདཝ་ཨིན།
  འཛིན་སྐྱོང་འཐབ་སྟེ་ ཀེ་ནོ་ནིཀ་ `AccountId` སླར་ལོག་འབདཝ་ཨིན། སྤོ་བཤུད་ད་དུང་ཡང་དམིགས་གཏད་ཡོད།
  ཀེ་ནོ་ནིཀ་རྩིས་ཐོ་ ཐད་ཀར་དུ་ `uaid:` ཡང་ན་ `opaque:` ཡིག་འབྲུ་མེན།
- སྲིད་བྱུས་ཚུ་གིས་ ད་ལྟོ་ བི་ཨེཕ་ཝི་ཨིན་པུཊི་-གསང་བཟོ་ཚད་བཟུང་ཚུ་ བརྒྱུད་དེ་ དཔར་བསྐྲུན་འབད་ཚུགས།
  `PolicyCommitment.public_parameters`. ཡོད་པའི་སྐབས་ Torii གིས་ཁོང་ཚོ་ ༡ ཐོག་ཁྱབ་བསྒྲགས་འབདཝ་ཨིན།
  `GET /v1/identifier-policies`, དང་ མཁོ་མངགས་འབད་མི་ཚུ་གིས་ བི་ཨེཕ་ཝི་-བཀབ་ཡོད་པའི་ཨིན་པུཊི་བཙུགས་ཚུགས།
  ཚིག་ཡིག་གསལ་པོའི་ཚབ་ལུ། ལས་རིམ་བཟོ་ཡོད་པའི་སྲིད་བྱུས་ཚུ་གིས་ བི་ཨེཕ་ཝི་ཚད་བཟུང་ཚུ་ གཅིག་ནང་བཀབ་བཞགཔ་ཨིན།
  ཀེ་ནོ་ནིཀ་ `BfvProgrammedPublicParameters` བང་སྒྲིག་འདི་གིས་ཡང་ དཔར་བསྐྲུན་འབདཝ་ཨིན།
  public `ram_fhe_profile`; གནའ་བོའི་བི་ཨེཕ་ཝི་པེ་ལོ་ཌི་ཚུ་ དེ་གུ་ཡར་འཕར་འབད་ཡོདཔ་ཨིན།
  ཁས་བླངས་འདི་ལོག་སྟེ་བཟོ་བསྐྲུན་འབད་བའི་སྐབས་ canonical bundle ཨིན།
- ངོས་འཛིན་འབད་མི་འགྲུལ་ལམ་ཚུ་ Torii འཛུལ་སྤྱོད་-རྟགས་མཚན་དང་ ཚད་གཞི་-ཚད་གཞི་གཅིག་པ་བརྒྱུད་དེ་འགྱོཝ་ཨིན།
  གཞན་མི་གློག་རིམ་གདོང་ཐུག་མཐའ་མཚམས་ཚུ་བཟུམ་སྦེ་ཞིབ་དཔྱད་འབདཝ་ཨིན། དེ་ཚུ་ སྤྱིར་བཏང་གི་མཐའ་འཁོར་ལུ་ བཱའི་པ་སི་མེན།
  API སྲིད་བྱུས།

## ༡.༢ ཐ་སྙད་རིག་པ།

མིང་བཏགས་ནི་འདི་ བསམ་བཞིན་དུ་ཨིན།- `ram_lfe` འདི་ ཕྱི་ཁའི་གསང་བའི་ལས་འགན་བཅུད་དོན་ཨིན། སྲིད་བྱུས་ཁྱབ་ཡོད།
  ཐོ་འགོད་དང་ཁས་ལེན་ མི་མང་མེ་ཊ་ཌེ་ཊ་ ལག་ལེན་འཐབ་ཐངས་ཀྱི་འབྱོར་འཛིན་ དེ་ལས་
  བདེན་དཔྱད་ཐབས་ལམ།
- `BFV` འདི་ བེརེ་ཀར་སི་ཀི་/ཕེན་-ཝར་ཀའུ་ཊར་ཧོ་མོ་མོར་ཕིག་གསང་བཟོའི་འཆར་གཞི་འདི་ཨིན།
  གསང་བཟོས་ཨིན་པུཊི་བརྟག་ཞིབ་འབད་ནི་ལུ་ `ram_lfe` རྒྱབ་གཞི་ལ་ལོ་ཅིག།
- `ram_fhe_profile` འདི་ བི་ཨེཕ་ཝི་-དམིགས་བསལ་མེ་ཊ་ཌེ་ཊ་ཨིན་ ཆ་མཉམ་གྱི་དོན་ལུ་ མིང་གཉིས་པ་མེན།
  ཁྱད་ཆོས། འདི་གིས་ ལས་རིམ་བཟོ་ཡོད་པའི་ BFV ལག་ལེན་འཐབ་འཕྲུལ་ཆས་འདི་ དངུལ་ཁུག་དང་
  སྲིད་བྱུས་ཅིག་གིས་ ལས་རིམ་བཟོ་ཡོད་པའི་རྒྱབ་གཞི་ལག་ལེན་འཐབ་པའི་སྐབས་ བདེན་དཔྱད་འབད་མི་ཚུ་གིས་ དམིགས་གཏད་བསྐྱེད་དགོ།

དངོས་ཡོད་ཀྱི་ཐ་སྙད་ནང་།

- `RamLfeProgramPolicy` དང་ `RamLfeExecutionReceipt` ཚུ་ LFE-བང་རིམ་གྱི་དབྱེ་བ་ཚུ་ཨིན།
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters`, དང་
  `BfvRamProgramProfile` ཚུ་ FHE-བང་རིམ་དབྱེ་བ་ཚུ་ཨིན།
- `HiddenRamFheProgram` དང་ `HiddenRamFheInstruction` ཚུ་ ༡ གི་དོན་ལུ་ ནང་འཁོད་མིང་ཚུ་ཨིན།
  ལས་རིམ་བཟོ་ཡོད་པའི་རྒྱབ་གཞི་གིས་ ལག་ལེན་འཐབ་མི་ སྦ་བཞག་ཡོད་པའི་ བི་ཨེཕ་ཝི་ལས་རིམ་འདི། དེ་ཚུ་ གུར་སྡོདཔ་ཨིན།
  FHE ཕྱོགས་ ག་ཅི་འབད་ཟེར་བ་ཅིན་ ཁོང་གིས་ གསང་བཟོས་ལག་ལེན་འཐབ་ཐབས་ལམ་འདི་ འགྲེལ་བཤད་རྐྱབ་མི་ལས་
  ཕྱིའི་སྲིད་བྱུས་ཡང་ན་ འོང་འབབ་ཀྱི་བཅུད་དོན།

## ༡.༣ རྩིས་ཐོའི་ངོ་རྟགས་དང་མིང་གཞན་ཚུ།

ཡོངས་ཁྱབ་-རྩིས་ཐོ་བཤུད་བརྙན་འདི་གིས་ ཀེ་ནོ་ནིཀ་རྩིས་ཐོ་ངོ་རྟགས་དཔེ་ཚད་འདི་བསྒྱུར་བཅོས་མི་འབད།- `AccountId` འདི་ ཁྲིམས་མཐུན་དང་ མངའ་ཁོངས་མེད་པའི་རྩིས་ཐོའི་དོན་ཚན་སྦེ་ལུསཔ་ཨིན།
- `AccountAlias` གནས་གོང་ཚུ་ དོན་ཚན་དེ་གི་མགུ་ལུ་ ཨེསི་ཨེན་ཨེསི་བཱའིན་ཌིང་སོ་སོ་ཨིན། A
  མངའ་ཁོངས་-ཤེས་ཚད་ཅན་གྱི་མིང་གཞན་ དཔེར་ན་ `merchant@banka.paynet` དང་ གནད་སྡུད་ས་སྟོང་-རྩ་བའི་མིང་གཞན་ཅིག
  དཔེར་ན་ `merchant@paynet` གཉིས་ཆ་ར་གིས་ ཀེན་ནོ་ནིཀ་ `AccountId` གཅིག་མཚུངས་ལུ་ ཐག་བཅད་ཚུགས།
- ཀེ་ནོ་ནིཀ་རྩིས་ཐོ་ཐོ་བཀོད་འདི་རྟག་བུ་རང་ `Account::new(AccountId)` / ཨིན།
  `NewAccount::new(AccountId)`; མངའ་ཁོངས་ཤེས་ཚད་ཅན་དང་ཡང་ན་མངའ་ཁོངས་དངོས་པོ་མེད།
  ཐོ་བཀོད་འགྲོ་ལམ།
- མངའ་ཁོངས་བདག་དབང་དང་ མིང་གཞན་ཆོག་ཐམ་ དེ་ལས་ གཞན་མངའ་ཁོངས་ཁྱབ་ཁོངས་སྤྱོད་ལམ་ཚུ་ གསོན་པོ་སྦེ་སྡོདཔ་ཨིན།
  རྩིས་ཐོའི་ངོ་རྟགས་འདི་གུ་མེན་པར་ ཁོང་རའི་གནས་སྟངས་དང་ཨེ་པི་ཨའི་ཚུ་ནང་ལུ།
- མི་མང་རྩིས་ཐོ་འཚོལ་ཞིབ་འདི་གིས་ བགོ་བཤའ་རྐྱབ་མི་འདི་ལུ་རྗེས་སུ་འཇུགཔ་ཨིན། མིང་གཞན་འདྲི་དཔྱད་ཚུ་ མི་མང་ལུ་སྡོདཔ་ཨིན།
  ཁྲིམས་མཐུན་རྩིས་ཁྲའི་ངོ་རྟགས་འདི་ `AccountId` གཙང་མ་སྦེ་ལུསཔ་ཨིན།

བཀོལ་སྤྱོད་པ་ཚུ་དང་ ཨེསི་ཌི་ཀེ་ཨེསི་ དེ་ལས་ བརྟག་དཔྱད་ཚུ་གི་དོན་ལུ་ ལག་ལེན་འཐབ་ནིའི་ལམ་ལུགས་: ཀེ་ནོ་ནིཀ་ལས་འགོ་བཙུགས།
`AccountId`, དེ་ལས་ མིང་གཞན་གླ་ཁར་གཏང་ནི་དང་ གནད་སྡུད་ས་སྟོང་/མངའ་ཁོངས་གནང་བ་ཚུ་ དེ་ལས་ གང་རུང་ཅིག་ཁ་སྐོང་རྐྱབས།
མངའ་ཁོངས་བདག་དབང་ཡོད་པའི་གནས་སྟངས་སོ་སོ། མིང་གཞན་ལས་བྱུང་བའི་རྩིས་ཐོ་རྫུས་མ་མཉམ་སྦྱོར་མ་འབད།
ཡང་ན་ རྩིས་ཐོའི་དྲན་ཐོ་ཚུ་ནང་ འབྲེལ་མཐུད་འབད་ཡོད་པའི་མངའ་ཁོངས་ས་སྒོ་གང་རུང་ཅིག་ རེ་བ་བསྐྱེདཔ་ ག་ཅི་འབད་ཟེར་བ་ཅིན་ མིང་གཞན་ཅིག་ཡང་ན་
route གིས་ མངའ་ཁོངས་ཆ་ཤས་ཅིག་འབག་འོང་།

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

## 2. UAIDs འབྱུང་ཁུངས་དང་བདེན་དཔྱད་འབད་ནི།

ཡུ་ཨེ་ཨའི་ཌི་ཐོབ་ཐབས་ལུ་རྒྱབ་སྐྱོར་འབད་མི་ཐབས་ལམ་གསུམ་ཡོདཔ་ཨིན།

1. **འཛམ་གླིང་གནས་སྟངས་ཡང་ན་ SDK དཔེ་ཚད་ལས་ལྷག།** གང་རུང་ `Account`/`AccountDetails`
   ད་ལྟོ་ Torii བརྒྱུད་དེ་འདྲི་དཔྱད་འབད་མི་ payload འདི་ `uaid` ས་སྒོ་འདི་ མི་རློབས་བཙུགས་ཡོདཔ་ཨིན།
   བཅའ་མར་གཏོགས་མི་གིས་ ཡོངས་ཁྱབ་རྩིས་ཁྲ་ཚུ་ནང་ གདམ་ཁ་རྐྱབ་ནུག།
2. **ཡུ་ཨེ་ཨའི་ཌི་ཐོ་བཀོད་ཚུ་འདྲི་དཔྱད་འབད།** Torii གསལ་སྟོན་འབདཝ་ཨིན།
   `GET /v1/space-directory/uaids/{uaid}` དེ་གིས་ གནད་སྡུད་ས་སྟོང་བཱའིན་ཌིང་ཚུ་སླར་ལོག་འབདཝ་ཨིན།
   དང་ གསལ་སྟོན་མེ་ཊ་ཌེ་ཊ་ ས་སྟོང་སྣོད་ཐོའི་ཧོསིཊི་འདི་ གནས་ཏེ་ཡོདཔ་ཨིན་ (བལྟ།
   `docs/space-directory.md` §3 དཔེ་ཚད་ཀྱི་དོན་ལུ་)།
3. **Deriverit it deterministically.** ཡུ་ཨེ་ཨའི་ཌི་གསརཔ་ཚུ་ བུཊི་སི་ཊརཔ་འབད་བའི་སྐབས་ ཨོཕ་ལའིན་ནང་ ཧེཤ་
   བེལེཀ་༢བི་-༢༥༦ དང་ཅིག་ཁར་ ཁྲིམས་མཐུན་བཅའ་མར་གཏོགས་མི་སོན་དང་ གྲུབ་འབྲས་འདི་ སྔོན་སྒྲིག་འབད།
   `uaid:`. འོག་གི་ཚིག་དུམ་འདི་གིས་ ༡ ནང་ཡིག་ཐོག་ལུ་བཀོད་ཡོད་པའི་ གྲོགས་རམ་འབད་མི་འདི་ མེ་ལོང་འབདཝ་ཨིན།
   `docs/space-directory.md` §༣.༣:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```ཨ་རྟག་ར་ ཡིག་འབྲུ་འདི་ཡིག་འབྲུ་ཆུང་བ་ནང་གསོག་འཇོག་འབད་ཞིནམ་ལས་ ཧེ་ཤི་མ་འབད་བའི་ཧེ་མ་ བར་སྟོང་དཀརཔོ་འདི་སྤྱིར་བཏང་བཟོ།
`iroha app space-directory manifest scaffold` དང་ Android བཟུམ་གྱི་ CLI གྲོགས་རམ་འབད་མི་ཚུ།
`UaidLiteral` དབྱེ་དཔྱད་པ་གིས་ གཞུང་སྐྱོང་བསྐྱར་ཞིབ་ཚུ་ འབད་ཚུགས།
དུས་ཐོག་ཡིག་ཚུགས་ཚུ་མེད་པར་ གནས་གོང་ཚུ་ ཕར་ཚུར་ཞིབ་དཔྱད་འབད།

## ༣ ཡུ་ཨེ་ཨའི་ཌི་བདག་དབང་དང་ གསལ་བསྒྲགས་ཚུ་ བརྟག་དཔྱད་འབད་ནི།

`iroha_core::nexus::portfolio` ནང་ཡོད་པའི་ གཏན་འབེབས་ཡིག་ཆ་བསྡུ་སྒྲིག་འབད་མི་འདི་
ཡུ་ཨེ་ཨའི་ཌི་ལུ་གཞི་བསྟུན་འབད་མི་ རྒྱུ་དངོས་/གནད་སྡུད་ས་སྟོང་ཆ་གཅིག་རེ་ལུ་ ཁ་ཐོག་བཏངམ་ཨིན། བཀོལ་སྤྱོད་པ་དང་ ཨེསི་ཌི་ཀེ་ཚུ།
འོག་གི་ཁ་ཐོག་ཚུ་བརྒྱུད་དེ་ གནད་སྡུད་འདི་ཟ་སྤྱོད་འབད་ཚུགས།

| ཁ་ངོས། | བེད་སྤྱོད། |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | གནད་སྡུད་ས་སྟོང་ → རྒྱུ་དངོས་ → ལྷག་ལུས་བཅུད་དོན་ཚུ་སླར་ལོག་འབདཝ་ཨིན། `docs/source/torii/portfolio_api.md` ནང་འགྲེལ་བཤད་རྐྱབ་ཡོདཔ། |
| `GET /v1/space-directory/uaids/{uaid}` | གནད་སྡུད་ས་སྟོང་ཨའི་ཌི་ཚུ་ + ཡུ་ཨེ་ཨའི་ཌི་ལུ་མཐུད་ཡོད་པའི་རྩིས་ཐོའི་ཡིག་འབྲུ་ཚུ་ཐོ་བཀོད་འབདཝ་ཨིན། |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | རྩིས་ཞིབ་ཚུ་གི་དོན་ལུ་ `AssetPermissionManifest` བྱུང་རབས་ཆ་ཚང་བྱིནམ་ཨིན། |
| `iroha app space-directory bindings fetch --uaid <literal>` | བཱའིན་ཌིང་མཇུག་སྣོད་འདི་བཀབ་སྟེ་ གདམ་ཁ་ཅན་སྦེ་ ཇེ་ཨེསི་ཨོ་ཨེན་འདི་ ཌིཀསི་ལུ་འབྲིཝ་ཨིན་མི་ སི་ཨེལ་ཨའི་མགྱོགས་ཐབས། (`--json-out`) |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | སྒྲུབ་བྱེད་ཐུམ་སྒྲིལ་ཚུ་གི་དོན་ལུ་ གསལ་སྟོན་ཇེ་ཨེསི་ཨོ་ཨེན་བང་རིམ་འདི་འཐེནམ་ཨིན། |

དཔེར་ན་ སི་ཨེལ་ཨའི་ལཱ་ཡུན་ (ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༡༩༥ཨེགསི་ནང་ ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༡༩༤ཨེགསི་བརྒྱུད་དེ་རིམ་སྒྲིག་འབད་ཡོད་པའི་ཨའི་༡༨ཨེན་ཊི་༠༠༠༠༠༠༠༡༦ཨེགསི་ཡུ་ཨར་ཨེལ):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

བསྐྱར་ཞིབ་ཀྱི་སྐབས་ལུ་ལག་ལེན་འཐབ་ཡོད་པའི་ གསལ་སྟོན་ཧེཤ་གི་མཐའ་མར་ JSON པར་ཆས་ཚུ་གསོག་འཇོག་འབད། འདི་
བར་སྟོང་སྣོད་ཐོ་ལྟ་རྟོག་པ་གིས་ ག་དུས་འབད་རུང་ གསལ་སྟོན་འབད་བའི་སྐབས་ `uaid_dataspaces` སབ་ཁྲ་འདི་ལོག་སྟེ་བཟོ་བསྐྲུན་འབདཝ་ཨིན།
ཤུགས་ལྡན་བཟོ་ནི་ དུས་ཡུན་རྫོགས་ནི་ ཡང་ན་ ཆ་མེད་གཏང་ནི་ དེ་འབདཝ་ལས་ པར་རིས་འདི་ཚུ་ བདེན་ཁུངས་བཀལ་ནི་གི་ཐབས་ལམ་མགྱོགས་ཤོས་ཅིག་ཨིན།
དུས་སྐབས་ཅིག་ནང་ བཱའིན་ཌིང་ག་ཅི་ཚུ་ ཤུགས་ལྡན་སྦེ་ཡོདཔ་ཨིན་ན།## 4. དཔར་བསྐྲུན་གྱི་ནུས་པ་དེ་སྒྲུབ་བྱེད་དང་མཉམ་དུ་མངོན་ཡོད།

འཐུས་གསརཔ་ཅིག་ བཏོན་གཏང་པའི་སྐབས་ འོག་གི་ སི་ཨེལ་ཨའི་ ཕོལོ་འདི་ལག་ལེན་འཐབ། གོམ་པ་རེ་རེ་དགོས།
གཞུང་སྐྱོང་མཚན་རྟགས་བཀོད་ནིའི་དོན་ལུ་ ཐོ་བཀོད་འབད་ཡོད་པའི་ སྒྲུབ་བྱེད་བསྡུ་སྒྲིག་ནང་ ས་ཆ།

1. **མངོན་གསལ་ JSON** འདི་ཨིན་ཀོཌ་འབད།
   ཕུལ་བ།

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **འཐུས་འདི་དཔར་བསྐྲུན་འབད།** ཡང་ན་ Norito པེ་ལོཌ་ (`--manifest`) ཡང་ན་
   the JSON འགྲེལ་བཤད་ (`--manifest-json`)། Torii/CLI འབྱོར་འཛིན་པ་ལཱསི་འདི་ཐོ་བཀོད་འབད།
   the `PublishSpaceDirectoryManifest` བཀོད་རྒྱ་ཧ་ཤི་:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SpaceDirectoryEvent བདེན་དཔང་བཟུང་།** ལུ་མངགས་ཉོ་འབད།
   `SpaceDirectoryEvent::ManifestActivated` དང་ 2019 ནང་བྱུང་ལས་ཀྱི་པེ་ལོཌི་འདི་ཚུདཔ་ཨིན།
   བཱན་ཌལ་འདི་ རྩིས་ཞིབ་པ་ཚུ་གིས་ བསྒྱུར་བཅོས་འདི་ ག་དེམ་ཅིག་ལྷོད་པའི་སྐབས་ ངེས་གཏན་བཟོ་ཚུགས།

4. **རྩིས་ཞིབ་བང་སྒྲིག་ཅིག་བཟོ་བཏོན་འབད་** གསལ་སྟོན་འདི་ དེ་གི་གནད་སྡུད་ས་སྟོང་གསལ་སྡུད་ལུ་བསྡམ་ཞིནམ་ལས་ དང་
   ཐག་རིང་ཚད་འཇལ་གྱི་ཧུཀ་:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` དང་ `manifests fetch`) བརྒྱུད་དེ་ བཱའིན་ཌིང་ཚུ་བདེན་དཔྱད་འབད།
   གོང་ལུ་ཡོད་པའི་ ཧེཤ་ + བཱན་ཌལ་དང་གཅིག་ཁར་ JSON ཡིག་སྣོད་དེ་ཚུ་ཡིག་མཛོད་འབད།

སྒྲུབ་བྱེད་ཞིབ་དཔྱད་ཐོ་ཡིག:

- [ ] བསྒྱུར་བཅོས་ཆ་འཇོག་འབད་མི་གིས་མིང་རྟགས་བཀོད་ཡོད་པའི་ མངོན་གསལ་ཧ་ཤི་ (`*.manifest.hash`)།
- [ ] དཔར་བསྐྲུན་འབོད་བརྡ་གི་དོན་ལུ་ CLI/Torii འབྱོར་འཛིན་ (stdout ཡང་ན་ `--json-out` བརྡ་རྟགས།)།
- [ ] `SpaceDirectoryEvent` འབབ་ཁུངས་བདེན་དཔང་བྱེད་པའི་ཤུགས་ལྡན་བཟོ་ནི།
- [ ] གནད་སྡུད་ས་སྟོང་གསལ་སྡུད་དང་ ཧུཀ་ དེ་ལས་ མངོན་གསལ་འདྲ་བཤུས་ཚུ་དང་གཅིག་ཁར་ རྩིས་ཞིབ་བང་རིམ་སྣོད་ཐོ།
- [ ] བཱའིན་ཌིང་ + མངོན་གསལ་གྱི་པར་རིས་ཚུ་ Torii ཤུགས་ལྡན་བཟོ་བའི་ཤུལ་ལས་ ལེན་ཡོདཔ་ཨིན།འདི་གིས་ ཨེསི་ཌི་ཀེ་ ༡ བྱིན་པའི་སྐབས་ `docs/space-directory.md` §3.2 ནང་དགོས་མཁོ་ཚུ་ མེ་ལོང་བཟོཝ་ཨིན།
ཇོ་བདག་ཚུ་གིས་ གསར་བཏོན་བསྐྱར་ཞིབ་འབད་བའི་སྐབས་ ཤོག་ལེབ་གཅིག་ལུ་ བརྡ་སྟོན་འབདཝ་ཨིན།

## 5. ཚད་འཛིན་/ལུང་ཕྱོགས་གསལ་སྟོན་ཊེམ་པེལེཊིསི།

ལྕོགས་གྲུབ་གསལ་སྟོན་ཚུ་བཟོ་བའི་སྐབས་ འགོ་བཙུགས་ས་ཚིགས་སྦེ་ ཨིན་-རི་པོ་སྒྲིག་ཆས་ཚུ་ལག་ལེན་འཐབ།
ཁྲིམས་ལུགས་འགོ་དཔོན་ཡང་ན་ ལུང་ཕྱོགས་ལྟ་རྟོག་པ་ཚུ་གི་དོན་ལུ་ཨིན། ཁོང་གིས་ ཁྱབ་ཁོངས་གནང་བ་/ངོས་ལེན་མ་འབད་ཐངས་སྟོནམ་ཨིན།
ལམ་ལུགས་དང་ བསྐྱར་ཞིབ་འབད་མི་ཚུ་གིས་ རེ་བ་བསྐྱེད་མི་ སྲིད་བྱུས་དྲན་ཐོ་ཚུ་ འགྲེལ་བཤད་རྐྱབ།

| ཕིགསི་ཊར་ | དམིགས་ཡུལ། | གཙོ་གནད། |
|---------|---------|-----------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB རྩིས་ཞིབ་ཕིཌ། | བཀག་འཛིན་པ་ UAIDs ཚུ་ ལཱ་འབད་མ་བཏུབ་སྦེ་བཞག་ནིའི་དོན་ལུ་ སིལ་ཚོང་སྤོ་བཤུད་ནང་ ངོས་ལེན་མ་འབད་བའི་ `compliance.audit::{stream_reports, request_snapshot}` གི་དོན་ལུ་ ལྷག་རྐྱངམ་ཅིག་གི་འཐུས་ཚུ། |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA ལྟ་རྟོག་ལམ། | ཚད་འཛིན་གཉིས་ལྡན་བཀག་དམ་འབད་ནི་ལུ་ ཚད་འཛིན་འབད་ཡོད་པའི་ `cbdc.supervision.issue_stop_order` འཐུས་ (ཉིནམ་རེ་སྒོ་སྒྲིག་ + `max_amount`) དང་ `force_liquidation` གུ་གསལ་ཏོག་ཏོ་བཀག་ཆ་ཁ་སྐོང་འབདཝ་ཨིན། |

འ་ནི་སྒྲིག་ཆས་ཚུ་རིགས་མཚུངས་བཟོ་བའི་སྐབས་དུས་མཐུན་བཟོ།

1. `uaid` དང་ `dataspace` ids ཚུ་ ཁྱོད་ཀྱིས་ལྕོགས་ཅན་བཟོ་བའི་བསྒང་ཡོད་མི་ བཅའ་མར་གཏོགས་མི་དང་ ལམ་ཚུ་མཐུན་སྒྲིག་འབད་ནིའི་དོན་ལུ་ཨིན།
༢ གཞུང་སྐྱོང་ལས་རིམ་ལུ་གཞི་བཞག་སྟེ་ `activation_epoch`/`expiry_epoch` སྒོ་སྒྲིག་ཚུ།
3. `notes` ས་སྒོ་ཚུ་ ཚད་འཛིན་གྱི་སྲིད་བྱུས་གཞི་བསྟུན་ཚུ་དང་གཅིག་ཁར་ (MiCA རྩོམ་ཡིག་ JFSA
   སྒོར་སྒོར་ ལ་སོགས་པ་ཚུ་)།
༤ ཆོག་ཐམ་སྒོ་སྒྲིག་ཚུ་ (ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༢༡༨ཨེགསི་, ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༢༡༩ཨེགསི་, ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༢༢༠ཨེགསི་) དང་གདམ་ཁ་ཅན་ཚུ།
   `max_amount` caps འབདཝ་ལས་ ཨེསི་ཌི་ཀེ་ཨེསི་ཚུ་གིས་ ཧོསིཊི་བཟུམ་སྦེ་ ཚད་འཛིན་ཚུ་ བསྟར་སྤྱོད་འབདཝ་ཨིན།

## 6. ཨེསི་ཌི་ཀེ་ ཉོ་སྤྱོད་པའི་དོན་ལུ་ གནས་སྤོ་དྲན་ཐོ།མངའ་ཁོངས་རེ་རེའི་རྩིས་ཐོ་ཨའི་ཌི་ཚུ་གཞི་བསྟུན་འབད་མི་ ད་ལྟོ་ཡོད་པའི་ཨེསི་ཌི་ཀེ་མཉམ་བསྡོམས་ཚུ་ ༡ ལུ་གནས་སྤོ་དགོཔ་ཨིན།
གོང་དུ་བརྗོད་པའི་ UAID-centric ཁ་ཐོག་ཚུ། ཡར་བསྐྱེད་འབད་བའི་སྐབས་ བརྟག་ཞིབ་ཐོ་ཡིག་འདི་ལག་ལེན་འཐབ།

  རྩིས་ཁྲའི་ ids. རསཊི་/ཇེ་ཨེསི་/སུའིཕཊི་/ཨེན་ཌོའིཌ་གི་དོན་ལུ་ འདི་གིས་ གསརཔ་ལུ་ཡར་འཕར་འབད་ནི་ཟེར་སླབ་ཨིན།
  ལཱ་གི་ས་སྒོ་ ཀེརེཊི་ཚུ་ ཡང་ན་ Norito བཱའིན་ཌིང་ཚུ་ བསྐྱར་བཟོ་འབད་དོ།
- **ཨེ་པི་ཨའི་འབོད་བརྡ་ཚུ་:** མངའ་ཁོངས་ཁྱབ་ཚད་ཡོད་པའི་ཡིག་ཆའི་འདྲི་དཔྱད་ཚུ་ ༡ དང་ཅིག་ཁར་ཚབ་བཙུགས།
  `GET /v1/accounts/{uaid}/portfolio` དང་ གསལ་སྟོན་/བསྡམ་ཐག་མཇུག་སྣོད་ཚུ།
  `GET /v1/accounts/{uaid}/portfolio` གིས་ གདམ་ཁ་ཅན་གྱི་ `asset_id` འདྲི་དཔྱད་ཅིག་དང་ལེན་འབདཝ་ཨིན།
  དངུལ་ཁུག་ཚུ་ལུ་ རྒྱུ་དངོས་དཔེ་ཚད་གཅིག་རྐྱངམ་ཅིག་དགོ་པའི་སྐབས་ཚད་བཟུང་། མཁོ་མངགས་འབད་མི་ཚུ་ དེ་བཟུམ་མའི་
  as `ToriiClient.getUaidPortfolio` (JS) དང་ཨེན་ཀྲོཌ་
  `SpaceDirectoryClient` གིས་ ཧེ་མ་ལས་རང་ ལམ་འདི་ཚུ་ བཀབ་ཡོདཔ་ཨིན། བེ་སི་པོག་ལས་ཁོང་ཚུ་ལུ་དགའ།
  ཨེཆ་ཊི་ཊི་པི་ཨང་རྟགས།
- **འདྲ་མཛོད་དང་ བརྡ་འཕྲིན་ཚད་འཇལ་:** འདྲ་མཛོད་ཐོ་བཀོད་ཚུ་ ཡུ་ཨེ་ཨའི་ཌི་ + གནད་སྡུད་ས་སྟོང་གི་ཚབ་ལུ་ སྔོ་མའི་ཚབ་ལུ་
  རྩིས་ཐོའི་ཨའི་ཌི་ཚུ་དང་ ཡུ་ཨེ་ཨའི་ཌི་ཚིག་ཡིག་སྟོན་མི་ ཊེ་ལི་མི་ཊར་བཏོན་གཏང་ནི་འདི་གིས་ བཀོལ་སྤྱོད་ཚུ་འབད་ཚུགས།
  བར་སྟོང་སྣོད་ཐོ་སྒྲུབ་བྱེད་དང་གཅིག་ཁར་ དྲན་ཐོ་ཚུ་གྲལ་ཐིག་བཟོ།
- **འཛོལ་བ་འཛིན་སྐྱོང་:** མཐའ་མཚམས་གསརཔ་ཚུ་གིས་ ཡུ་ཨེ་ཨའི་ཌི་དབྱེ་དཔྱད་འཛོལ་བ་དམ་དམ་ཚུ་སླར་ལོག་འབདཝ་ཨིན།
  `docs/source/torii/portfolio_api.md` ནང་ཡིག་ཆ་བཟོ་ཡོདཔ།; ཁ་ཐོག་དེ་ཚོའི་ཨང་རྟགས་ཚུ།
  verbatim དེ་འབདཝ་ལས་ རྒྱབ་སྐྱོར་སྡེ་ཚན་ཚུ་གིས་ གནད་དོན་ཚུ་ ལོག་སྟེ་རང་ གོམ་པ་མ་བཏང་པར་ དབྱེ་ཞིབ་འབད་ཚུགས།
- **བརྟག་དཔྱད:** གོང་དུ་བཤད་པའི་སྒྲིག་ཆས་ཚུ་ གློག་ཐག་བཏང་དགོ།
  བརྟག་དཔྱད་སྡེ་ཚན་ནང་ Norito སྐོར་འགྲུལ་དང་ གསལ་སྟོན་བརྟག་ཞིབ་ཚུ་ བདེན་དཔང་འབད་ནིའི་དོན་ལུ་
  ཧོསིཊི་ལག་ལེན་འཐབ་ཐངས་དང་མཐུན་སྒྲིག་འབད།

## 7. དཔྱད་གཞི།- `docs/space-directory.md` — མི་ཚེ་འཁོར་རིམ་གྱི་ཁ་གསལ་གཏིང་ཟབ་ཡོད་པའི་ བཀོལ་སྤྱོད་པའི་རྩེད་དེབ།
- `docs/source/torii/portfolio_api.md` — ཡུ་ཨེ་ཨའི་ཌི་ཡིག་ཆ་དང་
  manifest མཐའ་མཚམས་ཚུ།
- `crates/iroha_cli/src/space_directory.rs` — སི་ཨེལ་ཨའི་ ལག་ལེན་འཐབ་ཐངས་འདི་ ༢༠༢༠ ལུ་གཞི་བསྟུན་འབད་ཡོདཔ།
  ལམ་སྟོན་འདི།
- `fixtures/space_directory/capability/*.manifest.json` — ཚད་འཛིན་འབད་མི་ སིལ་ཚོང་དང་
  CBDC གསལ་སྟོན་ཊེམ་པེལེཊི་ཚུ་ རིགས་མཚུངས་བཟོ་བཅོས་འབད་ནི་ལུ་གྲ་སྒྲིག་ཡོདཔ་ཨིན།
