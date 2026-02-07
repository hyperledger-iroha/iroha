---
lang: ka
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
% გენერირებული: 2026-01-30

# გარემოს შეჯამება

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: იუწყება `LibreSSL 3.3.6` (სისტემის მიერ მოწოდებული TLS ხელსაწყოთა ნაკრები macOS-ზე).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: იხილეთ `sm_iroha_crypto_tree.txt` ზუსტი Rust-ის დამოკიდებულების დასტასთვის (`openssl` crate v0.10.74, `openssl-sys` v0.9.x, გამყიდველი OpenSSL 3.08 I0800X. crate; ფუნქცია `vendored` ჩართულია `crates/iroha_crypto/Cargo.toml`-ში განმსაზღვრელი გადახედვისას).

# შენიშვნები

- ლოკალური განვითარების გარემოს ბმულები LibreSSL სათაურების/ბიბლიოთეკების წინააღმდეგ; წარმოების წინასწარი გადახედვის კონსტრუქციები უნდა გამოიყენონ OpenSSL >= 3.0.0 ან Tongsuo 8.x. შეცვალეთ სისტემის ხელსაწყოთა ნაკრები ან დააყენეთ `OPENSSL_DIR`/`PKG_CONFIG_PATH` საბოლოო არტეფაქტის ნაკრების გენერირებისას.
- განაახლეთ ეს სნეპშოტი გამოშვების build გარემოში, რათა აღბეჭდოთ ზუსტი OpenSSL/Tongsuo tarball ჰეში (`openssl version -v`, `openssl version -b`, `openssl version -f`) და მიამაგრეთ რეპროდუცირებადი build სკრიპტი/ჩეკის ჯამი. გაყიდვადი კონსტრუქციებისთვის ჩაწერეთ Cargo-ს მიერ გამოყენებული `openssl-src` კრატის ვერსია/დავალება (ხილულია `target/debug/build/openssl-sys-*/output`-ში).
- Apple Silicon-ის მასპინძლებს სჭირდებათ `RUSTFLAGS=-Aunsafe-code` OpenSSL კვამლის აღკაზმულობის გაშვებისას, ასე რომ, AArch64 SM3/SM4 აჩქარების ნაკრებები შედგენილია (შინაგანი მიუწვდომელია macOS-ზე). სკრიპტი `scripts/sm_openssl_smoke.sh` ახდენს ამ დროშის ექსპორტს `cargo`-ის გამოძახებამდე, რათა შეინარჩუნოს CI და ლოკალური გაშვებები თანმიმდევრული.
- მიამაგრეთ ზედა დინების წყაროს წარმოშობა (მაგ., `openssl-src-<ver>.tar.gz` SHA256) შეფუთვის მილსადენის დამაგრების შემდეგ; გამოიყენეთ იგივე ჰეში CI არტეფაქტებში.