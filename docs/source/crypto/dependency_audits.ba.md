---
lang: ba
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Криптоға бәйлелек аудиттары

## Streebog (`streebog` йәшник)

- **Ағастағы версия:** `0.11.0-rc.2` `vendor/streebog` буйынса һатылған (`gost` функцияһы өҫтөндә ҡулланылғанда ҡулланыла).
- **Ҡулланыусылар:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + хәбәр хеширование).
- **Статус:** Тик сығарыу-кандидат ҡына. Бер ниндәй ҙә RC йәшник әлеге ваҡытта кәрәкле API өҫтө тәҡдим итә,
  шулай итеп, беҙ көҙгө йәшник-ағай өсөн аудитлы, ә беҙ өҫкө ағымды күҙәтеп, һуңғы релиз өсөн.
- **Тикшереү пункттарын тикшерергә:**
  - тикшерелгән хеш сығыу ҡаршы Wycheproof люкс һәм TC26 ҡоролмалары аша .
    `cargo test -p iroha_crypto --features gost` (ҡара: `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    күнекмәләр Ed25519/Secp256k1 менән бер рәттән һәр TC26 ҡойроғо менән ток бәйлелек.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    яңы үлсәүҙәрҙе тикшерелгән медианаларға ҡаршы сағыштыра (CI-ла `--summary-only` ҡулланыу, өҫтәй
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` ҡасан скидка).
  - `scripts/gost_bench.sh` эскәмйә + чек ағымын урап ала; үткәреү `--write-baseline` яңыртыу өсөн JSON.
    Ҡарағыҙ `docs/source/crypto/gost_performance.md` өсөн ос-ос эш ағымы.
- **Яҙылыу:** `streebog` тик ҡасан да булһа нульиз асҡыстары детерминистик урауҙар аша ғына саҡырыла;
  ҡул ҡуйыусы хеджировать nonces менән OS энтропия ҡотолоу өсөн катастрофик RNG етешһеҙлеге.
- **Киләһе ғәмәлдәр:** RustCrypto’s стребог `0.11.x` релизы; бер тапҡыр тег ерҙәре, дауалау
  яңыртыу стандарт бәйлелек ҡабарынҡы булараҡ (тикшерергә тикшерелгән сумма, тикшерергә дифф, яҙма провенанс, һәм
  һатыу көҙгөһөн төшөрөп).