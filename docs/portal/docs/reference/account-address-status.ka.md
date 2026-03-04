---
lang: ka
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b92bdfc323a4bc031ca7f2237f238d5d515f7238791a6ec9c50b55e361c85560
source_last_modified: "2026-01-28T17:11:30.639071+00:00"
translation_last_reviewed: 2026-02-07
id: account-address-status
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
---

კანონიკური ADDR-2 პაკეტი (`fixtures/account/address_vectors.json`) იჭერს
IH58 (სასურველია), შეკუმშული (`sora`, მეორე საუკეთესო; ნახევარი/სრული სიგანე), მრავალხელმოწერის და ნეგატიური მოწყობილობები.
ყოველი SDK + Torii ზედაპირი ეყრდნობა ერთსა და იმავე JSON-ს, ასე რომ ჩვენ შეგვიძლია აღმოვაჩინოთ ნებისმიერი კოდეკი
დრიფტი, სანამ ის წარმოებას მიაღწევს. ეს გვერდი ასახავს შიდა სტატუსის მოკლე ინფორმაციას
(`docs/source/account_address_status.md` root საცავში) ისე პორტალი
მკითხველს შეუძლია მიმართოს სამუშაო პროცესს მონო-რეპოს გათხრების გარეშე.

## შექმენით ან გადაამოწმეთ პაკეტი

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

დროშები:

- `--stdout` — გამოუშვით JSON stdout ad-hoc შემოწმებისთვის.
- `--out <path>` — ჩაწერეთ სხვა გზაზე (მაგ. ლოკალური ცვლილებების განსხვავებისას).
- `--verify` — სამუშაო ასლის შედარება ახლად გენერირებულ შინაარსთან (არ შეიძლება
  კომბინირებული იყოს `--stdout`-თან).

CI სამუშაო პროცესი **Address Vector Drift** მუშაობს `cargo xtask address-vectors --verify`
ნებისმიერ დროს, როდესაც მოწყობილობა, გენერატორი ან დოკუმენტები იცვლება, რათა დაუყოვნებლივ გააფრთხილონ მიმომხილველები.

## ვინ მოიხმარს მოწყობილობას?

| ზედაპირი | ვალიდაცია |
|---------|-----------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (სერვერი) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ყოველი აღკაზმულობა ორმხრივი მოგზაურობის კანონიკური ბაიტი + IH58 + შეკუმშული (`sora`, მეორე საუკეთესო) კოდირებები და
ამოწმებს, რომ Norito სტილის შეცდომის კოდები შეესაბამება ინსტრუმენტს უარყოფითი შემთხვევებისთვის.

## გჭირდებათ ავტომატიზაცია?

გამოშვების ინსტრუმენტს შეუძლია დამხმარეთან ერთად განაახლოს სკრიპტის მოწყობილობები
`scripts/account_fixture_helper.py`, რომელიც იღებს ან ამოწმებს კანონიკურს
ნაკრები კოპირების/ჩასმის ნაბიჯების გარეშე:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

დამხმარე იღებს `--source` გადაფარვას ან `IROHA_ACCOUNT_FIXTURE_URL`-ს
გარემოს ცვლადი, ასე რომ SDK CI სამუშაოებმა შეიძლება მიუთითონ სასურველი სარკეზე.
როდესაც `--metrics-out` მიეწოდება დამხმარე წერს
`account_address_fixture_check_status{target=\"…\"}` კანონიკურთან ერთად
SHA-256 დაიჯესტი (`account_address_fixture_remote_info`) ასე რომ, Prometheus ტექსტური ფაილი
კოლექციონერები და Grafana დაფა `account_address_fixture_status` შეუძლიათ დაამტკიცონ
ყველა ზედაპირი რჩება სინქრონიზებული. გაფრთხილება, როდესაც სამიზნე იტყობინება `0`. ამისთვის
მრავალზედაპირიანი ავტომატიზაცია გამოიყენეთ შეფუთვა `ci/account_fixture_metrics.sh`
(მიიღებს განმეორებით `--target label=path[::source]`) ასე რომ მოწვეულ გუნდებს შეუძლიათ გამოაქვეყნონ
ერთი კონსოლიდირებული `.prom` ფაილი კვანძის ექსპორტიორის ტექსტური ფაილის კოლექციონერისთვის.

## გჭირდებათ სრული მოკლე ინფორმაცია?

ADDR-2 შესაბამისობის სრული სტატუსი (მფლობელები, მონიტორინგის გეგმა, ღია სამოქმედო ელემენტები)
ცხოვრობს `docs/source/account_address_status.md`-ში საცავში ერთად
მისამართის სტრუქტურით RFC (`docs/account_structure.md`). გამოიყენეთ ეს გვერდი, როგორც a
სწრაფი ოპერატიული შეხსენება; გადადეთ რეპო დოკუმენტებს სიღრმისეული ხელმძღვანელობისთვის.