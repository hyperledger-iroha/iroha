<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus བརྒལ་གནད་སྡུད་ས་སྟོང་ ལོ་ཀཱལ་ནེཊ་བདེན་དཔང་།

རན་བུཀ་འདི་གིས་ Nexus མཉམ་བསྡོམས་བདེན་ཁུངས་འདི་ལག་ལེན་འཐབ་ཨིན།

- བཀག་ཆ་འབད་ཡོད་པའི་སྒེར་གྱི་གནད་སྡུད་ས་སྒོ་གཉིས་དང་གཅིག་ཁར་ ༤-པི་ཡར་ལོ་ཀཱལ་ནེཊི་ཅིག་བུཊི་འབདཝ་ཨིན་ (`ds1`, `ds2`),
- གནད་སྡུད་ས་སྒོ་རེ་རེ་ནང་ལུ་ རྩིས་ཐོའི་འགྲུལ་སྐྱོད་འདི་ལམ་བཟོཝ་ཨིན།
- གནད་སྡུད་ས་སྟོང་རེ་རེ་ནང་ རྒྱུ་དངོས་ཅིག་གསར་བསྐྲུན་འབདཝ་ཨིན།
- ཕྱོགས་གཉིས་ཆ་རའི་ནང་ གནད་སྡུད་ས་སྟོང་ཚུ་ནང་ རྡུལ་ཕྲན་བརྗེ་སོར་གཞི་ཆགས་འདི་ ལག་ལེན་འཐབ་ཨིན།
- མ་དངུལ་མ་ལང་པའི་རྐངམ་ཅིག་བཙུགས་ཏེ་ ལྷག་ལུས་ཚུ་ བསྒྱུར་བཅོས་མེད་པར་ བརྟག་དཔྱད་འབད་དེ་ ལོག་བལྟབ་ཀྱི་ ཡིག་བརྡ་ཚུ་ བདེན་ཁུངས་བསྐྱལཝ་ཨིན།

ཁྲིམས་མཐུན་བརྟག་དཔྱད་འདི་ནི།
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## མགྱོགས་རྒྱུགས།

མཛོད་ཁང་གི་རྩ་བ་ལས་ བཤུད་སྒྲིལ་ཡིག་ཚུགས་ལག་ལེན་འཐབ།

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

སྔོན་སྒྲིག་སྤྱོད་ལམ།

- གནད་སྡུད་བར་སྟོང་བདེན་དཔང་བརྟག་དཔྱད་རྐྱངམ་ཅིག་གཡོག་བཀོལཝ་ཨིན།
- ཆ་ཚན་ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༡༤ཨེགསི་,
- ཆ་ཚན་ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༡༥ཨེགསི་,
- ལག་ལེན་འཐབ་ཨིན་ `--test-threads=1`,
- བརྒྱུད་དེ་ `--nocapture`.

## ཕན་ཐོགས་ཅན་གྱི་གདམ་ཁ།

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` གིས་ གནས་སྐབས་ཀྱི་མཉམ་རོགས་སྣོད་ཐོ་ཚུ་ (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) ཁྲིམས་དཔྱད་ཀྱི་དོན་ལུ་བཞགཔ་ཨིན།
- `--all-nexus` གིས་ `mod nexus::` (Nexus མཉམ་བསྡོམས་ཆ་ཚན་ཆ་ཚང་) གཡོག་བཀོལཝ་ཨིན།

## CI སྒོ།

CI གྲོགས་རམ་པ་:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

དམིགས་གཏད་བཟོ:

```bash
make check-nexus-cross-dataspace
```

འ་ནི་སྒོ་ར་འདི་གིས་ གཏན་འབེབས་བདེན་ཁུངས་བཀལ་མི་འདི་ ལག་ལེན་འཐབ་ཨིནམ་དང་ གནད་སྡུད་ས་སྟོང་རྡུལ་ཕྲན་བརྒལ་བ་ཅིན་ ལཱ་འདི་འཐུས་ཤོར་བྱུང་པ་ཅིན་
swap གནས་སྟངས་འདི་ ལོག་འགྱོཝ་ཨིན།

## ལག་ཐོག་འདྲ་མཉམ་བརྡ་བཀོད་ཚུ།

དམིགས་གཏད་བདེན་དཔང་བརྟག་དཔྱད།

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

ཆ་ཚང་Nexusཡན་ལག་ཆ་ཚན་:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## རེ་བ་བསྐྱེད་པའི་བདེན་དཔང་བརྡ་རྟགས།- བརྟག་དཔྱད་འདི་མཐར་འཁྱོལ་ཡི།
- མ་དངུལ་མ་ལང་པའི་ གཞིས་ཆགས་རྐངམ་ལུ་ ཤེས་བཞིན་དུ་ འཐུས་ཤོར་བྱུང་མི་ལུ་ རེ་བ་བསྐྱེད་པའི་ཉེན་བརྡ་གཅིག་ཐོན་ནུག།
  `settlement leg requires 10000 but only ... is available`.
- མཐའ་མཇུག་གི་ལྷག་ལུས་བདེན་གཏམ་ཚུ་ ཤུལ་ལས་མཐར་འཁྱོལ་ཡོདཔ་ཨིན།
  - མཐར་འཁྱོལ་ཅན་གྱི་གདོང་ཕྱོགས་བརྗེ་སོར་,
  - མཐར་འཁྱོལ་ཅན་གྱི་རྒྱབ་འགལ་བརྗེ་སོར་,
  - མ་དངུལ་མ་ལང་པའི་བརྗེ་སོར་འཐུས་ཤོར་བྱུང་ཡོདཔ་ཨིན།

## ད་ལྟོའི་བདེན་དཔྱད་པར་རིས།

**སྤྱི་ལོ་༢༠༢༦ སྤྱི་ཟླ་༢ པའི་ཚེས་༡༩** ལུ་ ལཱ་གི་རྒྱུན་རིམ་འདི་ འདི་དང་གཅིག་ཁར་ འགྱོ་ཡོདཔ་ཨིན།

- དམིགས་གཏད་བརྟག་དཔྱད།: `1 passed; 0 failed`,
- ཆ་ཚང་ Nexus ཡན་ལག་ཆ་ཚན་: `24 passed; 0 failed`.