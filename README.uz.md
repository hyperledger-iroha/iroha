---
lang: uz
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

[![Litsenziya](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha - ruxsat etilgan va konsortsiumni joylashtirish uchun deterministik blokcheyn platformasi. U Iroha Virtual Machine (IVM) orqali hisob/aktivlarni boshqarish, zanjirdagi ruxsatlar va aqlli shartnomalarni taqdim etadi.

> Ish joyi holati va oxirgi oʻzgarishlar [`status.md`](./status.md) da kuzatiladi.

## Treklarni chiqarish

Ushbu ombor bir xil kod bazasidan ikkita o'rnatish treklarini yuboradi:

- **Iroha 2**: o'z-o'zidan ruxsat berilgan/konsorsium tarmoqlari.
- **Iroha 3 (SORA Nexus)**: bir xil yadro qutilari yordamida Nexus yo'naltirilgan joylashtirish treki.

Ikkala trek ham bir xil asosiy komponentlarga ega, jumladan Norito seriyali, Sumeragi konsensus va Kotodama -> IVM asboblar zanjiri.

## Repozitariy tartibi

- [`crates/`](./crates): asosiy zang qutilari (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `iroha_core`, I080NIX, I180NIX, `norito` va boshqalar).
- [`integration_tests/`](./integration_tests): o'zaro komponentli tarmoq/integratsiya testlari.
- [`IrohaSwift/`](./IrohaSwift): Swift SDK paketi.
- [`java/iroha_android/`](./java/iroha_android): Android SDK paketi.
- [`docs/`](./docs): foydalanuvchi/operator/ishlab chiquvchi hujjatlari.

## Tez boshlash

### Old shartlar

- [Zang barqaror](https://www.rust-lang.org/tools/install)
- Majburiy emas: Docker + Docker Mahalliy ko'p tengdoshlar uchun yozish

### Qurilish va sinovdan o'tkazish (ish maydoni)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Eslatmalar:

- To'liq ish joyini qurish taxminan 20 daqiqa davom etishi mumkin.
- To'liq ish maydoni testlari bir necha soat davom etishi mumkin.
- Ish maydoni maqsadlari `std` (WASM/no-std tuzilmalari qo'llab-quvvatlanmaydi).

### Maqsadli test buyruqlari

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK sinov buyruqlari

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

## Mahalliy tarmoqni ishga tushirish

Taqdim etilgan Docker Compose tarmog'ini ishga tushiring:

```bash
docker compose -f defaults/docker-compose.yml up
```

Standart mijoz konfiguratsiyasiga qarshi CLI dan foydalaning:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Demonga xos mahalliy joylashtirish bosqichlari uchun [`crates/irohad/README.md`](./crates/irohad/README.md) ga qarang.

## API va kuzatuvchanlik

Torii ikkala Norito va JSON API-larini ochib beradi. Umumiy operator oxirgi nuqtalari:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Toʻliq soʻnggi nuqta maʼlumotnomasiga qarang:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Asosiy qutilar

- [`crates/iroha`](./crates/iroha): mijozlar kutubxonasi.
- [`crates/irohad`](./crates/irohad): tengdosh demon ikkiliklari.
- [`crates/iroha_cli`](./crates/iroha_cli): CLI ma'lumotnomasi.
- [`crates/iroha_core`](./crates/iroha_core): daftar/yadro ijro mexanizmi.
- [`crates/iroha_config`](./crates/iroha_config): terilgan konfiguratsiya modeli.
- [`crates/iroha_data_model`](./crates/iroha_data_model): kanonik maʼlumotlar modeli.
- [`crates/iroha_crypto`](./crates/iroha_crypto): kriptografik primitivlar.
- [`crates/norito`](./crates/norito): deterministik seriyali kodek.
- [`crates/ivm`](./crates/ivm): Iroha Virtual mashina.
- [`crates/iroha_kagami`](./crates/iroha_kagami): kalit/genesis/konfiguratsiya asboblari.

## Hujjatlar xaritasi

- Asosiy hujjatlar indeksi: [`docs/README.md`](./docs/README.md)
- Ibtido: [`docs/genesis.md`](./docs/genesis.md)
- Konsensus (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Tranzaksiya quvuri: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P ichki qurilmalari: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM tizim qoʻngʻiroqlari: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama grammatikasi: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito sim formati: [`norito.md`](./norito.md)
- Joriy ishni kuzatish: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Tarjimalar

Yaponcha sharh: [`README.ja.md`](./README.ja.md)

Boshqa sharhlar:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Tarjima ish jarayoni: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Hissa va yordam

- Hissa qoʻllanmasi: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Hamjamiyat/qo‘llab-quvvatlash kanallari: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Litsenziya

Iroha Apache-2.0 ostida litsenziyalangan. Qarang: [`LICENSE`](./LICENSE).

Hujjatlar CC-BY-4.0 ostida litsenziyalangan: http://creativecommons.org/licenses/by/4.0/