---
lang: kk
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

[![Лицензия](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha – рұқсат етілген және консорциумды орналастыруға арналған детерминирленген блокчейн платформасы. Ол Iroha виртуалды машинасы (IVM) арқылы есептік жазбаны/активтерді басқаруды, тізбектегі рұқсаттарды және смарт келісімшарттарды қамтамасыз етеді.

> Жұмыс кеңістігінің күйі және соңғы өзгерістер [`status.md`](./status.md) ішінде бақыланады.

## Тректерді шығару

Бұл репозиторий бір код базасынан екі орналастыру жолын жібереді:

- **Iroha 2**: өздігінен орналастырылған рұқсат етілген/консорциум желілері.
- **Iroha 3 (SORA Nexus)**: бірдей негізгі жәшіктерді пайдаланатын Nexus бағдарланған орналастыру жолы.

Екі трек бірдей негізгі құрамдастарды, соның ішінде Norito сериялау, Sumeragi консенсус және Kotodama -> IVM құралдар тізбегін ортақ пайдаланады.

## Репозиторийдің орналасуы

- [`crates/`](./crates): негізгі тот жәшіктері (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, I18030X, I108NI `norito` және т.б.).
- [`integration_tests/`](./integration_tests): кросс-компоненттік желі/интеграция сынақтары.
- [`IrohaSwift/`](./IrohaSwift): Swift SDK бумасы.
- [`java/iroha_android/`](./java/iroha_android): Android SDK бумасы.
- [`docs/`](./docs): пайдаланушы/оператор/әзірлеуші ​​құжаттамасы.

## Жылдам бастау

### Алғышарттар

- [Тот тұрақты](https://www.rust-lang.org/tools/install)
- Қосымша: Docker + Docker Жергілікті көп деңгейлі жүгірулер үшін құрастырыңыз

### Құру және сынақтан өткізу (жұмыс кеңістігі)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Ескертулер:

- Толық жұмыс кеңістігін құру шамамен 20 минутқа созылуы мүмкін.
- Толық жұмыс кеңістігінің сынақтары бірнеше сағатқа созылуы мүмкін.
- Жұмыс кеңістігінің мақсаттары `std` (WASM/no-std құрастыруларына қолдау көрсетілмейді).

### Мақсатты сынақ пәрмендері

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK сынақ пәрмендері

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

## Жергілікті желіні іске қосыңыз

Берілген Docker Compose желісін бастаңыз:

```bash
docker compose -f defaults/docker-compose.yml up
```

Әдепкі клиент конфигурациясына қарсы CLI пайдаланыңыз:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Демонға тән жергілікті орналастыру қадамдары үшін [`crates/irohad/README.md`](./crates/irohad/README.md) бөлімін қараңыз.

## API және бақылау мүмкіндігі

Torii Norito және JSON API интерфейстерін көрсетеді. Жалпы оператордың соңғы нүктелері:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Толық соңғы нүкте сілтемесін мына жерден қараңыз:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Негізгі жәшіктер

- [`crates/iroha`](./crates/iroha): клиент кітапханасы.
- [`crates/irohad`](./crates/irohad): тең демонның екілік файлдары.
- [`crates/iroha_cli`](./crates/iroha_cli): CLI сілтемесі.
- [`crates/iroha_core`](./crates/iroha_core): бухгалтерлік кітап/өзекті орындау қозғалтқышы.
- [`crates/iroha_config`](./crates/iroha_config): терілген конфигурация үлгісі.
- [`crates/iroha_data_model`](./crates/iroha_data_model): деректердің канондық үлгісі.
- [`crates/iroha_crypto`](./crates/iroha_crypto): криптографиялық примитивтер.
- [`crates/norito`](./crates/norito): детерминирленген сериялау кодегі.
- [`crates/ivm`](./crates/ivm): Iroha Виртуалды машина.
- [`crates/iroha_kagami`](./crates/iroha_kagami): кілт/генезис/конфигурация құралдары.

## Құжаттама картасы

- Негізгі құжаттар индексі: [`docs/README.md`](./docs/README.md)
- Жаратылыс: [`docs/genesis.md`](./docs/genesis.md)
- Консенсус (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Транзакция құбыры: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P ішкі құрылғылары: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM жүйе қоңыраулары: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama грамматикасы: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito сым пішімі: [`norito.md`](./norito.md)
- Ағымдағы жұмысты қадағалау: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Аудармалар

Жапондық шолу: [`README.ja.md`](./README.ja.md)

Басқа шолулар:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Аударма жұмыс процесі: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Үлес қосу және көмек

- Үлес нұсқаулығы: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Қауымдастық/қолдау арналары: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Лицензия

Iroha Apache-2.0 бойынша лицензияланған. [`LICENSE`](./LICENSE) қараңыз.

Құжаттама CC-BY-4.0 бойынша лицензияланған: http://creativecommons.org/licenses/by/4.0/