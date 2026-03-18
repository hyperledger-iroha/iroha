---
lang: kk
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Криптотәуелділік аудиті

## Streebog (`streebog` жәшігі)

- **Ағаштағы нұсқа:** `0.11.0-rc.2` `vendor/streebog` астында сатылады (`gost` мүмкіндігі қосылған кезде пайдаланылады).
- **Тұтынушы:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + хабар хэштеу).
- **Мәртебе:** Тек үміткерге арналған шығарылым. Қазіргі уақытта ешбір RC емес жәшік қажетті API бетін ұсынбайды,
  сондықтан біз соңғы шығарылым үшін жоғары ағынды қадағалай отырып, тексерілу мүмкіндігі үшін ағаш ішіндегі жәшіктің айнасын көрсетеміз.
- **Бақылау пункттерін қарап шығу:**
  - Wycheproof жиынтығы мен TC26 құрылғыларына қарсы расталған хэш шығысы арқылы
    `cargo test -p iroha_crypto --features gost` (`crates/iroha_crypto/tests/gost_wycheproof.rs` қараңыз).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    ағымдағы тәуелділікпен әрбір TC26 қисығының жанында Ed25519/Secp256k1 жаттығуларын орындайды.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    жаңа өлшемдерді тіркелген медианалармен салыстырады (CI ішінде `--summary-only` пайдаланыңыз, қосыңыз
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` қайта базалау кезінде).
  - `scripts/gost_bench.sh` орындықты орады + ағынды тексеру; JSON жаңарту үшін `--write-baseline` өтіңіз.
    Үздік жұмыс процесі үшін `docs/source/crypto/gost_performance.md` қараңыз.
- **Бәсеңдету шаралары:** `streebog` пернелерді нөлге теңестіретін детерминирленген орауыштар арқылы ғана шақырылады;
  қол қоюшы апатты RNG сәтсіздігін болдырмау үшін операциялық жүйе энтропиясымен бейтараптарды хеджирлейді.
- **Келесі әрекеттер:** RustCrypto `0.11.x` шығарылымын орындаңыз; тег түскенде, емдеңіз
  стандартты тәуелділік соққысы ретінде жаңарту (бақылау сомасын тексеру, айырмашылықты қарап шығу, шығу тегін жазу және
  сатушы айнаны тастаңыз).