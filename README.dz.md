---
lang: dz
direction: ltr
source: README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:30:39.016220+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# I18NT0000000X Iroha

[![[ཆོག་ཐམ་](I18NU0000032X)](https://opensource.org/licenses/Apache-2.0)

I18NT0000001X Iroha འདི་ གནང་བ་དང་ ཀོན་ཊི་ཡམ་བཀྲམ་སྤེལ་ཚུ་གི་དོན་ལུ་ གཏན་འབེབས་བཀག་ཆ་ཡོད་པའི་ གཞི་རྟེན་ཅིག་ཨིན། འདི་གིས་ རྩིས་ཁྲ་/རྒྱུ་དངོས་འཛིན་སྐྱོང་དང་ རིམ་སྒྲིག་གནང་བ་ དེ་ལས་ Iroha བར་ཅུ་ཡལ་འཕྲུལ་ཆས་ (I18NT000000023X) བརྒྱུད་དེ་ གན་རྒྱ་ཚུ་བྱིནམ་ཨིན།

> ལཱ་གི་ས་སྒོ་དང་ འཕྲལ་གྱི་བསྒྱུར་བཅོས་ཚུ་ [`status.md`](./status.md) ནང་ལུ་ བརྟག་ཞིབ་འབད་ཡོདཔ་ཨིན།

## རྗེས་འཇུག་རྗེས་འཇུག་།

མཛོད་ཁང་འདི་གིས་ གསང་ཡིག་གཞི་རྟེན་གཅིག་ལས་ བཀྲམ་སྤེལ་ལམ་གཉིས་ བཏངམ་ཨིན།

- **I18NT0000015X 2**:: རང་གིས་གནང་བའི་གནང་བ་/མཉམ་བསྡོམས་དྲ་ཚིགས།
- **Iroha 3 (SORA Nexus)***: Nexus-oriented repenting repeeding dounteding repeed ed dee of ཀེརེཊི་འདི་ལག་ལེན་འཐབ་ཨིན།

ལམ་གཉིས་ཆ་ར་གིས་ Norito རིམ་སྒྲིག་ Sumeragi མོས་མཐུན་དང་ I18NT0000004X -> I18NT000000024X ལག་ཆས་ཚུ་ཚུ་རྩིས་ཏེ་ གཙོ་བོ་ཆ་ཤས་ཅོག་འཐདཔ་ཚུ་ བརྗེ་སོར་འབདཝ་ཨིན།

## མཛོད་ཁང་གི་སྒྲིག་བསྒྲགས།

- [I18NI0000000078X](./crates): རསཊ་ཀེརེསི་གཙོ་བོ་ (`iroha`, I18NI00000000081X, I18NI0000000081X, I18NI00000000082X, I18NI000000000083X, I18NI0000000083X, I18NI000000083X, `norito` ལ་སོགས་པ།).
- [I18NI0000085X](I18NU000000035X): ཆ་ཤས་-ཆ་ཤས་ཡོངས་འབྲེལ་/མཉམ་བསྡོམས་བརྟག་དཔྱད།
- [I18NI0000086X](./IrohaSwift): སོར་ཆུད་ཨེསི་ཌི་ཀེ་ཐུམ་སྒྲིལ་།
- [I18NI0000087X](I18NU000000037X): Android SDK ཐུམ་སྒྲིལ་།
- [I18NI0000088X](I18NU000000038X): ལག་ལེན་པ་/བཀོལ་སྤྱོད་པ་/གོང་འཕེལ་གཏང་མི་ཡིག་ཆ།

## མགྱོགས་འགོ་བཙུགས་པ།

### སྔོན་འགྲོའི་ཆ་རྐྱེན།

- [རྒྱུས་གྲངས།](I18NU0000039X)
- གདམ་ཁ་ཅན་: I18NT000000009X + I18NT000000010X ཉེ་གནས་མལ་ཊི་པི་ཡར་གཡོག་བཀོལ་ཚུ་གི་དོན་ལུ་ བརྩམས་སྒྲིག།

### བཟོ་སྐྲུན་དང་བརྟག་དཔྱད།(ལས་ཀ)།

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

དྲན་ཐོ།

- ལཱ་གི་ས་སྒོ་ཆ་ཚང་བཟོ་ནི་ལུ་ སྐར་མ་༢༠ དེ་ཅིག་འགོརཝ་ཨིན།
- ལཱ་གི་ས་སྒོ་བརྟག་དཔྱད་ཆ་ཚང་གིས་ ཆུ་ཚོད་ལེ་ཤ་ཅིག་འགོར་འོང་།
- ལཱ་གི་ས་སྒོ་དམིགས་གཏད་ཚུ་ I18NI000000089X (WASM/no-std བཟོ་བསྐྲུན་ཚུ་རྒྱབ་སྐྱོར་མ་འབད་བས།)

### དམིགས་ཚད་ཅན་གྱི་ཚོད་ལྟའི་བརྡ་ཆ།

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### ཨེསི་ཌི་ཀེ་བརྟག་དཔྱད་བརྡ་བཀོད།

I18NF0000028X

I18NF0000029X

## ས་གནས་དྲ་རྒྱ་གཡོག་བཀོལ།

བྱིན་འགོ་བཙུགས། Docker རྩོམ་སྒྲིག་དྲ་བ།

```bash
docker compose -f defaults/docker-compose.yml up
```

སྔོན་སྒྲིག་མཁོ་སྤྲོད་རིམ་སྒྲིག་འབད་མི་ལུ་ སི་ཨེལ་ཨའི་ལག་ལེན་འཐབ།

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

ཌེ་མཱོན་དམིགས་བསལ་གྱི་ བཀྲམ་སྤེལ་གྱི་གོ་རིམ་ཚུ་གི་དོན་ལུ་ [`crates/irohad/README.md`](./crates/irohad/README.md) ལུ་བལྟ།

## API དང་བལྟ་རྟོག་འབད་ཚུལ།

Torii གིས་ Norito དང་ JSON APIs གཉིས་ཆ་ར་ ཕྱིར་བཏོན་འབདཝ་ཨིན། སྤྱིར་བཏང་བཀོལ་སྤྱོད་པའི་མཇུག་སྣོད་ཚུ།

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

མཐའ་མཇུག་གི་གཞི་བསྟུན་ཆ་ཚང་ནང་བལྟ།

- [I18NI0000095X](I18NU0000041X)
- [I18NI0000096X](I18NU0000042X)

## ཚད་གཞི།

- [`crates/iroha`](./crates/iroha): མཁོ་སྤྲོད་པ་དཔེ་མཛོད་.
- [I18NI0000098X](./crates/irohad): མཉམ་རོགས་ཌེ་མཱོན་གཉིས་ལྡན་ཚུ།
- [I18NI0000099X](./crates/iroha_cli): གཞི་བསྟུན་ CLI.
- [`crates/iroha_core`](./crates/iroha_core): ལེཌ་ཇར་/ཀོར་བཀོལ་སྤྱོད་འཕྲུལ་ཆས།
- [I18NI0000010101X](./crates/iroha_config): ཡིག་དཔར་རྐྱབས་པའི་རིམ་སྒྲིག་དཔེ་ཚད།
- [`crates/iroha_data_model`](./crates/iroha_data_model): ཀེ་ནོ་ནིག་གནས་སྡུད་དཔེ་ཚད།
- [`crates/iroha_crypto`](./crates/iroha_crypto): ཀིརིཔ་ཊོ་གཱ་ར་ཕིག་གི་གནའ་དུས་ཚུ།
- [`crates/norito`](I18NU000000050X): གཏན་འབེབས་རིམ་སྒྲིག་ཀོ་ཌེཀ།
- [`crates/ivm`](I18NU000000051X): Iroha བརྡ་དོན་འཕྲུལ་ཆས།
- [`crates/iroha_kagami`](I18NU000000052X): ལྡེ་མིག་/རིགས་མཚན་/རིམ་སྒྲིག་ལག་ཆས།

## ཡིག་ཆའི་ས་ཁྲ།

- ཡིག་ཆ་གཙོ་བོ་: [`docs/README.md`](./docs/README.md)
- Genesi: [`docs/genesis.md`](./docs/genesis.md)
- མོས་མཐུན་ (I18NT0000003X): [I18NI000000109X](Kotodama)
- བརྒྱུད་འཕྲིན་གྱི་མདོང་ལམ་: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P ནང་འཁོད་: [`docs/source/p2p.md`](./docs/source/p2p.md)
- I18NT0000025X སི་ཀཱལ་: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama ཡིག་གཟུགས་: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito གློག་ཐག་རྩ་སྒྲིག་: [`norito.md`](I18NU0000000060X)
- ད་ལྟའི་ལས་ཀའི་རྗེས་འདེད་: [I18NI0000115X](./status.md), [`roadmap.md`](I18NU000000062X)

## བསྒྱུར།

ཇ་པཱན་གྱི་བལྟ་དཔྱད།: [`README.ja.md`](I18NU000000063X)

གཞན་ཡང་ བལྟ་སྟངས།
[I18NI000000118X](./README.he.md) [`README.es.md`](I18NI000000000065X](I18NI0000000065X) [I18NI000000120X](`README.pt.md`](I18NU000000066X). [`README.fr.md`](./README.fr.md) [`README.ru.md`](I18NI00000000068X)(I18NI0000000068X), [`README.ar.md`](I18NI0000000069X)(./README.ar.md). [`README.ur.md`](./README.ur.md)

སྐད་སྒྱུར་ལཱ་གི་རྒྱུན་རིམ་: [`docs/i18n/README.md`](I18NU000000071X)

## ཕན་འདེབས།

- ཕན་འདེབས་ལམ་སྟོན་: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- མི་སྡེ་/རྒྱབ་སྐྱོར་རྒྱུ་ལམ་: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## ཆོག་ཐམ།

Iroha འདི་ Apache-2.0 འོག་ལུ་ཆོག་ཐམ་ཡོདཔ་ཨིན། བལྟ། [`LICENSE`](./LICENSE).

ཡིག་ཆ་འདི་ CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/ འོག་ལུ་ཆོག་ཐམ་ཡོདཔ་ཨིན།