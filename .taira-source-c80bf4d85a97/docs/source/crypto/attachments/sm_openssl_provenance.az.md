---
lang: az
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
Yaradılan %: 2026-01-30

# Ətraf Mühit Xülasəsi

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: `LibreSSL 3.3.6` hesabatları (macOS-da sistem tərəfindən təmin edilən TLS alət dəsti).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: dəqiq Rustdan asılılıq yığını üçün baxın `sm_iroha_crypto_tree.txt` (`openssl` sandıq v0.10.74, `openssl-sys` v0.9.x, satıcı OpenSSL 3.100NI vasitəsilə mövcuddur; `pkg-config --modversion openssl` mənbəyi `vendored` xüsusiyyəti deterministik önizləmə quruluşları üçün `crates/iroha_crypto/Cargo.toml`-də aktivləşdirilib).

# Qeydlər

- LibreSSL başlıqlarına/kitabxanalarına qarşı yerli inkişaf mühiti əlaqələri; istehsalın önizləmə quruluşları OpenSSL >= 3.0.0 və ya Tongsuo 8.x istifadə etməlidir. Son artefakt paketini yaradan zaman sistem alət dəstini dəyişdirin və ya `OPENSSL_DIR`/`PKG_CONFIG_PATH` təyin edin.
- Dəqiq OpenSSL/Tongsuo tarball hashını (`openssl version -v`, `openssl version -b`, `openssl version -f`) ələ keçirmək üçün buraxılış qurma mühitində bu snapşotu yenidən yaradın və təkrarlana bilən qurma skripti/yoxlama məbləğini əlavə edin. Satıcı konstruksiyalar üçün Cargo tərəfindən istifadə edilən `openssl-src` sandıq versiyasını qeyd edin (`target/debug/build/openssl-sys-*/output`-də görünür).
- Apple Silicon hostları OpenSSL tüstü kəmərini işləyərkən `RUSTFLAGS=-Aunsafe-code` tələb edir ki, AArch64 SM3/SM4 sürətləndirici stubları tərtib etsin (intrinsiklər macOS-da mövcud deyil). `scripts/sm_openssl_smoke.sh` skripti CI və yerli qaçışları ardıcıl saxlamaq üçün `cargo`-ə müraciət etməzdən əvvəl bu bayrağı ixrac edir.
- Qablaşdırma boru kəməri bağlandıqdan sonra yuxarı mənbə mənşəyini (məsələn, `openssl-src-<ver>.tar.gz` SHA256) əlavə edin; CI artefaktlarında eyni hashdan istifadə edin.