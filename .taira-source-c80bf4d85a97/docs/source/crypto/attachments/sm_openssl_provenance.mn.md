---
lang: mn
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
% Үүсгэсэн: 2026-01-30

# Байгаль орчны тойм

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: `LibreSSL 3.3.6` тайлангууд (macOS дээр системээр хангагдсан TLS хэрэгсэл).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: Зэврэлтээс хамааралтай стекийг `sm_iroha_crypto_tree.txt`-с харна уу (`openssl` хайрцаг v0.10.74, `openssl-sys` v0.9.x, `openssl-sys` v0.9.x, `sm_iroha_crypto_tree.txt`; `pkg-config --modversion openssl` эх үүсвэрээр дамжуулан худалдаалж байна. `vendored` функцийг `crates/iroha_crypto/Cargo.toml` дээр тодорхойлогч урьдчилан үзэх бүтээцэд зориулж идэвхжүүлсэн).

# Тэмдэглэл

- LibreSSL толгой/номын сангийн эсрэг орон нутгийн хөгжлийн орчны холбоосууд; Үйлдвэрлэлийг урьдчилан үзэхэд OpenSSL >= 3.0.0 эсвэл Tongsuo 8.x ашиглах ёстой. Эцсийн олдворын багцыг үүсгэх үед системийн хэрэгслийг солих эсвэл `OPENSSL_DIR`/`PKG_CONFIG_PATH` тохируулна уу.
- Яг OpenSSL/Tongsuo tarball хэшийг (`openssl version -v`, `openssl version -b`, `openssl version -f`) авахын тулд хувилбар бүтээх орчин дотор энэ хормын хувилбарыг сэргээж, хуулбарлах скрипт/шалгах нийлбэрийг хавсаргана уу. Борлуулсан бүтээцүүдийн хувьд Cargo-д ашигладаг `openssl-src` хайрцагны хувилбарыг тэмдэглэнэ үү (`target/debug/build/openssl-sys-*/output` дээр харагдана).
- Apple Silicon хостууд нь OpenSSL утааны бэхэлгээг ажиллуулахдаа `RUSTFLAGS=-Aunsafe-code`-ийг шаарддаг тул AArch64 SM3/SM4 хурдатгалын бүдүүвчийг эмхэтгэдэг (үндсэн шинж чанарууд нь macOS дээр боломжгүй). `scripts/sm_openssl_smoke.sh` скрипт нь CI болон орон нутгийн гүйлтийг тогтвортой байлгахын тулд `cargo`-г дуудахын өмнө энэ тугийг экспортлодог.
- Сав баглаа боодлын шугамыг бэхэлсэний дараа эх үүсвэрийн эх сурвалжийг (жишээ нь, `openssl-src-<ver>.tar.gz` SHA256) хавсаргана; CI олдворуудад ижил хэшийг ашиглах.