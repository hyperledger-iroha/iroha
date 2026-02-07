---
lang: hy
direction: ltr
source: README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:30:39.016220+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hyperledger Iroha

[![Լիցենզիա](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha-ը դետերմինիստական բլոկչեյն հարթակ է թույլատրված և կոնսորցիումի տեղակայման համար: Այն տրամադրում է հաշվի/ակտիվների կառավարում, ցանցային թույլտվություններ և խելացի պայմանագրեր Iroha վիրտուալ մեքենայի միջոցով (IVM):

> Աշխատանքային տարածքի կարգավիճակը և վերջին փոփոխությունները դիտվում են [`status.md`]-ում (./status.md):

## Թողարկեք հետքերը

Այս պահոցը առաքում է երկու տեղակայման հետքեր նույն կոդերի բազայից.

- **Iroha 2**. ինքնակառավարվող թույլատրված/կոնսորցիումային ցանցեր:
- **Iroha 3 (SORA Nexus)**. Nexus-ի վրա հիմնված տեղակայման ուղին՝ օգտագործելով նույն միջուկային տուփերը:

Երկու հետքերը կիսում են նույն հիմնական բաղադրիչները, ներառյալ Norito սերիալիզացիան, Sumeragi կոնսենսուսը և Kotodama -> IVM գործիքների շղթան:

## Պահեստի դասավորություն

- [`crates/`](./crates): միջուկի ժանգոտ արկղեր (`iroha`, `irohad`, `iroha_cli`, I18NI00000008NI00000 `norito` և այլն):
- [`integration_tests/`](./integration_tests)՝ խաչաձեւ բաղադրիչ ցանցի/ինտեգրման թեստեր:
- [`IrohaSwift/`](./IrohaSwift)՝ Swift SDK փաթեթ:
- [`java/iroha_android/`](./java/iroha_android)՝ Android SDK փաթեթ:
- [`docs/`](./docs)՝ օգտվողի/օպերատորի/մշակողի փաստաթղթեր:

## Արագ մեկնարկ

### Նախադրյալներ

- [Ժանգը կայուն] (https://www.rust-lang.org/tools/install)
- Լրացուցիչ. Docker + Docker Կազմել տեղական բազմակի վազքի համար

### Կառուցել և փորձարկել (Աշխատանքային տարածք)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Նշումներ:

- Աշխատանքային տարածքի ամբողջական ստեղծումը կարող է տևել մոտ 20 րոպե:
- Աշխատանքային տարածքի ամբողջական փորձարկումները կարող են տեւել մի քանի ժամ:
- Աշխատանքային տարածքը ուղղված է `std`-ին (WASM/no-std կառուցումները չեն ապահովվում):

### Թիրախային փորձարկման հրամաններ

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK փորձարկման հրամաններ

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## Գործարկեք տեղական ցանց

Սկսեք տրամադրված Docker Compose ցանցը՝

```bash
docker compose -f defaults/docker-compose.yml up
```

Օգտագործեք CLI-ն կանխադրված հաճախորդի կազմաձևման դեմ.

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Դեմոնի հատուկ տեղակայման քայլերի համար տես [`crates/irohad/README.md`](./crates/irohad/README.md):

## API և դիտելիություն

Torii-ը բացահայտում է և՛ Norito, և՛ JSON API-ները: Ընդհանուր օպերատորի վերջնակետեր.

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Տեսեք վերջնական կետի ամբողջական հղումը հետևյալում.

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Հիմնական տուփեր

- [`crates/iroha`](./crates/iroha). հաճախորդի գրադարան:
- [`crates/irohad`](./crates/irohad)՝ նմանակ դեյմոն երկուականներ:
- [`crates/iroha_cli`](./crates/iroha_cli)՝ հղում CLI:
- [`crates/iroha_core`](./crates/iroha_core): մատյան/միջուկի կատարման շարժիչ:
- [`crates/iroha_config`](./crates/iroha_config)՝ մուտքագրված կոնֆիգուրացիայի մոդել:
- [`crates/iroha_data_model`](./crates/iroha_data_model). տվյալների կանոնական մոդել:
- [`crates/iroha_crypto`](./crates/iroha_crypto): գաղտնագրային պարզունակներ:
- [`crates/norito`](./crates/norito). սերիականացման դետերմինիստական ​​կոդեկ:
- [`crates/ivm`](./crates/ivm): Iroha վիրտուալ մեքենա:
- [`crates/iroha_kagami`](./crates/iroha_kagami): բանալի/ծագում/կոնֆիգուրացիայի գործիքավորում:

## Փաստաթղթային քարտեզ

- Հիմնական փաստաթղթերի ինդեքս՝ [`docs/README.md`](./docs/README.md)
- Ծննդոց՝ [`docs/genesis.md`](./docs/genesis.md)
- Համաձայնություն (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Գործարքի խողովակաշար՝ [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P ներքին տարրեր՝ [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM համակարգային զանգեր՝ [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama քերականություն՝ [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito մետաղալարերի ձևաչափ՝ [`norito.md`](./norito.md)
- Ընթացիկ աշխատանքի հետագծում՝ [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Թարգմանություններ

Ճապոնական ակնարկ՝ [`README.ja.md`](./README.ja.md)

Այլ ակնարկներ.
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Թարգմանության աշխատանքային հոսք՝ [`docs/i18n/README.md`](./docs/i18n/README.md)

## Նպաստել և օգնել

- Ներդրումների ուղեցույց՝ [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Համայնք/աջակցության ալիքներ՝ [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Լիցենզիա

Iroha-ը լիցենզավորված է Apache-2.0-ով: Տես [`LICENSE`](./LICENSE):

Փաստաթղթերը լիցենզավորված են CC-BY-4.0 համաձայն՝ http://creativecommons.org/licenses/by/4.0/