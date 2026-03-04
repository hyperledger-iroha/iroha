---
lang: hy
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Կրիպտո կախվածության աուդիտ

## Streebog (`streebog` տուփ)

- **Տարբերակ ծառի մեջ.** `0.11.0-rc.2` վաճառվում է `vendor/streebog`-ի ներքո (օգտագործվում է, երբ `gost` հատկությունը միացված է):
- **Սպառող.** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + հաղորդագրությունների հաշինգ):
- **Կարգավիճակ:** Միայն ազատ արձակել-թեկնածու: Ոչ մի RC տուփ ներկայումս չի առաջարկում API-ի պահանջվող մակերեսը,
  Այսպիսով, մենք արտացոլում ենք արկղի ներսի ծառը աուդիտի համար, մինչդեռ մենք հետևում ենք հոսանքին հակառակ վերջնական թողարկման համար:
- ** Վերանայեք անցակետերը.
  - Ստուգված հեշ ելքը Wycheproof փաթեթի և TC26 հարմարանքների դեմ միջոցով
    `cargo test -p iroha_crypto --features gost` (տես `crates/iroha_crypto/tests/gost_wycheproof.rs`):
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    իրականացնում է Ed25519/Secp256k1 յուրաքանչյուր TC26 կորի կողքին ընթացիկ կախվածությամբ:
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    համեմատում է ավելի թարմ չափումները մուտքագրված միջինների հետ (օգտագործեք `--summary-only` CI-ում, ավելացրեք
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` վերաբազավորման ժամանակ):
  - `scripts/gost_bench.sh`-ը փաթաթում է նստարանը + ստուգեք հոսքը; անցեք `--write-baseline`՝ JSON-ը թարմացնելու համար:
    Տե՛ս `docs/source/crypto/gost_performance.md` ավարտից մինչև վերջ աշխատանքային հոսքի համար:
- ** Մեղմացումներ.** `streebog`-ը երբևէ կանչվում է միայն դետերմինիստական ​​փաթաթանների միջոցով, որոնք զրոյացնում են ստեղները;
  Ստորագրողը հեջավորում է OS էնտրոպիայով նոցերը՝ խուսափելու RNG-ի աղետալի ձախողումից:
- **Հաջորդ գործողությունները.** Հետևեք RustCrypto-ի streebog `0.11.x` թողարկմանը; երբ պիտակը վայրէջք կատարի, բուժեք
  արդիականացրեք որպես ստանդարտ կախվածության բախում (ստուգեք ստուգման գումարը, վերանայեք տարբերությունը, գրանցեք ծագումը և
  գցել վաճառվող հայելին):