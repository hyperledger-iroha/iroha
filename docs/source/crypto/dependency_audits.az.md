---
lang: az
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kripto Asılılıq Auditləri

## Streebog (`streebog` qutu)

- **Ağacdakı versiya:** `0.11.0-rc.2` `vendor/streebog` altında satılır (`gost` funksiyası aktiv olduqda istifadə olunur).
- **İstehlakçı:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + mesaj hashing).
- **Statusu:** Yalnız buraxılış-namizəd. Hazırda heç bir RC qutusu tələb olunan API səthini təklif etmir,
  Beləliklə, biz son buraxılış üçün yuxarı axını izləyərkən, audit üçün ağacda olan qutunu əks etdiririk.
- **Yoxlama məntəqələrini nəzərdən keçirin:**
  - Wycheproof dəsti və vasitəsilə TC26 qurğularına qarşı təsdiqlənmiş hash çıxışı
    `cargo test -p iroha_crypto --features gost` (bax `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    cari asılılıqla hər TC26 əyrisi ilə yanaşı Ed25519/Secp256k1 məşq edir.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    daha təzə ölçmələri yoxlanılmış medianlarla müqayisə edir (CI-də `--summary-only` istifadə edin, əlavə edin
    Yenidən əsaslandırma zamanı `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json`).
  - `scripts/gost_bench.sh` dəzgahı sarır + axını yoxlayın; JSON-u yeniləmək üçün `--write-baseline` keçir.
    Başdan sona iş axını üçün `docs/source/crypto/gost_performance.md`-ə baxın.
- **Mütəxəssislər:** `streebog` yalnız açarları sıfırlayan deterministik sarğılar vasitəsilə işə salınır;
  imzalayan fəlakətli RNG uğursuzluğunun qarşısını almaq üçün OS entropiyası ilə nonces hedcinq edir.
- **Növbəti tədbirlər:** RustCrypto-nun streebog `0.11.x` buraxılışını izləyin; etiket torpaqları sonra, müalicə
  standart asılılıq zərbəsi kimi təkmilləşdirin (yoxlama məbləğini yoxlayın, fərqi nəzərdən keçirin, mənşəyi qeyd edin və
  satıcı güzgünü buraxın).