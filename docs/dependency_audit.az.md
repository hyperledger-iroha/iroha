---
lang: az
direction: ltr
source: docs/dependency_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb0a770fac1086462d949dbf17dd5a05f133169e57d50b0d90ddb48ae05f2853
source_last_modified: "2026-01-05T09:28:11.822642+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Asılılıq auditinin xülasəsi

Tarix: 09-01-2025

Əhatə dairəsi: Cargo.toml fayllarında elan edilmiş və Cargo.lock-da həll edilmiş bütün qutuların iş sahəsi üzrə nəzərdən keçirilməsi. RustSec məsləhət DB-yə qarşı yük auditi və sandıq qanuniliyi və alqoritmlər üçün "əsas qutu" seçimləri üçün əl ilə nəzərdən keçirildi.

Alətlər/əmrlər işləyir:
- `cargo tree -d --workspace --locked --offline` – yoxlanılmış dublikat versiyalar
- `cargo audit` – məlum zəifliklər və qopan qutular üçün skan edilmiş Cargo.lock

Təhlükəsizliyə dair tövsiyələr tapıldı (indi 0 boşluq; 2 xəbərdarlıq):
- çarpaz kanal - RUSTSEC-2025-0024
  - Sabit: `crates/ivm/Cargo.toml`-də `0.5.15`-ə çarpdı.

  - Sabit: `crates/iroha_torii/Cargo.toml`-də `pprof`-i `prost-codec`-a çevirdi.

- üzük — RUSTSEC-2025-0009
  - Sabit: QUIC/TLS yığını (`quinn 0.11`, `rustls 0.23`, `tokio-rustls 0.26`) və yenilənmiş WS yığını `tungstenite/tokio-tungstenite 0.24`. `cargo update -p ring --precise 0.17.12` vasitəsilə `ring 0.17.12`-ə məcburi kilidləmə.

Qalan məsləhətlər: yoxdur. Qalan xəbərdarlıqlar: `backoff` (təxmin edilmir), `derivative` (saxlanılmır).

Qanunilik və "əsas qutu" qiymətləndirməsi (diqqət mərkəzində):
- Hashing: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak` (geniş istifadə olunur) — kanonik seçimlər.
- AEAD/Simmetrik: `aes-gcm`, `chacha20poly1305`, `aead` əlamətləri (RustCrypto) — kanonik.
- İmzalar/ECC: `ed25519-dalek`, `x25519-dalek` (dalek layihəsi), `k256` (RustCrypto), `secp256k1` (libsecp bağlamaları) — hamısı qanunidir; səth sahəsini azaltmaq üçün tək secp256k1 yığınına (saf Rust üçün `k256` və ya libsecp üçün `secp256k1`) üstünlük verin.
- BLS12-381/ZK: `blstrs`, `halo2_*` — istehsalatda geniş istifadə olunan ZK ekosistemləri; qanuni.
- PQ: `pqcrypto-dilithium`, `pqcrypto-traits` — qanuni istinad qutuları.
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — kanonik müasir Rust TLS yığını.
- Səs-küy: `snow` - kanonik tətbiq.
- Serializasiya: `parity-scale-codec` SCALE üçün kanonikdir. Serde iş sahəsində istehsal asılılıqlarından silindi; Norito hər bir iş vaxtı yolunu əhatə edir/yazarlar. İstənilən qalıq Serde istinadları tarixi sənədlərdə, qoruyucu skriptlərdə və ya yalnız sınaq icazəli siyahılarda yaşayır.
- FFI/libs: `libsodium-sys-stable`, `openssl` — qanuni; istehsal yollarında OpenSSL üzərində Rustls üstünlük verir (cari kod artıq edir).

Tövsiyələr:
- Ünvan xəbərdarlıqları:
  - `backoff`-i `retry`/`futures-retry` və ya yerli eksponensial geriləmə köməkçisi ilə əvəz etməyi düşünün.
  - `derivative` törəmələrini əl ilə işarələr və ya müvafiq hallarda `derive_more` ilə əvəz edin.
- Orta: dublikat tətbiqləri azaltmaq üçün mümkün olduqda `k256` və ya `secp256k1` üzərində birləşdirin (hər ikisini yalnız həqiqətən tələb olunduqda buraxın).
- Orta: ZK istifadəsi üçün mənşəyi `poseidon-primitives 0.2.0` nəzərdən keçirin; mümkün olarsa, paralel ekosistemləri minimuma endirmək üçün Arkworks/Halo2-yə məxsus Poseidon tətbiqi ilə uyğunlaşmağı düşünün.

Qeydlər:
- `cargo tree -d` gözlənilən dublikat əsas versiyaları (`bitflags` 1/2, çoxsaylı `ring`) göstərir, özlüyündə təhlükəsizlik riski deyil, tikinti səthini artırır.
- Yazı tipinə bənzər yeşiklər müşahidə edilməmişdir; bütün adlar və mənbələr tanınmış ekosistem qutularına və ya daxili iş sahəsi üzvlərinə həll edilir.
- Eksperimental: BLS-ni yalnız blstrs-a köçürməyə başlamaq üçün `iroha_crypto` funksiyası əlavə edildi (aktiv olduqda arkworks-dən asılılığı aradan qaldırır). Davranış/kodlaşdırma dəyişikliklərinin qarşısını almaq üçün defolt olaraq `w3f-bls` qalır. Uyğunlaşma planı:
  - `crates/iroha_crypto/tests/bls_backend_compat.rs`-də açarları bir dəfə əldə edən və `SecretKey`, `PublicKey` və imza birləşməsini əhatə edən hər iki arxa tərəfdə bərabərliyi təsdiq edən `crates/iroha_crypto/tests/bls_backend_compat.rs`-də gediş-gəliş qurğuları əlavə edin.

Təqiblər (təklif olunan iş elementləri):
- Serde qoruyucularını CI-də (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) saxlayın ki, yeni istehsal istifadələri tətbiq oluna bilməz.

Bu audit üçün sınaq aparıldı:
- Ən son məsləhət DB ilə `cargo audit` işlətdi; dörd məsləhəti və onların asılılıq ağaclarını yoxladı.
- Düzəltmə yerlərini müəyyən etmək üçün təsirə məruz qalan qutuların birbaşa asılılıq bəyannaməsi axtarıldı.