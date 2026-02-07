---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ffbb145e1e0aa9dc71bdb6896c4f8be69eb6226194c5c165905af1ac243cc9
source_last_modified: "2025-12-29T18:16:35.199832+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
---

# SoraFS სიმძლავრის ბაზრის გადამოწმების ჩამონათვალი

** განხილვის ფანჯარა: ** 2026-03-18 → 2026-03-24  
** პროგრამის მფლობელები: ** შენახვის გუნდი (`@storage-wg`), მმართველი საბჭო (`@council`), სახაზინო გილდია (`@treasury`)  
** ფარგლები: ** SF-2c GA-სთვის საჭირო საბორტო მილსადენების, დავების განხილვის ნაკადების და ხაზინის შერიგების პროცესების მიმწოდებელი.

გარე ოპერატორებისთვის ბაზრის ჩართვამდე უნდა გადაიხედოს ქვემოთ მოცემული ჩამონათვალი. თითოეული სტრიქონი უკავშირდება განმსაზღვრელ მტკიცებულებებს (ტესტები, მოწყობილობები ან დოკუმენტაცია), რომელიც აუდიტორებს შეუძლიათ ხელახლა გაიმეორონ.

## მიღების ჩამონათვალი

### პროვაიდერის ჩართვა

| შემოწმება | ვალიდაცია | მტკიცებულება |
|-------|------------|----------|
| რეესტრი იღებს კანონიკურ ქმედუნარიანობის დეკლარაციებს | ინტეგრაციის ტესტის სავარჯიშოები `/v1/sorafs/capacity/declare` აპლიკაციის API-ს მეშვეობით, ხელმოწერის დამუშავების, მეტამონაცემების აღების და კვანძების რეესტრში გადაცემის გადამოწმებას. | `crates/iroha_torii/src/routing.rs:7654` |
| ჭკვიანი კონტრაქტი უარყოფს შეუსაბამო დატვირთვას | ერთეულის ტესტი უზრუნველყოფს პროვაიდერის პირადობის მოწმობებს და მოწოდებულ GiB ველებს ემთხვევა ხელმოწერილ დეკლარაციას გაგრძელებამდე. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI ასხივებს კანონიკურ ბორტზე არტეფაქტებს | CLI აღკაზმულობა წერს განმსაზღვრელ Norito/JSON/Base64 გამოსავალს და ამოწმებს ორმხრივ მგზავრობას, რათა ოპერატორებს შეეძლოთ დეკლარაციები ხაზგარეშე დადგმა. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| ოპერატორის გზამკვლევი ასახავს დაშვების სამუშაო პროცესს და მართვის ღობეებს | დოკუმენტაცია ჩამოთვლის დეკლარაციის სქემას, პოლიტიკის ნაგულისხმევს და საკრებულოს განხილვის ნაბიჯებს. | `../storage-capacity-marketplace.md` |

### დავის გადაწყვეტა

| შემოწმება | ვალიდაცია | მტკიცებულება |
|-------|------------|----------|
| დავის ჩანაწერები გრძელდება კანონიკური დატვირთვის დაიჯესტით | ერთეულის ტესტი აღრიცხავს დავას, დეკოდირებს შენახულ დატვირთვას და ამტკიცებს მომლოდინე სტატუსს, რათა გარანტირებული იყოს წიგნის დეტერმინიზმი. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI დავის გენერატორი შეესაბამება კანონიკურ სქემას | CLI ტესტი მოიცავს Base64/Norito გამოსავლებს და JSON შეჯამებებს `CapacityDisputeV1`-ისთვის, რაც უზრუნველყოფს მტკიცებულებების შეფუთვას დეტერმინისტულად. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| განმეორებითი ტესტი ადასტურებს დავა/ჯარიმების დეტერმინიზმს | ორჯერ გამეორებული ტელემეტრიის დამადასტურებელი მარცხი წარმოქმნის იდენტურ წიგნს, კრედიტს და დავის კადრებს, ამიტომ ხაზები განმსაზღვრელია თანატოლებში. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook დოკუმენტების ესკალაციისა და გაუქმების ნაკადი | ოპერაციების სახელმძღვანელო აღწერს საბჭოს სამუშაო პროცესს, მტკიცებულების მოთხოვნებს და უკან დაბრუნების პროცედურებს. | `../dispute-revocation-runbook.md` |

### სახაზინო შერიგება

| შემოწმება | ვალიდაცია | მტკიცებულება |
|-------|------------|----------|
| ლეჯერის დარიცხვა შეესაბამება 30-დღიან გაჟღენთვას პროექციას | Soak ტესტი მოიცავს ხუთ პროვაიდერს 30 ანგარიშსწორების ფანჯარაში, განასხვავებს წიგნის ჩანაწერებს მოსალოდნელი გადახდის მითითებით. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| ლეჯერის საექსპორტო შეჯერება დაფიქსირდა ღამით | `capacity_reconcile.py` ადარებს საკომისიო წიგნის მოლოდინებს განხორციელებულ XOR გადარიცხვის ექსპორტთან, გამოსცემს Prometheus მეტრიკას და ახორციელებს ხაზინის დამტკიცებას Alertmanager-ის მეშვეობით. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| ბილინგის დაფების ზედაპირული ჯარიმები და დარიცხვის ტელემეტრია | Grafana იმპორტის ნაკვეთები ასახავს GiB·საათის დარიცხვას, გაფიცვის მრიცხველებს და შეკრულ გირაოს მოწოდების ხილვადობისთვის. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| გამოქვეყნებული მოხსენების არქივები ასუფთავებს მეთოდოლოგიას და ბრძანებებს განმეორებით | მოხსენების დეტალების გაჟღენთვა, შესრულების ბრძანებები და დაკვირვებადობის კაკვები აუდიტორებისთვის. | `./sf2c-capacity-soak.md` |

## აღსრულების შენიშვნები

ხელახლა გაუშვით ვალიდაციის ნაკრები გაფორმებამდე:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

ოპერატორებმა უნდა განაახლონ საბორტო/დავის მოთხოვნის დატვირთვა `sorafs_manifest_stub capacity {declaration,dispute}`-ით და დაარქივონ მიღებული JSON/Norito ბაიტი მმართველობის ბილეთთან ერთად.

## ხელმოწერის არტეფაქტები

| არტეფაქტი | ბილიკი | blake2b-256 |
|----------|------|-------------|
| პროვაიდერის შეყვანის დამტკიცების პაკეტი | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| დავის გადაწყვეტის დამტკიცების პაკეტი | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| სახაზინო შერიგების დამტკიცების პაკეტი | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

შეინახეთ ამ არტეფაქტების ხელმოწერილი ასლები გამოშვების პაკეტთან და დააკავშირეთ ისინი მმართველობის ცვლილების ჩანაწერში.

## დამტკიცებები

- შენახვის გუნდის წამყვანი — @storage-tl (2026-03-24)  
- მმართველი საბჭოს მდივანი — @council-sec (2026-03-24)  
- სახაზინო ოპერაციების ლიდერი — @treasury-ops (2026-03-24)