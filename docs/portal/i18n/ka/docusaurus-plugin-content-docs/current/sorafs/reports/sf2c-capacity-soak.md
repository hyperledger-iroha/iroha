---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c სიმძლავრის დარიცხვის დატენვის ანგარიში

თარიღი: 2026-03-21

## სფერო

ეს ანგარიში აღრიცხავს დეტერმინისტულ SoraFS სიმძლავრის დარიცხვასა და ანაზღაურებას
ტესტები მოთხოვნილია SF-2c საგზაო რუქის მიხედვით.

- ** 30-დღიანი მრავალპროვაიდერის გაჟღენთვა:** ახორციელებს
  `capacity_fee_ledger_30_day_soak_deterministic` ინ
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  აღკაზმულობა ახდენს ხუთ პროვაიდერს, მოიცავს 30 დასახლების ფანჯარას და
  ადასტურებს, რომ წიგნის ჯამები ემთხვევა დამოუკიდებლად გამოთვლილ მითითებას
  პროექცია. ტესტი ასხივებს Blake3 დაიჯესტს (`capacity_soak_digest=...`).
  CI-ს შეუძლია გადაიღოს და განასხვავოს კანონიკური სნეპშოტი.
- **მიწოდების ქვეშ მყოფი ჯარიმები:** აღსრულებულია მიერ
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (იგივე ფაილი). ტესტი ადასტურებს დარტყმის ზღურბლებს, გაგრილებას, გირაოს დაწევას,
  და ledger მრიცხველები რჩება დეტერმინისტული.

## აღსრულება

გაუშვით დატენვის ვალიდაცია ადგილობრივად:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

ტესტები სრულდება ერთ წამში სტანდარტულ ლეპტოპზე და მოითხოვს არა
გარე მოწყობილობები.

## დაკვირვებადობა

Torii ახლა ამჟღავნებს პროვაიდერის საკრედიტო კადრებს საკომისიოს ჩანაწერებთან ერთად, ასე რომ, დაფები
შეუძლია კარიბჭე დაბალ ბალანსზე და პენალტების დარტყმებზე:

- დასვენება: `GET /v1/sorafs/capacity/state` აბრუნებს `credit_ledger[*]` ჩანაწერებს, რომლებიც
  ასახეთ ლეჯერის ველები, რომლებიც დამოწმებულია გაჟღენთის ტესტში. იხ
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana იმპორტი: `dashboards/grafana/sorafs_capacity_penalties.json` გამოსახავს
  ექსპორტირებული გაფიცვის მრიცხველები, ჯარიმის ჯარიმები და შემოტანილი გირაო ე.წ
  პერსონალს შეუძლია შეადაროს გაჟონვის საბაზისო ხაზები ცოცხალ გარემოსთან.

## შემდგომი

- დაგეგმეთ ყოველკვირეული კარიბჭე გადის CI-ში, რათა გაიმეოროთ გაჟღენთილი ტესტი (კვამლის დონე).
- გააფართოვეთ Grafana დაფა Torii დაფქული სამიზნეებით, წარმოების ტელემეტრიის შემდეგ
  ექსპორტი მიმდინარეობს.