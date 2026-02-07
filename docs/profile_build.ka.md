---
lang: ka
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# პროფილირება `iroha_data_model` Build

`iroha_data_model`-ში ნელი აშენების ნაბიჯების დასადგენად, გაუშვით დამხმარე სკრიპტი:

```sh
./scripts/profile_build.sh
```

ეს მუშაობს `cargo build -p iroha_data_model --timings` და წერს დროის ანგარიშებს `target/cargo-timings/`-ზე.
გახსენით `cargo-timing.html` ბრაუზერში და დაალაგეთ ამოცანები ხანგრძლივობის მიხედვით, რათა ნახოთ რომელ ყუთებს ან კონსტრუქციულ ნაბიჯებს სჭირდება ყველაზე მეტი დრო.

გამოიყენეთ დროები ოპტიმიზაციის ძალისხმევის ფოკუსირებისთვის ყველაზე ნელ ამოცანებზე.