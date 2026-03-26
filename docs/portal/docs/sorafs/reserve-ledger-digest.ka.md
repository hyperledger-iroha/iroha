---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

Reserve+Rent პოლიტიკა (საგზაო რუკის ელემენტი **SFM‑6**) ახლა იგზავნება `sorafs reserve`
CLI დამხმარეები პლუს `scripts/telemetry/reserve_ledger_digest.py` მთარგმნელი ასე
სახაზინო გაშვებამ შეიძლება გამოყოს დეტერმინისტული იჯარა/რეზერვი. ეს გვერდი სარკეა
`docs/source/sorafs_reserve_rent_plan.md`-ში განსაზღვრული სამუშაო პროცესი და განმარტავს
როგორ გავატაროთ ახალი გადაცემის არხი Grafana + Alertmanager, ასე რომ, ეკონომიკა და
მმართველობის მიმომხილველებს შეუძლიათ ბილინგის ყველა ციკლის შემოწმება.

## სამუშაო პროცესი ბოლომდე

1. **ციტატა + წიგნის პროექცია **
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account soraカタカナ... \
    --treasury-account soraカタカナ... \
    --reserve-account soraカタカナ... \
    --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   წიგნის დამხმარე ანიჭებს `ledger_projection` ბლოკს (ქირა, რეზერვი
   დეფიციტი, შევსების დელტა, ანდერრაიტინგული ლოგინები) პლუს Norito `Transfer`
   ISI-ები საჭიროა XOR-ის გადასატანად სახაზინო და სარეზერვო ანგარიშებს შორის.

2. ** დაიჯესტის გენერირება + Prometheus/NDJSON გამოსავლები**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   დაიჯესტის დამხმარე აორმაგებს მიკრო-XOR ჯამებს XOR-ად, აფიქსირებს თუ არა
   პროექცია ხვდება ანდერრაიტინგს და გამოსცემს **გადაცემის არხს** მეტრიკას
   `sorafs_reserve_ledger_transfer_xor` და
   `sorafs_reserve_ledger_instruction_total`. როდესაც საჭიროა მრავალი წიგნი
   დამუშავებული (მაგ., პროვაიდერების ჯგუფი), გაიმეორეთ `--ledger`/`--label` წყვილი და
   დამხმარე წერს ერთ NDJSON/Prometheus ფაილს, რომელიც შეიცავს ყველა დაიჯესტს.
   დაფები შთანთქავს მთელ ციკლს შეკვეთილი წებოს გარეშე. `--out-prom`
   ფაილი მიზნად ისახავს კვანძის ექსპორტიორის ტექსტური ფაილის კოლექციონერს - ჩააგდეთ `.prom` ფაილი
   ექსპორტიორის ნანახი დირექტორია ან ატვირთეთ იგი ტელემეტრიის თაიგულში
   მოხმარებულია სარეზერვო დაფის სამუშაოს მიერ — ხოლო `--ndjson-out` იკვებება იგივე
   იტვირთება მონაცემთა მილსადენებში.

3. ** გამოაქვეყნეთ არტეფაქტები + მტკიცებულებები **
   - შეინახეთ დაიჯესტები `artifacts/sorafs_reserve/ledger/<provider>/` ქვეშ და ბმული
     მარკდაუნის შეჯამება თქვენი ყოველკვირეული ეკონომიკური ანგარიშიდან.
   - მიამაგრეთ JSON დაიჯესტი ქირის დაწვას (რათა აუდიტორებს შეეძლოთ ხელახლა დაკვრა
     მათემატიკა) და ჩართეთ საკონტროლო ჯამი მმართველობის მტკიცებულებების პაკეტში.
   - თუ დაიჯესტი მიუთითებს შევსების ან ანდერაიტის დარღვევის შესახებ, მიუთითეთ გაფრთხილება
     ID (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) და აღნიშნეთ რომელი იყო გადაცემის ISI
     მიმართა.

## მეტრიკა → დაფები → გაფრთხილებები

| წყაროს მეტრიკა | Grafana პანელი | გაფრთხილება / პოლიტიკის კაკალი | შენიშვნები |
|---------------|--------------|--------------------|------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | „DA Rent Distribution (XOR/საათი)“ `dashboards/grafana/sorafs_capacity_health.json`-ში | იკვებეთ ყოველკვირეული სახაზინო დაიჯესტი; სარეზერვო ნაკადის მწვერვალები ვრცელდება `SoraFSCapacityPressure`-ში (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | „სიმძლავრის გამოყენება (GiB-თვეები)“ (იგივე დაფა) | დაწყვილება ledger digest-თან, რათა დაამტკიცოს, რომ ინვოისის შენახვა შეესაბამება XOR გადარიცხვებს. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | „Reserve Snapshot (XOR)“ + სტატუსის ბარათები `dashboards/grafana/sorafs_reserve_economics.json`-ში | `SoraFSReserveLedgerTopUpRequired` ირთვება `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` ირთვება, როდესაც `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | „ტრანსფერები სახის მიხედვით“, „უახლესი ტრანსფერის განხილვა“ და დაფარვის ბარათები `dashboards/grafana/sorafs_reserve_economics.json`-ში | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` და `SoraFSReserveLedgerTopUpTransferMissing` აფრთხილებენ, როდესაც გადარიცხვის არხი არ არის ან ნულოვანია, მიუხედავად იმისა, რომ დაქირავება/დამატებაა საჭირო; დაფარვის ბარათები იმავე შემთხვევებში 0%-მდე ეცემა. |

როდესაც დაქირავების ციკლი დასრულდება, განაახლეთ Prometheus/NDJSON სნეპშოტები, დაადასტურეთ
რომ Grafana პანელები აიღებენ ახალ `label`-ს და ანიჭებენ ეკრანის სურათებს +
Alertmanager ID-ები ქირავნობის მართვის პაკეტზე. ეს ადასტურებს CLI პროექციას,
ტელემეტრია და მმართველობის არტეფაქტები მომდინარეობს **იგივე** გადაცემის წყაროდან და
ინახავს საგზაო რუქის ეკონომიკურ დაფებს Reserve+Rent-თან შესაბამისობაში
ავტომატიზაცია. დაფარვის ბარათებზე უნდა იკითხებოდეს 100% (ან 1.0) და ახალი გაფრთხილებები
უნდა გაიწმინდოს მას შემდეგ, რაც დაიჯესტში იქნება ქირის და რეზერვის შევსების გადარიცხვები.