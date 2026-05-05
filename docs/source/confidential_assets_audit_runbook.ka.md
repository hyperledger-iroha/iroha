---
lang: ka
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! კონფიდენციალური აქტივების აუდიტისა და ოპერაციების სახელმძღვანელო მითითებულ `roadmap.md:M4`-ის მიერ.

# კონფიდენციალური აქტივების აუდიტი და ოპერაციების წიგნაკი

ეს სახელმძღვანელო აერთიანებს იმ მტკიცებულებებს, რომლებსაც ეყრდნობიან აუდიტორები და ოპერატორები
კონფიდენციალური აქტივების ნაკადების დამოწმებისას. ის ავსებს როტაციის სათამაშო წიგნს
(`docs/source/confidential_assets_rotation.md`) და კალიბრაციის წიგნი
(`docs/source/confidential_assets_calibration.md`).

## 1. შერჩევითი გამჟღავნება და მოვლენის არხები

- ყოველი კონფიდენციალური ინსტრუქცია ასხივებს სტრუქტურირებულ `ConfidentialEvent` დატვირთვას
  (`Shielded`, `Transferred`, `Unshielded`) გადაღებული
  `crates/iroha_data_model/src/events/data/events.rs:198` და სერიული
  შემსრულებლები (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  რეგრესიის ნაკრები ახორციელებს ბეტონის დატვირთვას, რათა აუდიტორებს დაეყრდნონ
  დეტერმინისტული JSON განლაგება (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii ავლენს ამ მოვლენებს სტანდარტული SSE/WebSocket მილსადენის მეშვეობით; აუდიტორები
  გამოწერა `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) გამოყენებით,
  სურვილისამებრ სკუპირება ერთი აქტივის განსაზღვრებამდე. CLI მაგალითი:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }'
  ```

- პოლიტიკის მეტამონაცემები და მომლოდინე გადასვლები ხელმისაწვდომია მეშვეობით
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), ასახულია Swift SDK-ით
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) და დოკუმენტირებულია
  როგორც კონფიდენციალური აქტივების დიზაინი, ასევე SDK სახელმძღვანელო
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. ტელემეტრია, დაფები და კალიბრაციის მტკიცებულებები

- გაშვების მეტრიკის ზედაპირის ხის სიღრმე, ვალდებულება/საზღვრის ისტორია, ფესვის გამოდევნა
  მრიცხველები და დამადასტურებელი ქეშის დარტყმის კოეფიციენტები
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Grafana დაფები შეყვანილია
  `dashboards/grafana/confidential_assets.json` გაგზავნის ასოცირებული პანელები და
  გაფრთხილებები, `docs/source/confidential_assets.md:401`-ში დოკუმენტირებული სამუშაო პროცესით.
- კალიბრაციის გაშვებები (NS/op, gas/op, ns/gas) ხელმოწერილი ჟურნალებით ცოცხალი
  `docs/source/confidential_assets_calibration.md`. Apple-ის უახლესი სილიკონი
  NEON გაშვება დაარქივებულია
  `docs/source/confidential_assets_calibration_neon_20260428.log` და იგივე
  წიგნი იწერს დროებით შეწყვეტას SIMD-ნეიტრალური და AVX2 პროფილებისთვის, სანამ
  x86 ჰოსტები შემოდის ონლაინ.

## 3. ინციდენტზე რეაგირება და ოპერატორის ამოცანები

- როტაციის/განახლების პროცედურები მოქმედებს
  `docs/source/confidential_assets_rotation.md`, რომელიც მოიცავს ახლის დადგმას
  პარამეტრების პაკეტები, დაგეგმეთ პოლიტიკის განახლებები და აცნობეთ საფულეებს/აუდიტორებს. The
  ტრეკერის (`docs/source/project_tracker/confidential_assets_phase_c.md`) სიები
  runbook მფლობელები და რეპეტიციების მოლოდინები.
- წარმოების რეპეტიციებისთვის ან საგანგებო ფანჯრებისთვის, ოპერატორები ამაგრებენ მტკიცებულებებს
  `status.md` ჩანაწერები (მაგ., მრავალ ზოლიანი რეპეტიციის ჟურნალი) და მოიცავს:
  `curl` პოლიტიკის გადასვლების მტკიცებულება, Grafana სნეპშოტები და შესაბამისი მოვლენა
  შეიმუშავებს ისე, რომ აუდიტორებმა შეძლონ პიტნის → გადაცემის → ვადების გამოვლენა.

## 4. გარე მიმოხილვის კადენცია

- უსაფრთხოების მიმოხილვის ფარგლები: კონფიდენციალური სქემები, პარამეტრების რეესტრები, პოლიტიკა
  გადასვლები და ტელემეტრია. ეს დოკუმენტი პლუს კალიბრაციის წიგნის ფორმები
  გამყიდველებისთვის გაგზავნილი მტკიცებულებათა პაკეტი; მიმოხილვის განრიგის თვალყურის დევნება ხდება მეშვეობით
  M4 `docs/source/project_tracker/confidential_assets_phase_c.md`-ში.
- ოპერატორებმა უნდა განაახლონ `status.md` ნებისმიერი გამყიდველის დასკვნით ან შემდგომი დაკვირვებით
  სამოქმედო ნივთები. გარე მიმოხილვის დასრულებამდე, ეს სახელმძღვანელო ემსახურება როგორც
  ოპერაციულ საბაზისო აუდიტორებს შეუძლიათ ტესტირება.