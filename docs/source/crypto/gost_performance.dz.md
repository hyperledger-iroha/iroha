---
lang: dz
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ལས་བྱེད་པའི་ལས་རིམ།

དྲན་འཛིན་འདི་གིས་ ང་བཅས་ཀྱིས་ ལཱ་འགན་ཡིག་ཤུབས་འདི་ ག་དེ་སྦེ་ བརྟག་ཞིབ་འབདཝ་ཨིན་ན་དང་ བསྟར་སྤྱོད་འབད་དོ་ཡོདཔ་ཨིན་ན་ ཡིག་ཆ་བཟོཝ་ཨིན།
TC26 GOST མཚན་རྟགས་བཀོད་པའི་རྒྱབ་ལྗོངས།

## ས་གནས་ནང་རྒྱུག་ནི།

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

མཐོང་སྣང་གི་རྒྱབ་ལུ་ དམིགས་གཏད་གཉིས་ཆ་ར་གིས་ `scripts/gost_bench.sh` ཟེར་སླབ་ཨིན།

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` བཀོལ་སྤྱོད་བྱེད།
2. `gost_perf_check` `target/criterion` ལུ་འགོག་ཐབས་འབད་དེ་ བདེན་དཔྱད་ཀྱི་བར་མཚམས་ཚུ་ བདེན་དཔྱད་འབདཝ་ཨིན།
   བརྟག་དཔྱད་འབད་ཡོད་པའི་གཞི་རྟེན་ (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
༣ འཐོབ་ཚུགས་པའི་སྐབས་ `$GITHUB_STEP_SUMMARY` ནང་ལུ་ རྟགས་བཀོད་ཀྱི་བཅུད་བསྡུས་འདི་བཙུགས་དོ་ཡོདཔ་ཨིན།

ལོག་རི་/ཡར་རྒྱས་ཅིག་ཆ་འཇོག་འབད་བའི་ཤུལ་ལས་ གཞི་རྟེན་ཐིག་འདི་གསར་བསྐྲུན་འབད་ནི་ལུ་ གཡོག་བཀོལ།

```bash
make gost-bench-update
```

ཡང་ན་ཐད་ཀར་ནི།

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` གིས་ བེན་ཇི་ + ཞིབ་དཔྱད་པ་ གཡོག་བཀོལ་ཏེ་ གཞི་རྟེན་ཇེ་ཨེསི་ཨོ་ཨེན་ དང་ དཔར་བསྐྲུན་ཚུ་ ཚབ་སྲུང་འབདཝ་ཨིན།
བར་མཚམས་གསརཔ། ༢༠༢༠ ནང་གྲོས་ཐག་དྲན་ཐོ་གི་མཉམ་དུ་དུས་མཐུན་བཟོ་ཡོད་པའི་ཇེ་ཨེསི་ཨོ་ཨེན་འདི་ཨ་རྟག་ར་ཁས་བླངས་འབད།
`crates/iroha_crypto/docs/gost_backend.md`.

### ད་ལྟའི་གཞི་བསྟུན་བརྡ་ཆར།

| ཨཱལ་གོ་རི་དམ་ | བར་མཚམས་ (μs) |
|------------------------------------------------ |
| ed25519 | ༦༩.༦༧ |
| sost256_paramset_a | ༡༡༣༦.༩༦ |
| sost256_paramset_b | 1129.05 |
| sost256_paramset_c | ༡༡༣༣.༢༥ |
| sost512_paramset_a | ༨༩༤༤.༣༩ |
| sost512_paramset_b | ༨༩༦༣.༦༠ |
| སེཔ་༢༥༦ཀེ་༡ | ༡༦༠.༥༣ |

## CI

`.github/workflows/gost-perf.yml` གིས་ ཡིག་གཟུགས་གཅིགཔོ་འདི་ལག་ལེན་འཐབ་ཨིནམ་དང་ ཌུ་ཌེཀ་དུས་ཚོད་སྲུང་སྐྱོབ་ཡང་གཡོག་བཀོལཝ་ཨིན།
CI འདི་ རིམ་སྒྲིག་འབད་ཡོད་པའི་ བཟོད་བསྲན་ལས་ ཚད་འཇལ་བའི་བར་མཚམས་ལས་ལྷག་སྟེ་ཡོད་པའི་སྐབས་ལུ་ འཐུས་ཤོར་འགྱོཝ་ཨིན།
(སྔོན་སྒྲིག་ལས་ ༢༠%) ཡང་ན་ དུས་ཚོད་ཀྱི་སྲུང་མི་གིས་ ཆུ་བཤལ་ཅིག་ ཤེས་རྟོགས་འབད་བའི་སྐབས་ རང་བཞིན་གྱིས་ བསྐྱར་ལོག་ཚུ་ འཛིན་བཟུང་འབདཝ་ཨིན།

## བཅུད་དོན་ཨའུཊི་པུཊི་།

`gost_perf_check` ག་བསྡུར་ཐིག་ཁྲམ་འདི་ཉེ་གནས་ལུ་དཔར་བསྐྲུན་འབདཝ་ཨིནམ་དང་ ནང་དོན་གཅིག་པ་ ལུ་ མཉམ་སྦྲགས་འབདཝ་ཨིན།
`$GITHUB_STEP_SUMMARY`, དེ་འབདཝ་ལས་ CI ལཱ་གི་དྲན་ཐོ་ཚུ་དང་ གཡོག་བཀོལ་བའི་ བཅུད་བསྡུས་ཚུ་ ཨང་གྲངས་གཅིག་མཚུངས་ཚུ་ བརྗེ་སོར་འབདཝ་ཨིན།