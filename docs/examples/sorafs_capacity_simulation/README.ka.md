---
lang: ka
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS სიმძლავრის სიმულაციის ინსტრუმენტარიუმი

ეს დირექტორია აგზავნის რეპროდუცირებადი არტეფაქტებს SF-2c სიმძლავრის ბაზრისთვის
სიმულაცია. ინსტრუმენტთა ნაკრები ახორციელებს კვოტების მოლაპარაკებას, მარცხის დამუშავებას და შემცირებას
გამოსწორება წარმოების CLI დამხმარეების და მსუბუქი ანალიზის სკრიპტის გამოყენებით.

## წინაპირობები

- Rust toolchain, რომელსაც შეუძლია `cargo run` აწარმოოს სამუშაო სივრცის წევრებისთვის.
- Python 3.10+ (მხოლოდ სტანდარტული ბიბლიოთეკა).

## სწრაფი დაწყება

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` სკრიპტი იწვევს `sorafs_manifest_stub capacity`-ს ასაგებად:

- განმსაზღვრელი პროვაიდერის დეკლარაციები კვოტების მოლაპარაკების სტანდარტების ნაკრებისთვის.
- რეპლიკაციის ბრძანება, რომელიც შეესაბამება მოლაპარაკების სცენარს.
- ტელემეტრიული კადრები ჩავარდნილი ფანჯრისთვის.
- დავის ტვირთამწეობა, რომელიც იჭერს შემცირების მოთხოვნას.

სკრიპტი წერს Norito ბაიტს (`*.to`), base64 payloads (`*.b64`), Torii მოთხოვნას
სხეულები და ადამიანის მიერ წასაკითხი რეზიუმეები (`*_summary.json`) არჩეული არტეფაქტის ქვეშ
დირექტორია.

`analyze.py` მოიხმარს გენერირებულ შეჯამებებს, აწარმოებს გაერთიანებულ ანგარიშს
(`capacity_simulation_report.json`) და გამოსცემს Prometheus ტექსტურ ფაილს
(`capacity_simulation.prom`) ტარება:

- `sorafs_simulation_quota_*` ლიანდაგები, რომლებიც აღწერს მოლაპარაკების სიმძლავრეს და განაწილებას
  გაზიარება თითო პროვაიდერზე.
- `sorafs_simulation_failover_*` ლიანდაგები, რომლებიც ხაზს უსვამს შეფერხების დელტას და არჩეულს
  შემცვლელი პროვაიდერი.
- `sorafs_simulation_slash_requested` აღრიცხავს ამოღებული გამოსწორების პროცენტს
  დავის დატვირთვიდან.

Grafana პაკეტის იმპორტი `dashboards/grafana/sorafs_capacity_simulation.json`-ში
და მიუთითეთ ის Prometheus მონაცემთა წყაროზე, რომელიც ასუფთავებს გენერირებულ ტექსტურ ფაილს (
მაგალითი კვანძის ექსპორტიორის ტექსტური ფაილის კოლექციონერის მეშვეობით). Runbook at
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` გადის სრულად
სამუშაო პროცესი, მათ შორის Prometheus კონფიგურაციის რჩევები.

## მოწყობილობები

- `scenarios/quota_negotiation/` — პროვაიდერის დეკლარაციის სპეციფიკაციები და რეპლიკაციის შეკვეთა.
- `scenarios/failover/` — ტელემეტრიული ფანჯრები პირველადი გათიშვისა და ავარიული ამწევისთვის.
- `scenarios/slashing/` — დავის სპეციფიკა, რომელიც მიუთითებს იგივე რეპლიკაციის თანმიმდევრობაზე.

ეს მოწყობილობები დამოწმებულია `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`-ში
იმის გარანტია, რომ ისინი დარჩებიან სინქრონიზებული CLI სქემასთან.