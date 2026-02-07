---
id: norito-rpc-adoption
lang: ka
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> კანონიკური დაგეგმვის ნოტები ცხოვრობს `docs/source/torii/norito_rpc_adoption_schedule.md`-ში.  
> პორტალის ეს ასლი ასახავს SDK-ს ავტორებს, ოპერატორებს და მიმომხილველებს გავრცელების მოლოდინებს.

## მიზნები

- გაასწორეთ ყველა SDK (Rust CLI, Python, JavaScript, Swift, Android) ბინარულ Norito-RPC ტრანსპორტზე AND4 წარმოების გადართვის წინ.
- შეინახეთ ფაზის კარიბჭეები, მტკიცებულებათა პაკეტები და ტელემეტრიული კაკვები განმსაზღვრელი, რათა მმართველობამ შეძლოს გაშვების აუდიტი.
- ტრივიალური გახადეთ მოწყობილობებისა და კანარის მტკიცებულებების აღბეჭდვა საერთო დამხმარეებთან ერთად, რომლებსაც საგზაო რუკა NRPC-4 მოუწოდებს.

## ფაზის ვადები

| ფაზა | ფანჯარა | ფარგლები | გასვლის კრიტერიუმები |
|-------|--------|-------|--------------|
| **P0 – ლაბორატორიული პარიტეტი ** | Q22025 | Rust CLI + Python smoke კომპლექტი მუშაობს `/v1/norito-rpc` CI-ში, JS helper გადის ერთეულის ტესტებს, Android-ის იმიტირებული აღკაზმულობა ახორციელებს ორმაგ ტრანსპორტს. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` და `javascript/iroha_js/test/noritoRpcClient.test.js` მწვანე CI-ში; Android-ის აღკაზმულობა ჩართულია `./gradlew test`-ში. |
| **P1 – SDK გადახედვა ** | Q32025 | საზიარო მოწყობილობების ნაკრები შემოწმებულია, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` ჩანაწერების ჟურნალები + JSON `artifacts/norito_rpc/`-ში, სურვილისამებრ Norito სატრანსპორტო დროშები გამოფენილია SDK ნიმუშებში. | მანიფესტზე ხელმოწერილია, README-ის განახლებები აჩვენებს არჩევის გამოყენებას, Swift preview API ხელმისაწვდომია IOS2 დროშის უკან. |
| **P2 – დადგმა / AND4 გადახედვა ** | Q12026 | დადგმის Torii აუზებს ურჩევნიათ Norito, Android AND4 წინასწარი გადახედვის კლიენტები და Swift IOS2 პარიტეტული კომპლექტები ნაგულისხმევად ორობითი ტრანსპორტით, ტელემეტრიული დაფა `dashboards/grafana/torii_norito_rpc_observability.json` დასახლებული. | `docs/source/torii/norito_rpc_stage_reports.md` იჭერს კანარის, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` პასს, Android-ის იმიტირებულ აღკაზმულობის გამეორებას აღბეჭდავს წარმატების/შეცდომის შემთხვევებს. |
| **P3 – წარმოება GA** | Q42026 | Norito ხდება ნაგულისხმევი ტრანსპორტი ყველა SDK-სთვის; JSON რჩება ბრაუნოუტ გამომწვევად. გამოუშვით ვაკანსიების არქივის პარიტეტის არტეფაქტები ყოველი ტეგით. | გამოუშვით საკონტროლო სიის პაკეტები Norito კვამლის გამომავალი Rust/JS/Python/Swift/Android-ისთვის; გაფრთხილების ზღურბლები Norito vs JSON შეცდომის სიხშირის SLO-ებისთვის დაწესებულია; `status.md` და გამოშვების შენიშვნები მოჰყავს GA მტკიცებულებებს. |

## SDK მიწოდება და CI კაკვები

- **Rust CLI და ინტეგრაციის აღკაზმულობა** - გააგრძელეთ `iroha_cli pipeline` კვამლის ტესტები, რათა აიძულოთ Norito ტრანსპორტი, როგორც კი `cargo xtask norito-rpc-verify` დაეშვება. მცველი `cargo test -p integration_tests -- norito_streaming` (ლაბორატორია) და `cargo xtask norito-rpc-verify` (დადგმა/GA), ინახავს არტეფაქტებს `artifacts/norito_rpc/` ქვეშ.
- **Python SDK** - ნაგულისხმევი გამოშვების კვამლი (`python/iroha_python/scripts/release_smoke.sh`) Norito RPC, შეინახეთ `run_norito_rpc_smoke.sh` როგორც CI შესასვლელი წერტილი და დოკუმენტის პარიტეტის დამუშავება `python/iroha_python/README.md`-ში. CI სამიზნე: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** – დაასტაბილურეთ `NoritoRpcClient`, ნება მიეცით მართვის/შეკითხვის დამხმარეებს ნაგულისხმევად Norito, როდესაც `toriiClientConfig.transport.preferred === "norito_rpc"`, და აღბეჭდეთ ნიმუშები ბოლოდან ბოლომდე `javascript/iroha_js/recipes/`-ში. გამოქვეყნებამდე CI-მ უნდა გაუშვას `npm test` პლუს dockerized `npm run test:norito-rpc` სამუშაო; წარმოშობის ატვირთვები Norito კვამლის ჟურნალი `javascript/iroha_js/artifacts/` ქვეშ.
- **Swift SDK** – გაატარეთ Norito ხიდის ტრანსპორტი IOS2 დროშის უკან, ასახეთ მოწყობილობების კადენცია და დარწმუნდით, რომ Connect/Norito პარიტეტული კომპლექტი გადის Buildkite ზოლში, რომელიც მითითებულია `docs/source/sdk/swift/index.md`-ში.
- **Android SDK** – AND4 წინასწარი გადახედვის კლიენტები და იმიტირებული Torii აღკაზმულობა იღებენ Norito, ხელახალი ცდის/დაბრუნების ტელემეტრიით დოკუმენტირებული `docs/source/sdk/android/networking.md`-ში. აღკაზმულობა აზიარებს მოწყობილობებს სხვა SDK-ებთან `scripts/run_norito_rpc_fixtures.sh --sdk android`-ის საშუალებით.

## მტკიცებულება და ავტომატიზაცია

- `scripts/run_norito_rpc_fixtures.sh` ახვევს `cargo xtask norito-rpc-verify`-ს, იჭერს stdout/stderr-ს და ასხივებს `fixtures.<sdk>.summary.json`-ს, ასე რომ SDK-ის მფლობელებს აქვთ დეტერმინისტული არტეფაქტი, რომ მიამაგრონ `status.md`. გამოიყენეთ `--sdk <label>` და `--out artifacts/norito_rpc/<stamp>/` CI პაკეტების მოწესრიგებისთვის.
- `cargo xtask norito-rpc-verify` აიძულებს სქემის ჰეშის პარიტეტს (`fixtures/norito_rpc/schema_hashes.json`) და ვერ ხერხდება, თუ Torii დააბრუნებს `X-Iroha-Error-Code: schema_mismatch`. დააწყვილეთ ყოველი წარუმატებლობა JSON-ის სარეზერვო ჩანაწერთან გამართვისთვის.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` და `dashboards/grafana/torii_norito_rpc_observability.json` განსაზღვრავს გაფრთხილების კონტრაქტებს NRPC-2-ისთვის. გაუშვით სკრიპტი დაფის ყოველი რედაქტირების შემდეგ და შეინახეთ `promtool` გამომავალი კანარის პაკეტში.
- `docs/source/runbooks/torii_norito_rpc_canary.md` აღწერს დადგმისა და წარმოების წვრთნებს; განაახლეთ ის, როდესაც ინსტრუმენტების ჰეშები ან გაფრთხილების კარიბჭე იცვლება.

## მიმომხილველი საკონტროლო სია

NRPC-4 ეტაპს მონიშვნამდე დაადასტურეთ:

1. უახლესი მოწყობილობების ნაკრების ჰეშები ემთხვევა `fixtures/norito_rpc/schema_hashes.json` და შესაბამის CI არტეფაქტს, რომელიც ჩაწერილია `artifacts/norito_rpc/<stamp>/` ქვეშ.
2. SDK README / პორტალის დოკუმენტები აღწერს, თუ როგორ უნდა აიძულოთ JSON-ის დაბრუნება და მიუთითოთ Norito სატრანსპორტო ნაგულისხმევი.
3. ტელემეტრიის დაფები აჩვენებს ორმაგი დასტას შეცდომის სიხშირის პანელებს გაფრთხილების ბმულებით და Alertmanager მშრალი გაშვება (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) მიმაგრებულია ტრეკერზე.
4. მიღების გრაფიკი აქ ემთხვევა ტრეკერის ჩანაწერს (`docs/source/torii/norito_rpc_tracker.md`) და საგზაო რუკა (NRPC-4) მიუთითებს იმავე მტკიცებულებათა პაკეტზე.

განრიგზე დისციპლინირებული დარჩენა ინარჩუნებს SDK-ს ქცევას პროგნოზირებადს და საშუალებას აძლევს მმართველობითი აუდიტის Norito-RPC მიღებას შეკვეთილი მოთხოვნების გარეშე.