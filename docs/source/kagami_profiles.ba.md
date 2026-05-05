---
lang: ba
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 профилдәре

Kagami суднолары Iroha 3 селтәрҙәре өсөн алдан ҡуйылған, шуға күрә операторҙар детерминистик штамплай ала
генез пер-селтәр ручкаларыһыҙ күренә.

- Профилдәр: `iroha3-dev` (сылбыр `iroha3-dev.local`, коллекционерҙар k=1 r=1, VRF орлоҡтан алынған сылбырлы id ҡасан NPoS һайланған), `iroha3-taira` (сылбыр `iroha3-taira`, коллекционерҙар k=3 р=3, талап итә `--vrf-seed-hex` NPoS һайланғанда), `iroha3-nexus` (сылбыр `iroha3-nexus`, коллекционерҙар k=5 р=3, NPoS һайланғанда `--vrf-seed-hex` талап итә).
- Консенсус: Сора профиле селтәрҙәре (Nexus + мәғлүмәт киңлеге) NPoS һәм стадияла өҙөклөктәрҙе кире ҡағыу талап итә; рөхсәт ителгән Iroha3 таратыу Сора профилеһыҙ эшләргә тейеш.
- быуын: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Ҡулланыу `--consensus-mode npos` өсөн Nexus; `--vrf-seed-hex` тик NPoS өсөн генә ғәмәлдә (һынау өсөн талап ителә/нексус). Kagami штекерҙары DA/RBC Iroha3 һыҙығында һәм дөйөм сығарыла (сылбыр, коллекционерҙар, DA/RBC, VRF орлоҡ, бармаҡ эҙ).
- Тикшеренеү: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` профиль өмөттәрен ҡабатлай (сылбыр ид, DA/RBC, коллекционерҙар, PoP ҡаплау, консенсус бармаҡ эҙҙәре). `--vrf-seed-hex` тапшырыу NPoS-ты тикшергәндә генә һынаусы/нексус.
- Өлгө өйөмдәр: алдан генерацияланған өйөмдәр `defaults/kagami/iroha3-{dev,taira,nexus}/` аҫтында йәшәй (genis.json, config.toml, docker.compose.yml, раҫлау.txt, README). Kagami менән регенерация.
- Mochi: `mochi`/`mochi-genesis` ҡабул итеү `--genesis-profile <profile>` һәм `--vrf-seed-hex <hex>` (NPoS ғына), уларҙы Kagami-ға тапшыра һәм шул уҡ Kagami summary баҫтырып сығара, ҡасан stdout/stderr. профиле ҡулланыла.

Бүләктәр BLS PoPs топология яҙмалары менән бер рәттән һеңдерелгән, шуға күрә `kagami verify` уңышҡа өлгәшә
йәшниктән; Ышаныслы тиҫтерҙәрҙе/порттарҙы конфигтарҙа урындағы өсөн кәрәк, урындағы өсөн
төтөн йүгерә.