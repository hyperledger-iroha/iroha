---
lang: ka
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## ანგარიშის მისამართის შესაბამისობის სტატუსი (ADDR-2)

სტატუსი: მიღებულია 2026-03-30  
მფლობელები: მონაცემთა მოდელის გუნდი / QA გილდია  
საგზაო რუქის მითითება: ADDR-2 — ორმაგი ფორმატის შესაბამისობის კომპლექტი

### 1. მიმოხილვა

- მოწყობილობა: `fixtures/account/address_vectors.json` (I105 (სასურველია) + შეკუმშული (`sora`, მეორე საუკეთესო) + მრავალმხრივი დადებითი/უარყოფითი შემთხვევები).
- ფარგლები: განმსაზღვრელი V1 დატვირთვა, რომელიც მოიცავს ნაგულისხმევს, ლოკალურ-12, გლობალურ რეესტრს და მრავალსიგლიან კონტროლერებს სრული შეცდომების ტაქსონომიით.
- განაწილება: გაზიარებული Rust მონაცემთა მოდელის, Torii, JS/TS, Swift და Android SDK-ებში; CI ვერ ხერხდება, თუ რომელიმე მომხმარებელი გადახრის.
- სიმართლის წყარო: გენერატორი ცხოვრობს `crates/iroha_data_model/src/account/address/compliance_vectors.rs`-ში და ვლინდება `cargo xtask address-vectors`-ის საშუალებით.
### 2. რეგენერაცია და შემოწმება

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

დროშები:

- `--out <path>` — სურვილისამებრ უგულებელყოფა ad-hoc პაკეტების წარმოებისას (ნაგულისხმევი `fixtures/account/address_vectors.json`).
- `--stdout` — გამოუშვით JSON stdout-ზე დისკზე ჩაწერის ნაცვლად.
- `--verify` — შეადარეთ მიმდინარე ფაილი ახლად გენერირებულ კონტენტთან (სწრაფად ვერ ხერხდება დრიფტის დროს; არ შეიძლება გამოყენებულ იქნას `--stdout`-თან).

### 3. არტეფაქტის მატრიცა

| ზედაპირი | აღსრულება | შენიშვნები |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | აანალიზებს JSON-ს, აღადგენს კანონიკურ დატვირთვას და ამოწმებს I105 (სასურველია)/შეკუმშული (`sora`, მეორე საუკეთესო)/კანონიკურ კონვერტაციებს + სტრუქტურირებულ შეცდომებს. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | ამოწმებს სერვერის კოდეკებს, ასე რომ Torii უარს ამბობს არასწორადფორმირებულ I105 (სასურველია)/შეკუმშული (`sora`, მეორე საუკეთესო) იტვირთებაზე. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | სარკეები V1 მოწყობილობები (I105 სასურველია/შეკუმშული (`sora`) მეორე საუკეთესო/სრული სიგანე) და ამტკიცებს Norito სტილის შეცდომის კოდებს ყველა უარყოფითი შემთხვევისთვის. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | სავარჯიშოები I105 (სასურველია)/შეკუმშული (`sora`, მეორე საუკეთესო) გაშიფვრა, მრავალსაფეხურიანი დატვირთვა და შეცდომა Apple-ის პლატფორმებზე. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | უზრუნველყოფს Kotlin/Java საკინძები დარჩება კანონიკურ ფიქსაციასთან შესაბამისობაში. |

### 4. მონიტორინგი და გამორჩეული სამუშაო- სტატუსის მოხსენება: ეს დოკუმენტი დაკავშირებულია `status.md`-დან და საგზაო რუქიდან, ასე რომ, ყოველკვირეულ მიმოხილვებს შეუძლია დაადასტუროს მოწყობილობების ჯანმრთელობა.
- დეველოპერის პორტალის შეჯამება: იხილეთ **მინიშნება → ანგარიშის მისამართების შესაბამისობა** დოკუმენტების პორტალში (`docs/portal/docs/reference/account-address-status.md`) გარედან მოყვანილი სინოპსისისთვის.
- Prometheus და დაფები: ყოველთვის, როცა SDK-ის ასლს ადასტურებთ, გაუშვით დამხმარე `--metrics-out` (და სურვილისამებრ `--metrics-label`), რათა Prometheus ტექსტის ფაილების კოლექციონერმა შეძლოს I1003NI00X-ის გადაყლაპვა. Grafana საინფორმაციო დაფა **ანგარიშის მისამართის დამაგრების სტატუსი** (`dashboards/grafana/account_address_fixture_status.json`) ასახავს უღელტეხილის/ავარიების რაოდენობას თითო ზედაპირზე და გამოაქვს კანონიკური SHA-256 დაიჯესტი აუდიტორული მტკიცებულებისთვის. გაფრთხილება, როდესაც რომელიმე სამიზნე იტყობინება `0`.
- Torii მეტრიკა: `torii_address_domain_total{endpoint,domain_kind}` ახლა ასხივებს ყოველი წარმატებით გაანალიზებული ანგარიშის სიტყვასიტყვით, `torii_address_invalid_total`/`torii_address_local8_total`-ის ასახვით. გაფრთხილება წარმოების ნებისმიერ `domain_kind="local12"` ტრაფიკზე და ასახეთ მრიცხველები SRE `address_ingest` დაფაზე, რათა Local-12 საპენსიო კარიბჭეს ჰქონდეს აუდიტორული მტკიცებულება.
- მოწყობილობების დამხმარე: `scripts/account_fixture_helper.py` ჩამოტვირთავს ან ამოწმებს კანონიკურ JSON-ს, რათა SDK-ის გამოშვების ავტომატიზაციამ შეძლოს პაკეტის მიღება/შემოწმება ხელით კოპირების/ჩასმის გარეშე, ხოლო სურვილისამებრ დაწერს Prometheus მეტრიკას. მაგალითი:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  დამხმარე წერს `account_address_fixture_check_status{target="android"} 1`-ს, როდესაც სამიზნე ემთხვევა, პლუს `account_address_fixture_remote_info` / `account_address_fixture_local_info` ლიანდაგები, რომლებიც ავლენს SHA-256-ის მონელებას. დაკარგული ფაილების ანგარიში `account_address_fixture_local_missing`.
  ავტომატიზაციის შეფუთვა: დარეკეთ `ci/account_fixture_metrics.sh`-დან cron/CI-დან კონსოლიდირებული ტექსტური ფაილის გამოსაცემად (ნაგულისხმევი `artifacts/account_fixture/address_fixture.prom`). გაიარეთ განმეორებითი `--target label=path` ჩანაწერები (სურვილისამებრ დაუმატეთ `::https://mirror/...` თითო სამიზნე წყაროს გადასალახად), ასე რომ, Prometheus აჭრის ერთ ფაილს, რომელიც მოიცავს ყოველ SDK/CLI ასლს. GitHub სამუშაო ნაკადი `address-vectors-verify.yml` უკვე ამუშავებს ამ დამხმარეს კანონიკურ მოწყობილობას და ატვირთავს `account-address-fixture-metrics` არტეფაქტს SRE-ის შესანახად.