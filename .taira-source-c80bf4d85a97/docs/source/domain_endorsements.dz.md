---
lang: dz
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# མངའ་ཁོངས་ཀྱི་ཆ་རྐྱེན།

མངའ་ཁོངས་ཀྱི་རྒྱབ་སྐྱོར་ཚུ་གིས་ བཀོལ་སྤྱོད་པ་ཚུ་ལུ་ མངའ་ཁོངས་གསར་བསྐྲུན་འབད་དེ་ ཚོགས་ཆུང་གི་མིང་རྟགས་བཀོད་ཡོད་པའི་གསལ་བཤད་ཀྱི་འོག་ལུ་ ལོག་སྟེ་ལག་ལེན་འཐབ་བཅུགཔ་ཨིན། ངོས་ལེན་གྱི་ པེ་ལོཌ་འདི་ Norito དངོས་པོ་འདི་ རིམ་སྒྲིག་ནང་ལུ་ཐོ་བཀོད་འབད་དེ་ཡོདཔ་ལས་ མཁོ་མངགས་འབད་མི་ཚུ་གིས་ མངའ་ཁོངས་ག་འདི་དང་ ནམ་ལུ་ བརྟག་དཔྱད་འབད་ཚུགས།

## གླ་ཆ་སྤྲོད་པའི་དབྱིབས་།

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: ཀེ་ནོ་ནིག་མངའ་ཁོངས་ངོས་འཛིན་
- `committee_id`: མི་ཡི་ལྷག་ཐུབ་པའི་ཚོགས་ཆུང་ཁ་ཡིག་།
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: སྡེབ་ཚན་གྱི་མཐོ་ཚད་ཀྱི་ཚད་གཞི།
- ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```: གདམ་ཁ་ཅན་གྱི་གནད་སྡུད་ས་སྟོང་དང་ གདམ་ཁའི་ `[block_start, block_end]` སྒོ་སྒྲིག་ (གྲངས་སུ་བཙུགས་) དེ་ **དགོཔ་** གིས་ ངོས་ལེན་འབད་མི་ སྡེབ་ཚན་མཐོ་ཚད་འདི་ གཡོག་བཀོལ་དགོ།
- `signatures`: `body_hash()` གུ་མིང་རྟགས་བཀོད་ཡོད།
- `metadata`: གདམ་ཁའི་ Norito མེ་ཊ་ཌེ་ཊ་ (གྲོས་འཆར་ཨའི་ཌི་, རྩིས་ཞིབ་འབྲེལ་ལམ་ ལ་སོགས་པ་ཚུ་)།

## སྲུང་སྐྱོབ།

- Nexus འདི་ལྕོགས་ཅན་བཟོ་ཞིནམ་ལས་ `nexus.endorsement.quorum > 0`, ཡང་ན་ མངའ་ཁོངས་རེ་ལུ་དགོས་མཁོ་ཡོད་པའི་མངའ་ཁོངས་འདི་རྟགས་བཀལཝ་ད་ ངོས་ལེན་ཚུ་དགོཔ་ཨིན།
- བདེན་དཔྱད་ཀྱིས་ མངའ་ཁོངས་/བརྡ་བཀོད་ ཧེསི་བཱའིན་ཌིང་ ཐོན་རིམ་ སྡེབ་ཚན་སྒོ་སྒྲིག་ གནད་སྡུད་ཀྱི་འཐུས་མི། དུས་ཡུན་ཚང་མི་/ལོ་ཚད་ དེ་ལས་ ཚོགས་ཆུང་གི་ བསྡོམས་རྩིས་ཚུ་ བརྟན་བཞུགས་འབདཝ་ཨིན། མཚན་རྟགས་བཀོད་མི་ཚུ་ལུ་ `Endorsement` གི་འགན་ཁུར་དང་གཅིག་ཁར་ མོས་མཐུན་གྱི་ལྡེ་མིག་ཚུ་དགོཔ་ཨིན། བསྐྱར་རྩེད་ཚུ་ `body_hash` གིས་ ངོས་ལེན་མི་འབད།
- མངའ་ཁོངས་ཐོ་བཀོད་ལུ་མཉམ་སྦྲགས་འབད་ཡོད་པའི་ མཐའ་མཚམས་ཚུ་ མེ་ཊ་ཌེ་ཊ་ལྡེ་མིག་ `endorsement` ལག་ལེན་འཐབ་ཨིན། བདེན་དཔྱད་འགྲུལ་ལམ་འདི་ `SubmitDomainEndorsement` བཀོད་རྒྱ་གིས་ལག་ལེན་འཐབ་ཨིན་ དེ་ཡང་ མངའ་ཁོངས་གསརཔ་ཅིག་ཐོ་བཀོད་མ་འབད་བར་ རྩིས་ཞིབ་འབད་ནིའི་དོན་ལུ་ ངོས་ལེན་ཚུ་ཐོ་བཀོད་འབདཝ་ཨིན།

## ཚོགས་ཆུང་དང་སྲིད་བྱུས།

- ཚོགས་ཆུང་ཚུ་ རིམ་སྒྲིག་ (`RegisterDomainCommittee`) ཡང་ན་ རིམ་སྒྲིག་སྔོན་སྒྲིག་ལས་ འཐོབ་ཚུགས་པའི་ (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`) ལུ་ཐོ་བཀོད་འབད་ཚུགས།
- མངའ་ཁོངས་ཀྱི་སྲིད་བྱུས་ཚུ་ `SetDomainEndorsementPolicy` བརྒྱུད་དེ་ རིམ་སྒྲིག་འབད་ཡོདཔ་ཨིན། མེད་པའི་སྐབས་ Nexus སྔོན་སྒྲིག་ཚུ་ལག་ལེན་འཐབ་ཨིན།

## CLI རོགས་རམ་པ།

- ངོས་ལེན་ཅིག་བཟོ་བསྐྲུན་/མིང་རྟགས་བཀོད། (ཐོན་འབྲས་ Norito JSON stdout ལུ་):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- ངོས་ལེན་ཅིག་ཕུལ་ནི།

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- གཞུང་སྐྱོང་འཛིན་སྐྱོང་།
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

བདེན་དཔྱད་འཐུས་ཤོར་ཚུ་ སླར་ལོག་གཏན་ཏོག་ཏོ་འཛོལ་བ་ཡིག་རྒྱུན་ཚུ་ (quorum མ་མཐུནམ་དང་ stale/pared netration, ཁྱབ་ཚད་མ་མཐུན་པའི་ མ་ཤེས་པའི་གནད་སྡུད་ས་སྟོང་ མ་ཆད་པའི་ཚོགས་ཆུང་)།