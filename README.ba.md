---
lang: ba
direction: ltr
source: README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:30:39.016220+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# I18NT000000000X Iroha

[![Лиценза] (I18NU000000032X)] (https://opensource.org/licenses/Apache-2.0)

I18NT0000001X Iroha — рөхсәт ителгән һәм консорциумдарҙы таратыу өсөн детерминистик блокчейн платформаһы. Ул иҫәп/активтар менән идара итеү, сылбырҙа рөхсәт, һәм аҡыллы килешеп, I18NT0000000014X виртуаль машина (I18NT000000023X).

> Эш киңлеге статусы һәм һуңғы үҙгәрештәр күҙәтелә [`status.md`] (./status.md).

##

Был һаҡлағыс караптар ике таратыу трек бер үк код базаһы:

- **Iroha 2**: үҙ-үҙен ҡабул иткән рөхсәт/консорциум селтәрҙәре.
- *I18NT000000016X 3 (I18NT000000022X I18NT000000019X)****: шул уҡ үҙәк йәшниктәрҙе ҡулланып I18NT000000000000020-сы йүнәлешле трасса.

Both tracks share the same core components, including Norito serialization, Sumeragi consensus, and the Kotodama -> IVM toolchain.

## Репозиторий макеты

- [I18NI000000078X] (I18NU000000034X): ядро ​​Rust йәшниктәре (I18NI00000000079X X, `irohad`, `iroha_cli`, I18NI0000082X, I18NI0000000083 X, `norito`, һ.б.).
- [`integration_tests/`](I18NU000000035X): селтәр/интеграция һынауҙары.
- [`IrohaSwift/` X] (I18NU000000036X): Свифт SDK пакеты.
- [`java/iroha_android/`] (I18NU000000037X): Android SDK пакеты.
- [I18NI000000088X](I18NU000000038X): ҡулланыусы/оператор/үҫтереүҙең документацияһы.

## Quickstart

### Алдан шарттар

- [Рат тотороҡло] (https://www.rust-lang.org/tools/install X)
- Һорау: I18NT000000009X X + I18NT00000000010X урындағы күп тиңдәштәр өсөн Компози

### Төҙөү һәм һынау (Воркспейс)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Иҫкәрмәләр:

- Тулы эш урыны төҙөү 20 минут тирәһе ваҡыт ала.
- Тулы эш урыны һынауҙары бер нисә сәғәт ваҡыт талап итә ала.
- Эш майҙаны `std` X (WASM/no-std төҙөүҙәр ярҙам итмәй).

### Маҡсатлы һынау командалары

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK һынау командалары

I18NF000000028X

I18NF000000029X.

## Урындағы селтәр йүгерегеҙ

Башланғыс бирелгән I18NT000000011X Композис селтәре:

I18NF000000030X

Ҡулланыу CLI ҡаршы ғәҙәттәгесә клиент конфиг:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Демон-специфик туған таратыу аҙымдары өсөн, ҡарағыҙ [I18NI000000090X] (./crates/irohad/README.md).

## API һәм күҙәтеүсәнлек

Torii I18NT000000007X һәм JSON API-ларын фашлай. Дөйөм оператор ос нөктәләре:

- I18NI000000091X
- I18NI000000092X
- I18NI000000093X
- `GET /v1/events/sse`

Ҡарағыҙ, тулы ос нөктәһе һылтанмаһы:

- [I18NI0000000955Х] (./docs/source/telemetry.md)
- [I18NI000000096X X] (./docs/portal/docs/reference/README.md)

## Ядро йәшниктәре

- [`crates/iroha`] (./crates/iroha): клиент китапханаһы.
- [`crates/irohad` X] (./crates/irohad): тиңдәштәре демонарийҙары.
- [I18NI0000009XX X] (I18NU000000045X): белешмә CLI.
- [`crates/iroha_core`](I18NU000000046X): баш китабы/ядро башҡарыу двигателе.
- [`crates/iroha_config`](I18NU000000047X): терпе конфигурация моделе.
- [`crates/iroha_data_model`](I18NU000000048X): канон мәғлүмәттәр моделе.
- [`crates/iroha_crypto`](I18NU000000049X): криптографик примитивтар.
- [`crates/norito`] (./crates/norito): детерминистик сериализация кодек.
- [`crates/ivm`] (./crates/ivm): Iroha виртуаль машина.
- [`crates/iroha_kagami`](I18NU000000052X): асҡыс/генез/конфиг инструменттары.

## Документация картаһы

- Төп docs индексы: [`docs/README.md`] (./docs/README.md)
- Башланмыш: [`docs/genesis.md`] (I18NU000000054X)
- Консенсус (I18NT000000003X): [I18NI000000109X X] (./docs/source/sumeragi.md)
- Транзакция торбаһы: [I18NI000000110X] (./docs/source/pipeline.md)
- P2P эске: [`docs/source/p2p.md`] (I18NU000000057X)
- I18NT0000000025X syscalls: [I18NI000000112X] (./docs/source/ivm_syscalls.md X)
- I18NT0000000005X грамматикаһы: [`docs/source/kotodama_grammar.md`] (./docs/source/kotodama_grammar.md X)
- I18NT0000000008X сым форматында: [I18NI000000114X] (./norito.md)
- Ағымдағы эштәрҙе күҙәтеү: [`status.md`] (I18NU000000061X), [Iroha] (./roadmap.md)

## Тәржемә

Япония дөйөм ҡараш: [`README.ja.md`] (./README.ja.md)

Башҡа дөйөм ҡараш:
[I18NI000000118X] (I18NU000000064X), [I18NI000000119X X] (I18NU000000065X), [I18NI0000000120X] (./README.pt.md), . [`README.fr.md`] (I18NU000000067X), [I18NI000000122X] (I18NU000000068X), [I18NI0000000123X] (I18NU00000069ХХ). [I18NI000000124X] (./README.ur.md)

Тәржемә эш ағымы: [`docs/i18n/README.md`] (./docs/i18n/README.md)

## Үҙ өлөшө һәм ярҙам

- Иғәнә ҡулланмаһы: [`CONTRIBUTING.md`X] (./CONTRIBUTING.md)
- Йәмәғәт/ярҙам каналдары: [I18NI000000127X] (I18NU0000073X)

## Лицензия

I18NT000000018X лицензияһы буйынса Apache-2.0. Ҡара: [`LICENSE`] (./LICENSE).

Документация лицензияһы буйынса CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/ .