<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi ལུགས་མཐུན་དཔེ་ཚད་ (TLA+ / ཨ་པ་ལ་ཆི)

སྣོད་ཐོ་འདི་ནང་ Sumeragi ཁས་ལེན་འགྲུལ་ལམ་ཉེན་སྲུང་དང་ འཚོ་བའི་དོན་ལུ་ ཚད་འཛིན་འབད་ཡོད་པའི་ལུགས་མཐུན་དཔེ་ཚད་ཅིག་ཡོདཔ་ཨིན།

## ཁྱབ་ཁོངས།

དཔེ་ཚད་འདི་གིས་ འཛིན་བཟུང་འབདཝ་ཨིན།
- དུས་རིམ་འཕེལ་རིམ་ (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ཚོགས་རྒྱན་དང་ཚོགས་གྲངས་ཚད་གཞི་ (`CommitQuorum`, `ViewQuorum`),
- ལྗིད་ཚད་ཅན་གྱི་བགོ་བཤའ་ཚད་གཞི་ (`StakeQuorum`) ཨེན་པི་ཨོ་ཨེསི་བཟོ་རྣམ་གྱི་ཁས་བླངས་སྲུང་སྐྱོབ་ཚུ་གི་དོན་ལུ་,
- མགོ་ཡིག་/བཞུ་བའི་སྒྲུབ་བྱེད་དང་གཅིག་ཁར་ RBC རྒྱུ་རྐྱེན་ (`Init -> Chunk -> Ready -> Deliver`) དང་།
- དྲང་བདེན་གྱི་ཡར་རྒྱས་ཀྱི་བྱ་སྤྱོད་ཚུ་ལས་ ཇི་ཨེསི་ཊི་དང་ དྲང་བདེན་གྱི་ བསམ་ཚུལ་ཞན་ཁོག་ཚུ།

འདི་གིས་ ཤེས་བཞིན་དུ་ ཝའིར་རྩ་སྒྲིག་དང་ མིང་རྟགས་ དེ་ལས་ ཡོངས་འབྲེལ་གྱི་ཁ་གསལ་ཆ་ཚང་ཚུ་ བཅུད་བསྡུཝ་ཨིན།

## ཡིག་སྣོད་ཚུ།

- `Sumeragi.tla`: མཐུན་སྒྲིག་དཔེ་ཚད་དང་རྒྱུ་དངོས་ཚུ།
- `Sumeragi_fast.cfg`: CI-མཐུན་འབྲེལ་ཚད་གཞི་ཆ་ཚན་ཆུང་བ།
- `Sumeragi_deep.cfg`: གནོན་ཤུགས་ཚད་བཟུང་ཆ་ཚན་སྦོམ་ཡོདཔ།

## རྒྱུ་དངོས་ཚུ།

འགྱུར་ལྡོག་མེད་མི་ཚུ་:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

དུས་སྐབས་ཀྱི་རྒྱུ་དངོས།
- `EventuallyCommit` (`[] (gst => <> committed)`), ཇི་ཨེསི་ཊི་རྗེས་མའི་དྲང་བདེན་ཨེན་ཀོ་ཌི་འབད་ཡོདཔ།
  ལག་ལེན་འཐབ་ཐོག་ལས་ `Next` ནང་ལུ་ (དུས་ཚོད་རྫོགས་/འཛོལ་བ་སྔོན་འགོག་སྲུང་སྐྱོབ་ཚུ་ ལྕོགས་ཅན་བཟོ་ཡོདཔ།
  ཡར་རྒྱས་ཀྱི་བྱ་བ་ཚུ་)། འདི་གིས་ དཔེ་ཚད་འདི་ ཨ་པ་ལ་ཆི་ ༠.༥༢.ཨེགསི་དང་གཅིག་ཁར་ བརྟག་ཞིབ་འབད་ཚུགསཔ་སྦེ་བཞགཔ་ཨིན།
  བརྟག་ཞིབ་འབད་ཡོད་པའི་དུས་སྐབས་རྒྱུ་དངོས་ཚུ་གི་ནང་འཁོད་ལུ་ `WF_` དྲང་བདེན་བཀོལ་སྤྱོད་པ་ཚུ་ལུ་རྒྱབ་སྐྱོར་མི་འབད།

## རྒྱུག་དོ།

མཛོད་ཁང་གི་རྩ་བ་ལས་:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### བསྐྱར་བཟོ་འབད་བཏུབ་པའི་ཉེ་གནས་གཞི་སྒྲིག་ (Docker དགོས་མཁོ་མེདཔ་)མཛོད་ཁང་འདི་གིས་ལག་ལེན་འཐབ་མི་ པིན་འབད་ཡོད་པའི་ཉེ་གནས་ཨ་པ་ལ་ཆི་ལག་ཆས་རྒྱུན་རིམ་འདི་གཞི་བཙུགས་འབད།

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

རྒྱུག་མི་གིས་ གཞི་བཙུགས་འདི་ རང་བཞིན་གྱིས་ ཤེས་རྟོགས་འབདཝ་ཨིན།
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
གཞི་བཙུགས་འབད་བའི་ཤུལ་ལས་ `ci/check_sumeragi_formal.sh` འདི་ env vars ཁ་སྐོང་མེད་པར་ལཱ་འབད་དགོ།

```bash
bash ci/check_sumeragi_formal.sh
```

གལ་སྲིད་ཨ་པ་ལ་ཆི་ `PATH` ནང་མེད་པ་ཅིན་ ཁྱོད་ཀྱིས་འབད་ཚུགས།

- བཀོལ་སྤྱོད་འབད་བཏུབ་པའི་འགྲུལ་ལམ་ལུ་ `APALACHE_BIN` གཞི་སྒྲིག་འབད་ ཡང་ན་
- Docker ཕོལ་བེཀ་འདི་ལག་ལེན་འཐབ།
  - པར་རིས་: ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༣༦ཨེགསི་ (སྔོན་སྒྲིག་ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༣༧ཨེགསི་)
  - གཡོག་བཀོལ་བའི་ Docker ཌེ་མཱོན་ཅིག་དགོཔ་ཨིན།
  - `APALACHE_ALLOW_DOCKER=0` དང་ཅིག་ཁར་ ཕོལབེཀ་ལྕོགས་མིན་བཟོ།

དཔེར་ན།

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## མཆན་འགྲེལ།

- དཔེ་ཚད་འདི་གིས་ 2019 ནང་ ལག་ལེན་འཐབ་བཏུབ་པའི་ རསཊི་དཔེ་ཚད་བརྟག་དཔྱད་ཚུ་ མཐུན་སྒྲིག་འབདཝ་ཨིན།
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  དང་ །
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ཞིབ་དཔྱད་ཚུ་ `.cfg` ཡིག་སྣོད་ཚུ་ནང་ དུས་རྒྱུན་གནས་གོང་ཚུ་གིས་ ཚད་འཛིན་འབད་ཡོདཔ་ཨིན།
- PR CI གིས་ འ་ནི་ཞིབ་དཔྱད་ཚུ་ `.github/workflows/pr.yml` བརྒྱུད་དེ་ གཡོག་བཀོལཝ་ཨིན།
  `ci/check_sumeragi_formal.sh`.