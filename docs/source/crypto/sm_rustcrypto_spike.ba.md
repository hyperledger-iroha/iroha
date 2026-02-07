---
lang: ba
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Иҫкәрмәләр өсөн RustCrypto SM интеграция шпик.

# RustCrypto SM шпик иҫкәрмәләр

## Ғәҙел
18NI000000002X, һәм `sm4` йәшниктәр (плюс `rfc6979`, `ccm`, `gcm`) индереү. `iroha_crypto` йәшниктә таҙа компиляциялана һәм киңерәк эш урынына функция флагын проводка алдынан ҡабул итеүгә яраҡлы төҙөү ваҡытын бирә.

## Тәҡдим ителгәнгә бәйлелек картаһы

| Йәшник | Тәҡдим ителгән версия | Үҙенсәлектәре | Иҫкәрмәләр |
|------|-------------------|----------|--------|
| `sm2` | `0.13` (RustCrypto/ҡултамдар) | `std` | `elliptic-curve`-ға бәйле; раҫлау MSRV матчтар эш урыны. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/hases) | ғәҙәттәгесә | API параллель `sha2`, ғәмәлдәге `digest` һыҙаттары менән интеграциялана. |
| `sm4` | `0.5.1` (RustCrypto/блок-шифрҙар) | ғәҙәттәгесә | шифр һыҙаттары менән эшләй; AEAD уратып алыу өсөн кисектереп, һуңыраҡ шпик. |
| `rfc6979` | `0.4` | ғәҙәттәгесә | Ҡабаттан ҡулланыу өсөн детерминистик булмаған no derivation. |

*версиялар 2024-12 йылдарҙағы ағымдағы релиздарҙы сағылдыра; раҫлау менән `cargo search` десант алдынан.*

## Манифест үҙгәрештәре (проект)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Һуңынан: `elliptic-curve` шрифты буйынса `iroha_crypto` версияларын матчҡа (әлеге ваҡытта `0.13.8` X).

## Шпик тикшерелгән исемлек
- [x] `crates/iroha_crypto/Cargo.toml`-ҡа опциональ бәйлелек һәм функция өҫтәү.
- [x] `signature::sm` модулен `cfg(feature = "sm")`-тан артында булдырыу, унда проводкаларҙы раҫлау өсөн урын биллегы.
- [x] Йүгереп `cargo check -p iroha_crypto --features sm` компиляцияны раҫлау өсөн; рекордлы төҙөү ваҡыты һәм яңы бәйлелек иҫәбе (`cargo tree --features sm`).
- [x] std-тик-тик поза менән `cargo check -p iroha_crypto --features sm --locked` раҫлай; ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
``` төҙөүҙәр башҡаса ярҙам итмәй.
- [x] Файл һөҙөмтәләре (ваҡыт, бәйлелек ағас дельта) `docs/source/crypto/sm_program.md`.

## Күҙәтеүҙәр алыу өсөн
- Өҫтәмә компиляция ваҡыты ҡаршы база һыҙығы.
- `cargo builtinsize` менән Бинар күләме йоғонтоһо (әгәр үлсәмле).
- Теләһә ниндәй МСРВ йәки конфликттар (мәҫәлән, `elliptic-curve` ваҡ версиялары менән).
- Иҫкәртмәләр сығарылған (хәүефһеҙ код, конст-фн ҡапҡаһы), тип, өҫкө ағым патчтары талап итә ала.

## көтөп торған әйберҙәр
- Крипто WG раҫлауын көтөп, эш урыны менән бәйлелек графигын ҡабартыу алдынан.
- Раҫлау өсөн һатыусы йәшниктәр өсөн тикшерергә йәки йәшниктәргә таянып.ио (көҙгөләр кәрәк булыуы мөмкин).
- `Cargo.lock` координатаһы `sm_lock_refresh_plan.md` өсөн маркировка тикшерелгән исемлек тамамланғансы.
- Ҡулланыу `scripts/sm_lock_refresh.sh` бер тапҡыр раҫлау бирелә, локфей һәм бәйлелек ағасын тергеҙеү өсөн.

## 2025-01-19 Спайк журналы
- Өҫтәлгән опциональ бәйлелектәр (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1` X, `rfc6979 0.4`) һәм `sm` `iroha_crypto`.
- Stubbed `signature::sm` модулендә компиляция ваҡытында хеширование/блок шифр API-ларҙы ҡулланырға.
- `cargo check -p iroha_crypto --features sm --locked` хәҙер бәйлелек графигын хәл итә, әммә `Cargo.lock` яңыртыу талаптары менән аборттарҙы хәл итә; Һаҡланыу сәйәсәте тыя lockfile мөхәррирләү, шуға күрә компиляция йүгерә ҡала көтөп, беҙ рөхсәт ителгән блокировка яңыртыу координациялау.## 2026-02-12 Спайк журналы
- Алдағы локфейсыны хәл итеү — бәйлелектәр инде тотола — шуға күрә `cargo check -p iroha_crypto --features sm --locked` уңыш (һалҡын төҙөү 7.9s dev Mac; өҫтәмә яңынан йүгерә 0.23s).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` 1,0-да үтә, был `std`-тик конфигурацияларҙағы опциональ функциялар компиляторҙарын раҫлай (`no_std` юлы юҡ).
- `sm` функцияһы менән бәйле дельта 11 йәшник индереү мөмкинлеген бирә: `base64ct`, `ghash`, `opaque-debug`, `rfc6979`, `ccm`. `polyval`, `primeorder`, `sm2`, `sm3`, `sm4` һәм `sm4-gcm`. (`rfc6979` инде база графикының бер өлөшө булды.)
- Ҡулланылмаған NEON сәйәсәт ярҙамсылары өсөн иҫкәртмәләр төҙөү; ҡалдырырға, нисек-был тиклем счетчик тигеҙләү эшләү ваҡытын ҡабаттан индереү, был код юлдары.