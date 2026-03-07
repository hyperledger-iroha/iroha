---
lang: ka
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IH58 Rollout Note SDK & Codec მფლობელებისთვის

გუნდები: Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec tooling

კონტექსტი: `docs/account_structure.md` ახლა ასახავს მიწოდების IH58 ანგარიშის ID-ს
განხორციელება. გთხოვთ, შეუსაბამოთ SDK ქცევა და ტესტები კანონიკურ სპეციფიკას.

ძირითადი მითითებები:
- მისამართის კოდეკი + სათაურის განლაგება — `docs/account_structure.md` §2
- მრუდის რეესტრი — `docs/source/references/address_curve_registry.md`
- ნორმის v1 დომენის მართვა — `docs/source/references/address_norm_v1.md`
- სამაგრის ვექტორები — `fixtures/account/address_vectors.json`

მოქმედების ელემენტები:
1. **კანონიკური გამომავალი:** `AccountId::to_string()`/დისპლეი უნდა გამოსცეს მხოლოდ IH58
   (არა `@domain` სუფიქსი). Canonical hex განკუთვნილია გამართვისთვის (`0x...`).
2. **მიღებული შეყვანები:** პარსერებმა უნდა მიიღონ IH58 (სასურველია), `sora` შეკუმშული,
   და კანონიკური თექვსმეტობითი (მხოლოდ `0x...`; შიშველი თექვსმეტი უარყოფილია). შეყვანები შეიძლება იყოს
   `@<domain>` სუფიქსი მარშრუტიზაციის მინიშნებებისთვის; `<label>@<domain>` მეტსახელები მოითხოვს ა
   გადამწყვეტი.  3. **გამხსნელები:** დომენის გარეშე IH58/sora ანალიზს სჭირდება დომენის სელექტორი
   გადამწყვეტი, თუ სელექტორი არ არის ნაგულისხმევი ნაგულისხმევი (გამოიყენეთ კონფიგურირებული ნაგულისხმევი
   დომენის ეტიკეტი). UAID (`uaid:...`) და გაუმჭვირვალე (`opaque:...`) ლიტერალები მოითხოვს
   გადამწყვეტები.
4. **IH58 საკონტროლო ჯამი:** გამოიყენეთ Blake2b-512 `IH58PRE || prefix || payload`-ზე, აიღეთ
   პირველი 2 ბაიტი. შეკუმშული ანბანის საფუძველია **105**.
5. **მრუდის კარიბჭე:** SDK ნაგულისხმევად არის მხოლოდ Ed25519. მიაწოდეთ მკაფიო არჩევა
   ML‑DSA/GOST/SM (Swift build flags; JS/Android `configureCurveSupport`). გააკეთე
   არ ვივარაუდოთ, რომ secp256k1 ჩართულია ნაგულისხმევად Rust-ის გარეთ.
6. **არ არის CAIP-10:** ჯერ არ არის გაგზავნილი CAIP‑10 რუკა; არ გამოამჟღავნოს ან
   დამოკიდებულია CAIP-10 კონვერტაციებზე.

გთხოვთ, დაადასტუროთ კოდეკების/ტესტების განახლების შემდეგ; ღია კითხვების თვალყურის დევნება შესაძლებელია
ანგარიშის მისამართების RFC თემაში.