---
id: nexus-elastic-lane
lang: ka
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
ეს გვერდი ასახავს `docs/source/nexus_elastic_lane.md`-ს. შეინახეთ ორივე ასლი გასწორებული მანამ, სანამ თარგმანის გაწმენდა არ მოხვდება პორტალში.
:::

# ელასტიური ზოლის უზრუნველყოფის ხელსაწყოების ნაკრები (NX-7)

> **საგზაო რუკის პუნქტი:** NX-7 — ელასტიური ზოლის უზრუნველყოფის ხელსაწყოები  
> **სტატუსი:** Tooling დასრულებულია — წარმოქმნის მანიფესტებს, კატალოგის ფრაგმენტებს, Norito დატვირთვას, კვამლის ტესტებს,
> და დატვირთვის ტესტის პაკეტის დამხმარე ახლა კერავს სლოტის შეყოვნებას
> load runs შეიძლება გამოქვეყნდეს შეკვეთილი სკრიპტის გარეშე.

ეს გზამკვლევი აცნობს ოპერატორებს ახალი `scripts/nexus_lane_bootstrap.sh` დამხმარე საშუალებით, რომელიც ავტომატიზირებს
ზოლის მანიფესტის გენერირება, ზოლის/მონაცემთა სივრცის კატალოგის ფრაგმენტები და გამოქვეყნების მტკიცებულებები. მიზანი არის გაკეთება
ადვილია დაატრიალოთ ახალი Nexus ბილიკები (საჯარო ან პირადი) მრავალი ფაილის ხელით რედაქტირების გარეშე ან
კატალოგის გეომეტრიის ხელახლა გამოტანა ხელით.

## 1. წინაპირობები

1. მმართველობის დამტკიცება ზოლის მეტსახელისთვის, მონაცემთა სივრცისთვის, ვალიდატორის ნაკრებისთვის, შეცდომის ტოლერანტობისთვის (`f`) და ანგარიშსწორების პოლიტიკაზე.
2. დასრულებული ვალიდატორების სია (ანგარიშის ID) და დაცული სახელთა სივრცის სია.
3. წვდომა კვანძის კონფიგურაციის საცავზე, ასე რომ თქვენ შეგიძლიათ დაურთოთ გენერირებული ფრაგმენტები.
4. ბილიკები ხაზის მანიფესტის რეესტრისთვის (იხ. `nexus.registry.manifest_directory` და
   `cache_directory`).
5. ტელემეტრიული კონტაქტები/PagerDuty სახელურები ზოლისთვის, რათა სიგნალიზაცია შესაძლებელი იყოს ზოლის გავლისთანავე
   შემოდის ონლაინ.

## 2. შექმენით ზოლის არტეფაქტები

გაუშვით დამხმარე საცავის ფესვიდან:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

ძირითადი დროშები:

- `--lane-id` უნდა ემთხვეოდეს ახალი ჩანაწერის ინდექსს `nexus.lane_catalog`-ში.
- `--dataspace-alias` და `--dataspace-id/hash` აკონტროლებენ მონაცემთა სივრცის კატალოგის ჩანაწერს (ნაგულისხმევი
  ზოლის id როდესაც გამოტოვებულია).
- `--validator` შეიძლება განმეორდეს ან მიიღება `--validators-file`-დან.
- `--route-instruction` / `--route-account` გამოსცემს მზა მარშრუტიზაციის წესებს.
- `--metadata key=value` (ან `--telemetry-contact/channel/runbook`) აიღეთ runbook კონტაქტები ასე
  დაფებზე დაუყოვნებლივ ჩამოთვლილია უფლება მფლობელები.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` დაამატე გაშვების დროის განახლების კაუჭი მანიფესტს
  როდესაც ზოლი მოითხოვს ოპერატორის გაფართოებულ კონტროლს.
- `--encode-space-directory` ავტომატურად იწვევს `cargo xtask space-directory encode`-ს. დააწყვილეთ იგი
  `--space-directory-out` როდესაც გსურთ კოდირებული `.to` ფაილი ნაგულისხმევის გარდა სხვა ადგილას.

სკრიპტი აწარმოებს სამ არტეფაქტს `--output-dir`-ში (ნაგულისხმევია მიმდინარე დირექტორიაში),
პლუს არჩევითი მეოთხე, როდესაც კოდირება ჩართულია:

1. `<slug>.manifest.json` — ხაზის მანიფესტი, რომელიც შეიცავს ვალიდატორის კვორუმს, დაცულ სახელთა სივრცეებს და
   არჩევითი გაშვების დროის განახლების Hook მეტამონაცემები.
2. `<slug>.catalog.toml` — TOML ფრაგმენტი `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`,
   და ნებისმიერი მოთხოვნილი მარშრუტის წესები. დარწმუნდით, რომ `fault_tolerance` დაყენებულია მონაცემთა სივრცის ზომაზე
   ზოლის სარელეო კომიტეტი (`3f+1`).
3. `<slug>.summary.json` — აუდიტის რეზიუმე, რომელიც აღწერს გეომეტრიას (სლაგი, სეგმენტები, მეტამონაცემები) პლუს
   საჭირო გაშვების ნაბიჯები და ზუსტი `cargo xtask space-directory encode` ბრძანება (ქვემოთ
   `space_directory_encode.command`). მიამაგრეთ ეს JSON ჩასვლის ბილეთს მტკიცებულებისთვის.
4. `<slug>.manifest.to` — გამოიყოფა `--encode-space-directory` დაყენებისას; მზადაა Torii-ისთვის
   `iroha app space-directory manifest publish` ნაკადი.

გამოიყენეთ `--dry-run` JSON/სნიპეტების გადახედვისთვის ფაილების ჩაწერის გარეშე და `--force` გადასაწერად
არსებული არტეფაქტები.

## 3. გამოიყენეთ ცვლილებები

1. დააკოპირეთ manifest JSON კონფიგურირებულ `nexus.registry.manifest_directory`-ში (და ქეშში
   დირექტორია, თუ რეესტრი ასახავს დისტანციურ პაკეტებს). ფაილის ჩაბარება, თუ მანიფესტები არის ვერსიაში
   თქვენი კონფიგურაციის რეპო.
2. დაამატეთ კატალოგის ფრაგმენტი `config/config.toml`-ს (ან შესაბამის `config.d/*.toml`-ს). უზრუნველყოს
   `nexus.lane_count` არის მინიმუმ `lane_id + 1` და განაახლეთ ნებისმიერი `nexus.routing_policy.rules`, რომელიც
   უნდა მიუთითებდეს ახალ ზოლზე.
3. დაშიფვრეთ (თუ გამოტოვეთ `--encode-space-directory`) და გამოაქვეყნეთ მანიფესტი Space Directory-ში
   შეჯამებაში აღბეჭდილი ბრძანების გამოყენებით (`space_directory_encode.command`). ეს აწარმოებს
   `.manifest.to` ტვირთამწეობა Torii მოელის და აღრიცხავს მტკიცებულებებს აუდიტორებისთვის; გაგზავნა მეშვეობით
   `iroha app space-directory manifest publish`.
4. გაუშვით `irohad --sora --config path/to/config.toml --trace-config` და დაარქივეთ კვალის გამომავალი
   გაშვების ბილეთი. ეს ადასტურებს, რომ ახალი გეომეტრია ემთხვევა გენერირებულ შლაგ/კურას სეგმენტებს.
5. მანიფესტის/კატალოგის ცვლილებების განლაგების შემდეგ გადატვირთეთ ზოლზე მინიჭებული ვალიდატორები. შეინახეთ
   შემაჯამებელი JSON ბილეთში მომავალი აუდიტისთვის.

## 4. შექმენით რეესტრის სადისტრიბუციო პაკეტი

შეფუთეთ გენერირებული მანიფესტი და გადაფარვა, რათა ოპერატორებმა შეძლონ ზოლის მართვის მონაცემების გარეშე გავრცელება
კონფიგურაციის რედაქტირება ყველა ჰოსტზე. Bundler-ის დამხმარე ასლები მანიფესტებს კანონიკურ განლაგებაში,
აწარმოებს არასავალდებულო მართვის კატალოგის გადაფარვას `nexus.registry.cache_directory`-ისთვის და შეუძლია გამოუშვას
tarball ოფლაინ გადარიცხვებისთვის:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

შედეგები:

1. `manifests/<slug>.manifest.json` — დააკოპირეთ ისინი კონფიგურირებულში
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — გადადით `nexus.registry.cache_directory`-ში. ყოველი `--module`
   ჩანაწერი ხდება ჩამრთველი მოდულის განმარტება, რომელიც საშუალებას აძლევს მართვის მოდულის გაცვლას (NX-2) მიერ
   ქეშის გადაფარვის განახლება `config.toml` რედაქტირების ნაცვლად.
3. `summary.json` — მოიცავს ჰეშებს, გადაფარვის მეტამონაცემებს და ოპერატორის ინსტრუქციებს.
4. სურვილისამებრ `registry_bundle.tar.*` — მზად არის SCP, S3 ან არტეფაქტის ტრეკერებისთვის.

დაასინქრონეთ მთელი დირექტორია (ან არქივი) თითოეულ ვალიდატორთან, ამოიღეთ საჰაერო უფსკრული ჰოსტებზე და დააკოპირეთ
მანიფესტები + ქეში გადაფარავს მათ რეესტრის ბილიკებს Torii-ის გადატვირთვამდე.

## 5. ვალიდატორის კვამლის ტესტები

Torii გადატვირთვის შემდეგ, გაუშვით ახალი კვამლის დამხმარე, რათა გადაამოწმოთ ზოლის ანგარიშები `manifest_ready=true`,
მეტრიკა ასახავს ზოლის მოსალოდნელ რაოდენობას და დალუქული ლიანდაგი ნათელია. ბილიკები, რომლებიც საჭიროებენ მანიფესტებს
უნდა გამოაშკარავდეს არა ცარიელი `manifest_path`; დამხმარე ახლა მაშინვე მარცხდება, როცა გზა აკლია
NX-7 განლაგების ყოველი ჩანაწერი მოიცავს ხელმოწერილ მანიფესტ მტკიცებულებებს:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

დაამატეთ `--insecure` თვითმოწერილი გარემოს ტესტირებისას. სკრიპტი გამოდის ნულის გარეშე, თუ ზოლი არის
აკლია, დალუქულია ან მეტრიკა/ტელემეტრია გადახრის მოსალოდნელ მნიშვნელობებს. გამოიყენეთ
`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` და
`--max-headroom-events` სახელურები თითო ზოლის ბლოკის სიმაღლის/ფინალურობის/ჩამორჩენილი/თავის ოთახის ტელემეტრიის შესანარჩუნებლად
თქვენს საოპერაციო კონვერტებში და დააკავშირეთ ისინი `--max-slot-p95` / `--max-slot-p99`
(პლუს `--min-slot-samples`) NX‑18 სლოტის ხანგრძლივობის სამიზნეების განსახორციელებლად დამხმარედან გაუსვლელად.

საჰაერო უფსკრული ვალიდაციისთვის (ან CI) შეგიძლიათ გაიმეოროთ გადაღებული Torii პასუხი პირდაპირ ეთერში გასვლის ნაცვლად
საბოლოო წერტილი:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

ჩაწერილი მოწყობილობები `fixtures/nexus/lanes/`-ის ქვეშ ასახავს ბუტსტრაპის მიერ წარმოებულ არტეფაქტებს
დამხმარე, ასე რომ ახალი მანიფესტების გაფორმება შესაძლებელია შეკვეთილი სკრიპტის გარეშე. CI ახორციელებს იმავე ნაკადს მეშვეობით
`ci/check_nexus_lane_smoke.sh` და `ci/check_nexus_lane_registry_bundle.sh`
(ასევე: `make check-nexus-lanes`) დაამტკიცოს, რომ NX-7 კვამლის დამხმარე შეესაბამება გამოქვეყნებულს
დატვირთვის ფორმატი და იმის უზრუნველსაყოფად, რომ პაკეტების დაიჯესტები/გადაფარვა კვლავწარმოებადია.

როდესაც ზოლს სახელი გადაერქვა, გადაიღეთ ტელემეტრიული მოვლენები `nexus.lane.topology` (მაგალითად,
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) და მიაწოდეთ ისინი უკან
კვამლის დამხმარე. `--telemetry-file/--from-telemetry` დროშა იღებს ახალი ხაზით გამოყოფილ ჟურნალს და
`--require-alias-migration old:new` ამტკიცებს, რომ `alias_migrated` მოვლენამ ჩაწერა სახელის შეცვლა:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` მოწყობილობა აერთიანებს კანონიკურ გადარქმევის ნიმუშს, რათა CI-მ შეძლოს გადამოწმება
ტელემეტრიის ანალიზის გზა ცოცხალ კვანძთან დაკავშირების გარეშე.

## ვალიდატორის დატვირთვის ტესტები (NX-7 მტკიცებულება)

საგზაო რუკა **NX-7** მოითხოვს ყოველ ახალ ზოლს, რათა გადაიტანოს რეპროდუცირებადი ვალიდატორის დატვირთვა. გამოყენება
`scripts/nexus_lane_load_test.py` კვამლის ჩეკების, სლოტის ხანგრძლივობის კარიბჭის და სლოტის შეკვრის შესაკერად
გამოიხატება ერთ არტეფაქტურ კომპლექტში, რომლის გამეორებაც მმართველობას შეუძლია:

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

დამხმარე ახორციელებს იგივე DA კვორუმს, ორაკულს, ანგარიშსწორების ბუფერს, TEU-ს და სლოტის ხანგრძლივობის კარიბჭეს.
კვამლის დამხმარის მიერ და წერს `smoke.log`, `slot_summary.json`, სლოტის პაკეტის მანიფესტს და
`load_test_manifest.json` არჩეულ `--out-dir`-ში, ასე რომ დატვირთვის გაშვებები შეიძლება პირდაპირ დაერთოს
ბილეთების გაშვება შეკვეთილი სკრიპტის გარეშე.

## 6. ტელემეტრია და მმართველობის შემდგომი მონიტორინგი

- განაახლეთ ზოლის დაფები (`dashboards/grafana/nexus_lanes.json` და შესაბამისი გადაფარვები)
  ახალი ზოლის ID და მეტამონაცემები. გენერირებული მეტამონაცემების გასაღებები (`contact`, `channel`, `runbook` და ა.შ.) ქმნის
  მარტივია ეტიკეტების წინასწარ შევსება.
- Wire PagerDuty/Alertmanager წესები ახალი ზოლისთვის დაშვების ჩართვამდე. `summary.json`
  შემდეგი ნაბიჯების მასივი ასახავს საკონტროლო სიას [Nexus ოპერაციებში] (./nexus-operations).
- დაარეგისტრირეთ manifest-ის ნაკრები Space Directory-ში, როგორც კი ვალიდატორის ნაკრები გამოვა. გამოიყენეთ იგივე
  მანიფესტი JSON, გენერირებული დამხმარის მიერ, ხელმოწერილი მმართველობის სახელმძღვანელოს მიხედვით.
- მიჰყევით [Sora Nexus ოპერატორის ჩართვა](./nexus-operator-onboarding) კვამლის ტესტებისთვის (FindNetworkStatus, Torii
  ხელმისაწვდომობა) და აღბეჭდეთ მტკიცებულება ზემოთ მოყვანილი არტეფაქტის ნაკრებით.

## 7. მშრალი გაშვების მაგალითი

არტეფაქტების წინასწარ გადახედვა ფაილების დაწერის გარეშე:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --dry-run
```

ბრძანება ბეჭდავს JSON შეჯამებას და TOML ფრაგმენტს stdout-ში, რაც საშუალებას აძლევს სწრაფ გამეორებას
დაგეგმვა.

---

დამატებითი კონტექსტისთვის იხილეთ:- [Nexus ოპერაციები](./nexus-operations) — ოპერატიული საკონტროლო სია და ტელემეტრიის მოთხოვნები.
- [Sora Nexus ოპერატორის ჩართვა](./nexus-operator-onboarding) — დეტალური ჩართვის ნაკადი, რომელიც მიუთითებს
  ახალი დამხმარე.
- [Nexus ზოლის მოდელი](./nexus-lane-model) — ზოლის გეომეტრია, შლაკები და შენახვის განლაგება, რომელსაც იყენებს ხელსაწყო.