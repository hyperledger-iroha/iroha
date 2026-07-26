---
lang: ba
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Тонссуо провенанс снимок
% генерацияланған: 2026-01-30

# Тирә-яҡ мөхиткә йомғаҡ

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: хәбәр итеүҙәренсә, `LibreSSL 3.3.6` (система менән тәьмин ителгән TLS инструменттары macOS буйынса).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: ҡара: `sm_iroha_crypto_tree.txt` теүәл тут бәйлелек стекаһы өсөн (`openssl` йәшник v0.10.74, `openssl-sys` v0.9.x, һатыу OpenSSL 3.x сығанаҡтары аша мөмкин `pkg-config --modversion openssl`. йәшник;

# Иҫкәрмәләр

- LibreSSL башлыҡтары/китапханаларына ҡаршы урындағы үҫеш мөхите; етештереүҙе алдан ҡарау төҙөүҙәре OpenSSL >= 3.0.0 йәки Тонгсуо 8.x ҡулланырға тейеш. Система инструменталь йәки ҡуйырға `OPENSSL_DIR`/`PKG_CONFIG_PATH` генерациялау һуңғы артефакт өйөмө.
- Был снимокты регенерациялау эсендә релиз төҙөү мөхите аныҡ OpenSSL/Тонссуо татар хеш (`openssl version -v`, `openssl version -b`, `openssl version -f`) һәм беркетергә ҡабатланған төҙөү сценарийы/чексум. Һатыу өсөн төҙөлгән төҙөү өсөн, `openssl-src` йәшник версияһы/коммиты ҡулланылған Cargo (`target/debug/build/openssl-sys-*/output`-та күренгән).
- Apple кремний хужалары талап итә `RUSTFLAGS=-Aunsafe-code` OpenSSL төтөн йүгәнен эшләгәндә шулай AArch64 SM3/SM4 тиҙләтеү стабтары компиляция (эске macOS-та доступный түгел). `scripts/sm_openssl_smoke.sh` сценарийы был флагты экспортлай, ә һуңынан CI һәм урындағы йүгереүҙе эҙмә-эҙлекле тотоу өсөн `cargo` мөрәжәғәт итә.
- Өҫкө ағым сығанаҡ провенанс беркетелгән (мәҫәлән, `openssl-src-<ver>.tar.gz` SHA256) бер тапҡыр упаковка торбаһы ҡыҫтырылған; CI артефакттарында шул уҡ хеш ҡулланыу.