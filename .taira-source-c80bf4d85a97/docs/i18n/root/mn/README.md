---
lang: mn
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

[![Лиценз](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha нь зөвшөөрөгдсөн болон консорциумын байршуулалтад зориулагдсан блокчейн платформ юм. Энэ нь Iroha виртуал машинаар (IVM) дамжуулан данс/хөрөнгийн удирдлага, сүлжээн дэх зөвшөөрөл, ухаалаг гэрээгээр хангадаг.

> Ажлын талбарын байдал болон сүүлийн үеийн өөрчлөлтүүдийг [`status.md`](./status.md)-д хянадаг.

## Дуунуудыг гаргах

Энэ агуулах нь нэг кодын сангаас хоёр байршуулах замыг илгээдэг:

- **Iroha 2**: өөрөө зохион байгуулсан зөвшөөрөлтэй/консорциумын сүлжээ.
- **Iroha 3 (SORA Nexus)**: ижил үндсэн хайрцаг ашиглан Nexus-д чиглэсэн байршуулах зам.

Хоёр зам хоёулаа Norito цуваа, Sumeragi зөвшилцөл, Kotodama -> IVM хэрэгслийн гинж зэрэг ижил үндсэн бүрэлдэхүүн хэсгүүдийг хуваалцдаг.

## Хадгалах сангийн зохион байгуулалт

- [`crates/`](./crates): зэвний үндсэн хайрцаг (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, I108082X, I1080X `norito` гэх мэт).
- [`integration_tests/`](./integration_tests): бүрэлдэхүүн хэсгүүд хоорондын сүлжээ/интеграцийн туршилтууд.
- [`IrohaSwift/`](./IrohaSwift): Swift SDK багц.
- [`java/iroha_android/`](./java/iroha_android): Android SDK багц.
- [`docs/`](./docs): хэрэглэгч/оператор/хөгжүүлэгчийн баримт бичиг.

## Хурдан эхлэл

### Урьдчилсан нөхцөл

- [Зэв тогтвортой](https://www.rust-lang.org/tools/install)
- Нэмэлт: Docker + Docker Орон нутгийн олон үет гүйлтэд зориулж зохиох

### Үүсгэх ба турших (Ажлын талбар)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Тэмдэглэл:

- Ажлын байрыг бүрэн барихад 20 минут зарцуулагдана.
- Ажлын талбарыг бүрэн шалгахад олон цаг зарцуулагдана.
- Ажлын талбар нь `std` зорилтот (WASM/no-std бүтцийг дэмждэггүй).

### Зорилтот туршилтын командууд

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK тестийн командууд

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

## Дотоод сүлжээ ажиллуулах

Өгөгдсөн Docker Compose сүлжээг эхлүүлнэ үү:

```bash
docker compose -f defaults/docker-compose.yml up
```

CLI-г үйлчлүүлэгчийн өгөгдмөл тохиргооны эсрэг ашиглана уу:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Демон-д зориулсан үндсэн байршуулалтын алхмуудыг [`crates/irohad/README.md`](./crates/irohad/README.md) үзнэ үү.

## API ба ажиглах чадвар

Torii нь Norito болон JSON API-г хоёуланг нь харуулж байна. Нийтлэг операторын төгсгөлийн цэгүүд:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Төгсгөлийн цэгийн бүрэн лавлагааг дараахаас үзнэ үү:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Үндсэн хайрцаг

- [`crates/iroha`](./crates/iroha): үйлчлүүлэгчийн номын сан.
- [`crates/irohad`](./crates/irohad): үе тэнгийн демон хоёртын файлууд.
- [`crates/iroha_cli`](./crates/iroha_cli): CLI лавлагаа.
- [`crates/iroha_core`](./crates/iroha_core): дэвтэр/үндсэн гүйцэтгэлийн хөдөлгүүр.
- [`crates/iroha_config`](./crates/iroha_config): бичсэн тохиргооны загвар.
- [`crates/iroha_data_model`](./crates/iroha_data_model): каноник өгөгдлийн загвар.
- [`crates/iroha_crypto`](./crates/iroha_crypto): криптограф командууд.
- [`crates/norito`](./crates/norito): детерминист цуваа кодлогч.
- [`crates/ivm`](./crates/ivm): Iroha Виртуал машин.
- [`crates/iroha_kagami`](./crates/iroha_kagami): түлхүүр/генезис/тохируулгын хэрэгсэл.

## Баримт бичгийн зураг

- Үндсэн баримтын индекс: [`docs/README.md`](./docs/README.md)
- Эхлэл: [`docs/genesis.md`](./docs/genesis.md)
- Зөвшилцөл (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Гүйлгээ дамжуулах хоолой: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P дотоод: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM системийн дуудлага: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama дүрэм: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito утасны формат: [`norito.md`](./norito.md)
- Одоогийн ажлын хяналт: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Орчуулга

Японы тойм: [`README.ja.md`](./README.ja.md)

Бусад тойм:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Орчуулгын ажлын урсгал: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Хувь нэмэр оруулж, тусалж байна

- Хувь нэмэр оруулах гарын авлага: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Олон нийтийн/тусламжийн сувгууд: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Лиценз

Iroha нь Apache-2.0 дагуу лицензтэй. [`LICENSE`](./LICENSE) харна уу.

Баримт бичгийг CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/ дагуу лицензжүүлсэн.