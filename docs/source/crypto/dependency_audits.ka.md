---
lang: ka
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# კრიპტო დამოკიდებულების აუდიტი

## Streebog (`streebog` crate)

- **ვერსია ხეში:** `0.11.0-rc.2` გაყიდვადი ქვეშ `vendor/streebog` (გამოიყენება, როდესაც ჩართულია `gost` ფუნქცია).
- **მომხმარებელი:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + შეტყობინების ჰეშირება).
- ** სტატუსი: ** მხოლოდ გათავისუფლების კანდიდატი. არცერთი არა-RC ყუთი ამჟამად არ გვთავაზობს საჭირო API ზედაპირს,
  ასე რომ, ჩვენ ასახავს კრატის ხეს აუდიტორობისთვის, ხოლო ჩვენ ვაკონტროლებთ ზემოთ დინებაში საბოლოო გამოშვებისთვის.
- ** გადახედეთ საგუშაგოებს:**
  - დამოწმებული ჰეშის გამომავალი Wycheproof კომპლექტის და TC26 მოწყობილობების საშუალებით
    `cargo test -p iroha_crypto --features gost` (იხ. `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    ავარჯიშებს Ed25519/Secp256k1 ყოველი TC26 მრუდის გვერდით მიმდინარე დამოკიდებულებით.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    ადარებს ახალ გაზომვებს შემოწმებულ მედიანასთან (გამოიყენეთ `--summary-only` CI-ში, დაამატეთ
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` ხელახალი ბაზირებისას).
  - `scripts/gost_bench.sh` ახვევს სკამს + გამშვები ნაკადი; გაიარეთ `--write-baseline` JSON-ის განახლებისთვის.
    იხილეთ `docs/source/crypto/gost_performance.md` ბოლოდან ბოლომდე სამუშაო ნაკადისთვის.
- **შემარბილებელი ღონისძიებები:** `streebog` გამოიყენება მხოლოდ დეტერმინისტული შეფუთვების საშუალებით, რომლებიც ნულოვანი კლავიშები არიან;
  ხელმომწერი ჰეჯირებს არასანქციებს OS ენტროპიით, რათა თავიდან აიცილოს კატასტროფული RNG უკმარისობა.
- **შემდეგი მოქმედებები:** მიჰყევით RustCrypto-ს streebog `0.11.x` გამოშვებას; ერთხელ tag მიწები, მკურნალობა
  განაახლეთ, როგორც სტანდარტული დამოკიდებულების ბამპი (დაამოწმეთ საკონტროლო ჯამი, გადახედეთ განსხვავებას, ჩაწერეთ წარმოშობა და
  ჩამოაგდე გამყიდველი სარკე).