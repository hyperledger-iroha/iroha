---
lang: ka
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Payload v1 გაშვების დამტკიცება (SDK საბჭო, 2026-04-28).
//!
//! ასახავს SDK საბჭოს გადაწყვეტილების მემორანდუმს, რომელიც მოითხოვს `roadmap.md:M1`-ს, ასე რომ
//! დაშიფრული payload v1 rollout-ს აქვს აუდიტის ჩანაწერი (მიწოდება M1.4).

# Payload v1 Rollout გადაწყვეტილება (2026-04-28)

- **თავმჯდომარე:** SDK საბჭოს ხელმძღვანელი (მ. ტაკემია)
- ** ხმის მიცემის წევრები: ** Swift Lead, CLI Maintainer, Confidential Assets TL, DevRel WG
- ** დამკვირვებლები: ** პროგრამა Mgmt, Telemetry Ops

## შეყვანები განხილულია

1. **Swift Bindings & Subscribers** — `ShieldRequest`/`UnshieldRequest`, ასინქრონული გამომგზავნი და Tx builder დამხმარეები პარიტეტული ტესტებით და დოკუმენტები.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI ერგონომიკა** — დამხმარე `iroha app zk envelope` მოიცავს სამუშაო ნაკადების კოდირება/შემოწმება და უკმარისობის დიაგნოსტიკა, რომელიც შეესაბამება საგზაო რუქის ერგონომიკის მოთხოვნას.【crates/iroha_cli/src/zk.rs:1256】
3. **დეტერმინისტული მოწყობილობები და პარიტეტული კომპლექტები** — საერთო მოწყობილობა + Rust/Swift ვალიდაცია Norito ბაიტის/შეცდომის ზედაპირების შესანარჩუნებლად გასწორებული.【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_en crypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## გადაწყვეტილება

- **დაამტკიცეთ payload v1 გაშვება** SDK-ებისთვის და CLI-ებისთვის, რაც საშუალებას აძლევს Swift-ის საფულეებს შექმნან კონფიდენციალური კონვერტები შეკვეთილი სანტექნიკის გარეშე.
- **პირობები:** 
  - შეინახეთ პარიტეტული მოწყობილობები CI დრიფტის გაფრთხილების ქვეშ (მიბმული `scripts/check_norito_bindings_sync.py`-თან).
  - ოპერაციული სათამაშო წიგნის დოკუმენტირება `docs/source/confidential_assets.md`-ში (უკვე განახლებულია Swift SDK PR-ის მეშვეობით).
  - ჩაწერეთ კალიბრაცია + ტელემეტრიული მტკიცებულება ნებისმიერი წარმოების დროშის ატრიალებამდე (მიმართული M2-ის ქვეშ).

## მოქმედების ელემენტი

| მფლობელი | ნივთი | ვადა |
|-------|------|-----|
| Swift Lead | გამოაცხადეთ GA ხელმისაწვდომობა + README ფრაგმენტები | 2026-05-01 |
| CLI Maintainer | დაამატეთ `iroha app zk envelope --from-fixture` დამხმარე (სურვილისამებრ) | ბექლოგი (არ იბლოკება) |
| DevRel WG | განაახლეთ საფულის სწრაფი სტარტები payload v1 ინსტრუქციებით | 2026-05-05 |

> **შენიშვნა:** ეს ჩანაწერი ანაცვლებს `roadmap.md:2426`-ში დროებით „საბჭოთა დამტკიცების მოლოდინში“ გამოძახებას და აკმაყოფილებს ტრეკერის პუნქტს M1.4. განაახლეთ `status.md`, როდესაც შემდგომი ქმედებების ერთეულები იხურება.