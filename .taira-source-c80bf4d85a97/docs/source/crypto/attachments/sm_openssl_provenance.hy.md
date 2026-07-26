---
lang: hy
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo Provenance Snapshot
% Ստեղծված՝ 2026-01-30

# Շրջակա միջավայրի ամփոփում

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`. հաղորդում է `LibreSSL 3.3.6` (համակարգի կողմից տրված TLS գործիքակազմ macOS-ում):
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`. տե՛ս `sm_iroha_crypto_tree.txt` Rust կախվածության ճշգրիտ կույտը (`openssl` crate v0.10.74, `openssl-sys` v0.9.x, վաճառվող OpenSSL080000X աղբյուրը հասանելի է OpenSSL08000X. crate; `vendored` հատկանիշը միացված է `crates/iroha_crypto/Cargo.toml`-ում՝ որոշիչ նախադիտման կառուցումների համար):

# Նշումներ

- Տեղական զարգացման միջավայրի հղումներ LibreSSL վերնագրերի/գրադարանների դեմ; արտադրության նախադիտման կառուցումները պետք է օգտագործեն OpenSSL >= 3.0.0 կամ Tongsuo 8.x: Փոխարինեք համակարգի գործիքակազմը կամ սահմանեք `OPENSSL_DIR`/`PKG_CONFIG_PATH` վերջնական արտեֆակտ փաթեթը ստեղծելիս:
- Վերականգնեք այս լուսանկարը թողարկման կառուցման միջավայրում, որպեսզի նկարահանեք ճշգրիտ OpenSSL/Tongsuo tarball հեշը (`openssl version -v`, `openssl version -b`, `openssl version -f`) և կցեք վերարտադրվող կառուցման սցենարը/ստուգիչ գումարը: Վաճառվող շինությունների համար գրանցեք `openssl-src` տուփի տարբերակը/հանձնարարությունը, որն օգտագործվում է Cargo-ի կողմից (տեսանելի է `target/debug/build/openssl-sys-*/output`-ում):
- Apple Silicon հոսթինգները պահանջում են `RUSTFLAGS=-Aunsafe-code`, երբ աշխատում է OpenSSL ծխի զրահը, որպեսզի AArch64 SM3/SM4 արագացման կոճղերը կազմվեն (ներքին տարրերն անհասանելի են macOS-ում): `scripts/sm_openssl_smoke.sh` սկրիպտը արտահանում է այս դրոշը նախքան `cargo`-ը կանչելը` CI-ի և տեղական գործարկումների հետևողականությունը պահպանելու համար:
- Փաթեթավորման խողովակաշարը ամրացնելուց հետո միացրեք աղբյուրի ծագումը (օրինակ՝ `openssl-src-<ver>.tar.gz` SHA256); օգտագործեք նույն հեշը CI արտեֆակտներում: