---
lang: kk
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo Provenance суреті
% Жасалған: 2026-01-30

# Қоршаған орта туралы қорытынды

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: `LibreSSL 3.3.6` есептер (macOS жүйесінде жүйемен қамтамасыз етілген TLS құралдар жинағы).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: дәл Rust тәуелділік стегі үшін `sm_iroha_crypto_tree.txt` қараңыз (`openssl` жәшігі v0.10.74, `openssl-sys` v0.9.x, жеткізуші OpenSSL 3.1000NI арқылы қолжетімді; `vendored` мүмкіндігі детерминирленген алдын ала қарау құрастырулары үшін `crates/iroha_crypto/Cargo.toml` жүйесінде қосылған).

# Ескерту

- LibreSSL тақырыптарына/кітапханаларына қарсы жергілікті даму ортасының сілтемелері; өндірісті алдын ала қарау құрастырулары OpenSSL >= 3.0.0 немесе Tongsuo 8.x пайдалануы керек. Жүйенің құралдар жинағын ауыстырыңыз немесе соңғы артефакт бумасын жасау кезінде `OPENSSL_DIR`/`PKG_CONFIG_PATH` орнатыңыз.
- Нақты OpenSSL/Tongsuo tarball хэшін (`openssl version -v`, `openssl version -b`, `openssl version -f`) түсіру үшін шығарылымды құрастыру ортасында осы суретті қайта жасаңыз және қайталанатын құрастыру сценарийін/бақылау сомасын тіркеңіз. Сатылатын құрылымдар үшін Cargo пайдаланатын `openssl-src` жәшік нұсқасын/міндеттемесін жазыңыз (`target/debug/build/openssl-sys-*/output` ішінде көрінеді).
- Apple Silicon хосттары OpenSSL түтін қондырғысын іске қосқан кезде `RUSTFLAGS=-Aunsafe-code` талап етеді, осылайша AArch64 SM3/SM4 жеделдету түтіктері құрастырылады (ішкі элементтер macOS жүйесінде қолжетімсіз). `scripts/sm_openssl_smoke.sh` сценарийі CI және жергілікті іске қосуларды дәйекті сақтау үшін `cargo` шақыру алдында осы жалауды экспорттайды.
- Қаптама құбыры бекітілгеннен кейін жоғары ағын көзінің шығу тегін (мысалы, `openssl-src-<ver>.tar.gz` SHA256) бекітіңіз; CI артефактілерінде бірдей хэшті пайдаланыңыз.