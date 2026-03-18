---
lang: dz
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## རྩིས་ཐོ་ཁ་བྱང་མཐུན་སྒྲིག་གནས་ཚད།(ADDR-2)

གནས་སྟངས་: ༢༠༢༦-༠༣-༣༠ བཀོད།  
ཇོ་བདག་: གནད་སྡུད་དཔེ་ཚད་སྡེ་ཚན་ / QA གིལཌི།  
ལམ་སྟོན་གྱི་གཞི་བསྟུན་: ADDR-2 — གཉིས་ལྡན་རྣམ་པ་མཐུན་སྒྲིག་སྒྲུབ་ཁང་།

### 1. སྤྱིར་བཏང་ལྟ་སྟངས།

- བཅོ་ཁ་: `fixtures/account/address_vectors.json` (I105 (pordered) + བསྡམ་བཞག་ (`sora`, གཉིས་པ་) + སྣ་མང་སིག་ པོ་སི་པོ་ཕེསི་/ནེ་གེ་ཊིབ་གནད་དོན་ཚུ་)།
- ཁྱབ་ཁོངས། འཛོལ་བ་ཆ་ཚང་ཡོད་པའི་ བརྡ་རྟགས་ཅན་གྱི་ V1 གིས་ བརྡ་སྟོན་གྱི་ བརྡ་སྟོན་དང་ ས་གནས་-༡༢ འཛམ་གླིང་ཐོ་བཀོད་དང་ སྣ་མང་ཚད་འཛིན་ཚུ་ཨིན།
- བཀྲམ་སྤེལ་: རསཊི་གནས་སྡུད་དཔེ་ཚད་ Torii, JS/TS, Swift, དང་ Android SDKs ཚུ་བརྒྱུད་དེ་ བརྗེ་སོར་འབད་ཡོདཔ་ཨིན། CI འདི་ ཉོ་སྤྱོད་པ་གང་རུང་གིས་ ཐ་དད་འབད་བ་ཅིན་ འཐུས་ཤོར་འབྱུང་འོང་།
- བདེན་པ་གི་འབྱུང་ཁུངས་: གློག་ཤུགས་འཕྲུལ་ཆས་འདི་ `crates/iroha_data_model/src/account/address/compliance_vectors.rs` ནང་ལུ་སྡོད་ཡོདཔ་དང་ `cargo xtask address-vectors` བརྒྱུད་དེ་ གསལ་སྟོན་འབད་ཡོདཔ་ཨིན།
### 2. བསྐྱར་གསོ་དང་བདེན་དཔང་།

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

སྐུགས་:

- `--out <path>` — ཨེ་ཌི་ཧོག་བཱན་ཌལ་ཚུ་བཟོ་བའི་སྐབས་ གདམ་ཁ་ཅན་གྱི་བཀག་ཆ་ (`fixtures/account/address_vectors.json` ལུ་སྔོན་སྒྲིག་)།
- `--stdout` — ཌིཀསི་ལུ་འབྲི་ནིའི་ཚབ་ལུ་ ཇེ་ཨེསི་ཨོ་ཨེན་འདི་ stdout ལུ་ བཏོན་གཏང་།
- `--verify` — ད་ལྟོའི་ཡིག་སྣོད་འདི་ གསརཔ་བཟོ་ཡོད་པའི་ནང་དོན་ལུ་རྒྱབ་འགལ་འབད་ (ཌིརཕཊི་གུ་མགྱོགས་དྲགས་སྦེ་འཐུས་ཤོར་; `--stdout` དང་ཅིག་ཁར་ལག་ལེན་འཐབ་མི་བཏུབ།)

### 3. ཅ་རྙིང་མེ་ཊིགསི།

| ཁ་ཐོག་ | བསྟར་སྤྱོད་ | དྲན་ཐོ། |
|----------------------------------------- |
| རསཊི་གནས་སྡུད་-དཔེ་ཚད་ | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON དང་ ཀེན་ནོ་ནིག་པེ་ལོཌི་ཚུ་ བསྐྱར་བཟོ་འབད་ཞིནམ་ལས་ I105 (དགའ་གདམ་)/བསྡོམས་རྩིས་ (`sora`, གཉིས་པ་)/ཁྲིམས་ལུགས་གཞི་བསྒྱུར་ + གཞི་བཀོད་འཛོལ་བ་ཚུ་ ཞིབ་དཔྱད་འབདཝ་ཨིན། |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | སར་བར་གྱི་ཕྱོགས་ཀྱི་གསང་ཡིག་ཚུ་ བདེན་དཔྱད་འབདཝ་ཨིན། Torii གིས་ I105 (དགའ་གདམ་)/བསྡམ་བཞག་མི་ (`sora`, གཉིས་པ་) གིས་ གཏན་འབེབས་བཟོ་ཐོག་ལས་ སྐྱོན་བརྗོད་འབདཝ་ཨིན། |
| ཇ་བ་ཨིསི་ཀིརིཔཊི་ཨེསི་ཌི་ཀེ་ | `javascript/iroha_js/test/address.test.js` | མེ་ལོང་ཝི་༡ གི་སྒྲིག་བཀོད་ཚུ་ (I105 གིས་ དགའ་གདམ་/བསྡམ་བཞག་འབད་ཡོདཔ་ (`sora`) གཉིས་པ་/ཆ་ཚང་/ཆ་ཚང་) དེ་ལས་ Norito-style sytle sytle sytle sytle syme- style fronor code ཚུ་ ངན་ལྷད་ཅན་ཚུ་ཚུ་གི་དོན་ལུ་ཨིན། |
| Swift ཨེསི་ཌི་ཀེ་ | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | I105 (དགའ་གདམ་)/བསྡམ་བཞག་མི་ (`sora`, དྲག་ཤོས་གཉིས་པ་) ཌི་ཀོ་ཌིང་དང་ སྣ་མང་སིག་པེ་ལོཌི་ དེ་ལས་ ཨེ་པཱལ་སྟེགས་བུ་ཚུ་ནང་ འཛོལ་བ་ཚུ་ བརྒལ་ཚུགསཔ་ཨིན། |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | ཀོཊ་ལིན་/ཇ་བ་བཱའིན་ཌིང་ཚུ་ ཀེ་ནོ་ནིག་གཏན་འཇགས་དང་གཅིག་ཁར་ ཕྲང་སྒྲིག་སྦེ་སྡོད་དགོཔ་ངེས་གཏན་བཟོཝ་ཨིན། |

### 4. བལྟ་རྟོག་དང་ལས་འདས་པའི་ལས་ཀ།- གནས་ཚད་སྙན་ཞུ། ཡིག་ཆ་འདི་ `status.md` ལས་འབྲེལ་མཐུད་འབད་ཡོདཔ་དང་ བདུན་ཕྲག་རེ་ནང་བསྐྱར་ཞིབ་ཚུ་གིས་ བརྟན་བཞུགས་གསོ་བའི་བདེན་དཔྱད་འབད་ཚུགས།
- གོང་འཕེལ་གཏང་མི་དྲྭ་ཚིགས་བཅུད་བསྡུས་: བལྟ་ **གཞི་བསྟུན་ → རྩིས་ཐོ་གི་ཁ་བྱང་མཐུན་སྒྲིག་** ཡིག་ཆ་དྲྭ་ཚིགས་ (`docs/portal/docs/reference/account-address-status.md`) ནང་ ཕྱི་ཁའི་-གདོང་ཕྱོགས་ཀྱི་ མཉམ་བསྡོམས་དོན་ལུ་ཨིན།
- Prometheus དང་ dashboards: ཁྱོད་ཀྱིས་ SDK འདྲ་བཤུས་ཅིག་བདེན་དཔྱད་འབད་བའི་སྐབས་ `--metrics-out` གིས་ གྲོགས་རམ་པ་གཡོག་བཀོལ་བའི་སྐབས་ (དེ་ལས་ `--metrics-label`) དེ་འབདཝ་ལས་ Prometheus ཚིག་ཡིག་བསྡུ་གསོག་འདི་གིས་ `account_address_fixture_check_status{target=…}`. Grafana dashboard **Account ཁ་བྱང་ ཕིག་ཅར་གནས་སྟངས་** (`dashboards/grafana/account_address_fixture_status.json`) ཁ་ཐོག་རེ་ལུ་ ཆོག་ཐམ་/འཐུས་ཤོར་གྱི་གྱངས་ཁ་ཚུ་ བཀྲམ་སྟོན་འབདཝ་ཨིནམ་དང་ ཁ་ཐོག་ལུ་ ཨེསི་ཨེཆ་ཨེ་-༢༥༦ འདི་ རྩིས་ཞིབ་སྒྲུབ་བྱེད་ཀྱི་ འཇལཝ་ཨིན། དམིགས་གཏད་སྙན་ཞུ་ `0` གང་རུང་ལུ་ཉེན་བརྡ།
- Prometheus metrics: `torii_address_domain_total{endpoint,domain_kind}` གིས་ ད་ལྟོ་ མཐར་འཁྱོལ་ཅན་སྦེ་ རྩིས་ཐོ་ཡིག་ཐོག་བཀོད་ཡོད་པའི་ རྩིས་ཁྲའི་ཡིག་ཆའི་ རེ་རེ་ལུ་ བཤུབ་བཏང་སྟེ་ `torii_address_invalid_total`/`torii_address_local8_total`. བཟོ་སྐྲུན་དང་མེ་ལོང་གང་རུང་ལུ་ ཉེན་བརྡ་འབད་མི་ལུ་བརྟེན་ ས་གནས་༡༢ པའི་དགོངས་ཞུའི་སྒོ་ར་ལུ་ རྩིས་ཞིབ་འབད་ཚུགས་པའི་ སྒྲུབ་བྱེད་ཚུ་ཡོདཔ་ཨིན།
- གཏན་བཟོས་གྲོགས་རམ་པ་: `scripts/account_fixture_helper.py` ཕབ་ལེན་ཡང་ན་ ཀེ་ནོ་ནིག་ཇེ་ཨེསི་ཨོ་ཨེན་ བདེན་བཤད་འབད་དེ་ ཨེསི་ཌི་ཀེ་ གསར་བཏོན་རང་བཞིན་གྱིས་ ལག་ཐོག་འདྲ་བཤུས་/སྦྱར་མ་མེད་པར་ འཐོབ་ཚུགསཔ་/ཞིབ་དཔྱད་འབད་ཚུགསཔ་ཨིན་ གདམ་ཁ་ཅན་གྱི་ཐོག་ལས་ Prometheus མེཊིཀ་ཚུ་ འབྲི་ཚུགས། དཔེ:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  གྲོགས་རམ་པ་འདི་གིས་ དམིགས་གཏད་འདི་མཐུན་སྒྲིག་འབད་བའི་སྐབས་ `account_address_fixture_check_status{target="android"} 1` བྲིསཝ་ཨིན་ དེ་ལས་ `account_address_fixture_remote_info` / Prometheus གི་འཇལ་ཚད་ཚུ་ SHA-256 འཇུ་བྱེད་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན། ཡིག་སྣོད་བརླག་སྟོར་ཞུགས་མི་སྙན་ཞུ་ `account_address_fixture_local_missing`.
  རང་འགུལ་གྱི་ wrapper: cron/CI ལས་ `ci/account_fixture_metrics.sh` ལུ་ མཉམ་སྡེབ་འབད་ཡོད་པའི་ཚིག་ཡིག་ཡིག་སྣོད་ (སྔོན་སྒྲིག་ Prometheus) ལུ་ འབོཝ་ཨིན། `--target label=path` ཐོ་བཀོད་ཚུ་ (འབྱུང་ཁུངས་འདི་བཀག་ཆ་འབད་ནིའི་དོན་ལུ་ དམིགས་གཏད་རེ་ལུ་ གདམ་ཁ་ཅན་སྦེ་ `::https://mirror/...` ཁ་སྐོང་བརྐྱབ་ཨིན། GitHub ལཱ་གི་རྒྱུན་རིམ་ `address-vectors-verify.yml` གིས་ ཧེ་མ་ལས་རང་ གྲོགས་རམ་པ་འདི་ ཀེན་ནོ་ནིག་སྒྲིག་ཆས་ལུ་ གཡོག་བཀོལཝ་ཨིནམ་དང་ ཨེསི་ཨར་ཨི་ བཙུགས་ནིའི་དོན་ལུ་ `account-address-fixture-metrics` ཅ་ཆས་ཚུ་ སྐྱེལ་བཙུགས་འབདཝ་ཨིན།