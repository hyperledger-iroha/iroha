---
lang: ka
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

[![ლიცენზია](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha არის დეტერმინისტული ბლოკჩეინის პლატფორმა ნებადართული და კონსორციუმის განლაგებისთვის. ის უზრუნველყოფს ანგარიშის/აქტივების მენეჯმენტს, ნებართვებს და ჭკვიან კონტრაქტებს Iroha ვირტუალური აპარატის საშუალებით (IVM).

> სამუშაო სივრცის სტატუსს და ბოლო ცვლილებებს თვალყურს ადევნებთ [`status.md`](./status.md).

## გამოუშვით ტრეკები

ეს საცავი აგზავნის ორ განლაგების ბილიკს ერთი და იგივე კოდების ბაზიდან:

- **Iroha 2**: ნებადართული/კონსორციუმის ქსელები, რომლებიც განთავსებულია საკუთარ თავზე.
- **Iroha 3 (SORA Nexus)**: Nexus-ზე ორიენტირებული განლაგების ბილიკი იმავე ბირთვიანი უჯრების გამოყენებით.

ორივე ბილიკი იზიარებს ერთსა და იმავე ძირითად კომპონენტებს, მათ შორის Norito სერიალიზაციას, Sumeragi კონსენსუსს და Kotodama -> IVM ხელსაწყოების ჯაჭვს.

## საცავის განლაგება

- [`crates/`](./crates): ბირთვის ჟანგის ყუთები (`iroha`, `irohad`, `iroha_cli`, I18NI00000008NI00000 `norito` და ა.შ.).
- [`integration_tests/`](./integration_tests): ჯვარედინი კომპონენტის ქსელის/ინტეგრაციის ტესტები.
- [`IrohaSwift/`](./IrohaSwift): Swift SDK პაკეტი.
- [`java/iroha_android/`](./java/iroha_android): Android SDK პაკეტი.
- [`docs/`](./docs): მომხმარებლის/ოპერატორის/დეველოპერის დოკუმენტაცია.

## სწრაფი დაწყება

### წინაპირობები

- [ჟანგის სტაბილური] (https://www.rust-lang.org/tools/install)
- სურვილისამებრ: Docker + Docker შედგენა ლოკალური მრავალთანაბარიანი გაშვებისთვის

### აშენება და ტესტირება (სამუშაო სივრცე)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

შენიშვნები:

- სრული სამუშაო სივრცის შექმნა შეიძლება დაახლოებით 20 წუთი დასჭირდეს.
- სამუშაო სივრცის სრულ ტესტებს შეიძლება რამდენიმე საათი დასჭირდეს.
- სამუშაო სივრცე მიზნად ისახავს `std` (WASM/no-std builds არ არის მხარდაჭერილი).

### მიზნობრივი ტესტის ბრძანებები

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK ტესტის ბრძანებები

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

## გაუშვით ლოკალური ქსელი

გაუშვით მოწოდებული Docker Compose ქსელი:

```bash
docker compose -f defaults/docker-compose.yml up
```

გამოიყენეთ CLI ნაგულისხმევი კლიენტის კონფიგურაციის წინააღმდეგ:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

დემონის სპეციფიკური დანერგვის საფეხურებისთვის იხილეთ [`crates/irohad/README.md`](./crates/irohad/README.md).

## API და დაკვირვებადობა

Torii ავლენს როგორც Norito, ასევე JSON API-ებს. საერთო ოპერატორის საბოლოო წერტილები:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

იხილეთ სრული საბოლოო წერტილის მითითება:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## ძირითადი ყუთები

- [`crates/iroha`](./crates/iroha): კლიენტის ბიბლიოთეკა.
- [`crates/irohad`](./crates/irohad): თანატოლი დემონის ბინარები.
- [`crates/iroha_cli`](./crates/iroha_cli): მითითება CLI.
- [`crates/iroha_core`](./crates/iroha_core): წიგნის/ბირთვული შესრულების ძრავა.
- [`crates/iroha_config`](./crates/iroha_config): აკრეფილი კონფიგურაციის მოდელი.
- [`crates/iroha_data_model`](./crates/iroha_data_model): მონაცემთა კანონიკური მოდელი.
- [`crates/iroha_crypto`](./crates/iroha_crypto): კრიპტოგრაფიული პრიმიტივები.
- [`crates/norito`](./crates/norito): განმსაზღვრელი სერიალიზაციის კოდეკი.
- [`crates/ivm`](./crates/ivm): Iroha ვირტუალური მანქანა.
- [`crates/iroha_kagami`](./crates/iroha_kagami): გასაღები/გენეზისი/კონფიგურაციის ხელსაწყოები.

## დოკუმენტაციის რუკა

- ძირითადი დოკუმენტების ინდექსი: [`docs/README.md`](./docs/README.md)
- დაბადება: [`docs/genesis.md`](./docs/genesis.md)
- კონსენსუსი (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- ტრანზაქციის მილსადენი: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P შიდა: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM სისტემის ზარები: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama გრამატიკა: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito მავთულის ფორმატი: [`norito.md`](./norito.md)
- მიმდინარე სამუშაოს თვალყურის დევნება: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## თარგმანები

იაპონური მიმოხილვა: [`README.ja.md`](./README.ja.md)

სხვა მიმოხილვები:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

თარგმანის სამუშაო პროცესი: [`docs/i18n/README.md`](./docs/i18n/README.md)

## წვლილი და დახმარება

- წვლილის სახელმძღვანელო: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- საზოგადოება/მხარდაჭერის არხები: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## ლიცენზია

Iroha ლიცენზირებულია Apache-2.0-ით. იხილეთ [`LICENSE`](./LICENSE).

დოკუმენტაცია ლიცენზირებულია CC-BY-4.0-ით: http://creativecommons.org/licenses/by/4.0/