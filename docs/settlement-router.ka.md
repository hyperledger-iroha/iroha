---
lang: ka
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-12-29T18:16:35.914434+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# განმსაზღვრელი დასახლების როუტერი (NX-3)

** სტატუსი: ** დასრულებული (NX-3)  
**მფლობელები:** Economics WG / Core Ledger WG / Treasury / SRE  
**ფარგლები:** კანონიკური XOR დასახლების გზა გამოიყენება ყველა ზოლში/მონაცემთა სივრცეში. გაგზავნილი როუტერის ყუთი, ზოლის დონის ქვითრები, ბუფერული დამცავი რელსები, ტელემეტრია და ოპერატორის მტკიცებულების ზედაპირები.

## გოლები
- გააერთიანეთ XOR-ის კონვერტაცია და მიღებების გენერაცია ერთი ხაზისა და Nexus კონსტრუქციებში.
- გამოიყენეთ დეტერმინისტული თმის შეჭრა + ცვალებადობის მინდვრები დამცავი მოაჯირებით ბუფერებით, რათა ოპერატორებმა შეძლონ უსაფრთხოდ გადაადგილება.
- გამოავლინეთ ქვითრები, ტელემეტრია და დაფები, რომლებზეც აუდიტორებს შეუძლიათ ხელახლა დაკვრა შეკვეთილი ინსტრუმენტების გარეშე.

## არქიტექტურა
| კომპონენტი | მდებარეობა | პასუხისმგებლობა |
|-----------|----------|---------------|
| როუტერის პრიმიტივები | `crates/settlement_router/` | ჩრდილის ფასის კალკულატორი, თმის შეჭრის ფენები, ბუფერული პოლიტიკის დამხმარეები, ანგარიშსწორების ქვითრის ტიპი.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/rsrc.
| გაშვების ფასადი | `crates/iroha_core/src/settlement/mod.rs:1` | ახვევს როუტერის კონფიგურაციას `SettlementEngine`-ში, აჩვენებს `quote` + აკუმულატორს, რომელიც გამოიყენება ბლოკის შესრულების დროს. |
| ბლოკის ინტეგრაცია | `crates/iroha_core/src/block.rs:120` | ასუფთავებს `PendingSettlement` ჩანაწერებს, აგროვებს `LaneSettlementCommitment` ხაზს/მონაცემთა სივრცეს, აანალიზებს ზოლის ბუფერულ მეტამონაცემებს და ასხივებს ტელემეტრიას. |
| ტელემეტრია და დაფები | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | Prometheus/OTLP მეტრიკა ბუფერებისთვის, ვარიაციები, თმის შეჭრა, კონვერტაციის რაოდენობა; Grafana დაფა SRE-სთვის. |
| მითითების სქემა | `docs/source/nexus_fee_model.md:1` | დოკუმენტების ანგარიშსწორების ქვითრის ველები შენარჩუნდა `LaneBlockCommitment`-ში. |

## კონფიგურაცია
როუტერის სახელურები მუშაობს `[settlement.router]` ქვეშ (დამოწმებულია `iroha_config`-ით):

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

ზოლის მეტამონაცემების სადენები თითო მონაცემთა სივრცის ბუფერულ ანგარიშში:
- `settlement.buffer_account` — ანგარიში, რომელიც ინახავს რეზერვს (მაგ., `buffer::cbdc_treasury`).
- `settlement.buffer_asset` — აქტივის განმარტება დებეტირდება სათავე ოთახისთვის (ჩვეულებრივ `xor#sora`).
- `settlement.buffer_capacity_micro` — კონფიგურირებული სიმძლავრე მიკრო-XOR-ში (ათწილადი სტრიქონი).

მეტამონაცემების არარსებობა თიშავს ამ ზოლის ბუფერულ სნეპშოტს (ტელემეტრია ბრუნდება ნულოვანი სიმძლავრის/სტატუსზე).## კონვერტაციის მილსადენი
1. **ციტატა:** `SettlementEngine::quote` იყენებს კონფიგურირებულ epsilon + არასტაბილურობის ზღვარს და თმის შეჭრის დონეს TWAP ციტატებზე, აბრუნებს `SettlementReceipt`-ს `xor_due`-ით და `xor_due`-ით და Prometheus-ით და Prometheus. `source_id`.【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **აკუმულირება:** ბლოკის შესრულებისას შემსრულებელი აღრიცხავს `PendingSettlement` ჩანაწერებს (ადგილობრივი თანხა, TWAP, epsilon, არასტაბილურობის თაიგული, ლიკვიდობის პროფილი, ორაკულის დროის ანაბეჭდი). `LaneSettlementBuilder` აგროვებს ჯამებს და ცვლის მეტამონაცემებს `(lane, dataspace)`-ზე ბლოკის დალუქვამდე.【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:
3. **ბუფერის სნეპშოტი:** თუ ზოლის მეტამონაცემები გამოაცხადებს ბუფერს, შემქმნელი აფიქსირებს `SettlementBufferSnapshot`-ს (დარჩენილ სათავეს, ტევადობას, სტატუსს) `BufferPolicy` ზღვრების გამოყენებით კონფიგურაციიდან.
4. **მიმართვა + ტელემეტრია:** ქვითრები და მტკიცებულებების გაცვლა ხდება `LaneBlockCommitment`-ში და აისახება სტატუსის სურათებში. ტელემეტრია აღრიცხავს ბუფერულ ლიანდაგებს, დისპერსიას (`iroha_settlement_pnl_xor`), გამოყენებულ ზღვარს (`iroha_settlement_haircut_bp`), არჩევით სვოპლაინის გამოყენებას და თითო აქტივის კონვერტაციის/თმის შეჭრის მრიცხველებს, რათა დაფები და გაფრთხილებები დარჩეს ბლოკთან სინქრონული. შინაარსი.【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **მტკიცებულებების ზედაპირები:** `status::set_lane_settlement_commitments` აქვეყნებს ვალდებულებებს რელეების/DA მომხმარებლებისთვის, Grafana დაფები კითხულობენ Prometheus მეტრიკას, ხოლო ოპერატორები იყენებენ `ops/runbooks/settlement-buffers.md` `ops/runbooks/settlement-buffers.md`0303 რელეს/და თრეკეტის მოვლენებთან ერთად I10NI.

## ტელემეტრია და მტკიცებულება
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — ბუფერული სურათი თითო ზოლზე/მონაცემთა სივრცეში (მიკრო-XOR + კოდირებული მდგომარეობა).【crates/iroha_telemetry/src/metrics.rs:2
- `iroha_settlement_pnl_xor` — რეალიზებული განსხვავება ბლოკის პარტიისთვის თმის შეჭრის XOR-ს შორის.【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — ეფექტური epsilon/თმის შეჭრის საბაზისო ქულები გამოიყენება პარტიაზე.【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — არასავალდებულო გამოყენება ლიკვიდურობის პროფილით, როდესაც არსებობს გაცვლის მტკიცებულება.【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — ზოლის/მონაცემთა სივრცის მრიცხველები ანგარიშსწორების კონვერტაციისთვის და კუმულაციური თმის შეჭრისთვის (XOR ერთეული).
- Grafana დაფა: `dashboards/grafana/settlement_router_overview.json` (ბუფერული ადგილი, ვარიაცია, თმის შეჭრა) პლუს Alertmanager წესები ჩაშენებული Nexus ზოლის გაფრთხილების პაკეტში.
- ოპერატორის გაშვების წიგნი: `ops/runbooks/settlement-buffers.md` (შევსება/გაფრთხილების სამუშაო პროცესი) და ხშირად დასმული კითხვები `docs/source/nexus_settlement_faq.md`-ში.## დეველოპერი და SRE საკონტროლო სია
- დააყენეთ `[settlement.router]` მნიშვნელობები `config/config.json5`-ში (ან TOML) და დაადასტურეთ `irohad --version` ჟურნალების მეშვეობით; უზრუნველყოს, რომ ზღურბლები აკმაყოფილებს `alert > throttle > xor_only > halt`.
- შეავსეთ ზოლის მეტამონაცემები ბუფერული ანგარიშით/აქტივი/ტევადობა, რათა ბუფერული ლიანდაგები ასახავდეს ცოცხალ რეზერვებს; გამოტოვეთ ველები ზოლებისთვის, რომლებიც არ უნდა აკონტროლონ ბუფერები.
- `settlement_router_*` და `iroha_settlement_*` მეტრიკის მონიტორინგი `dashboards/grafana/settlement_router_overview.json`-ის საშუალებით; გაფრთხილება დროსელის/მხოლოდ XOR/შეჩერების მდგომარეობებზე.
- გაუშვით `cargo test -p settlement_router` ფასების/პოლიტიკის დაფარვისთვის და არსებული ბლოკის დონის აგრეგაციის ტესტებისთვის `crates/iroha_core/src/block.rs`-ში.
- ჩაიწერეთ მართვის დადასტურებები კონფიგურაციის ცვლილებებისთვის `docs/source/nexus_fee_model.md`-ში და განაახლეთ `status.md`, როდესაც იცვლება ზღურბლები ან ტელემეტრიული ზედაპირები.

## გაშვების გეგმის Snapshot
- როუტერი + ტელემეტრიული ხომალდი ყველა მშენებლობაში; არ არის ფუნქციური კარიბჭე. ზოლის მეტამონაცემები აკონტროლებს, გამოქვეყნდება თუ არა ბუფერული სნეპშოტები.
- ნაგულისხმევი კონფიგურაცია ემთხვევა საგზაო რუქის მნიშვნელობებს (60s TWAP, 25bp საბაზისო epsilon, 72h ბუფერული ჰორიზონტი); დააინსტალირეთ კონფიგურაციის საშუალებით და გადატვირთეთ `irohad` გამოსაყენებლად.
- მტკიცებულებათა ნაკრები = ზოლის გადახდის ვალდებულებები + Prometheus ნაკაწრი `settlement_router_*`/`iroha_settlement_*` სერიისთვის + Grafana ეკრანის ანაბეჭდი/JSON ექსპორტი დაზარალებული ფანჯრისთვის.

## მტკიცებულებები და ცნობები
- NX-3 ანგარიშსწორების როუტერის მიღების შენიშვნები: `status.md` (NX-3 განყოფილება).
- ოპერატორის ზედაპირები: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- ქვითრების სქემა და API ზედაპირები: `docs/source/nexus_fee_model.md`, `/v1/sumeragi/status` -> `lane_settlement_commitments`.