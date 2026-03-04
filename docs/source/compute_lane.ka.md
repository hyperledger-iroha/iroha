---
lang: ka
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# გამოთვლითი ხაზი (SSC-1)

გამოთვლითი ხაზი იღებს HTTP სტილის დეტერმინისტულ ზარებს, ასახავს მათ Kotodama-ზე
შესასვლელი პუნქტები და აღრიცხავს აღრიცხვის / ქვითრებს ბილინგისა და მმართველობითი განხილვისთვის.
ეს RFC ყინავს მანიფესტის სქემას, ზარის/მიღების კონვერტებს, ქვიშის ყუთის დამცავ რელსებს,
და კონფიგურაციის ნაგულისხმევი პირველი გამოშვებისთვის.

## მანიფესტი

- სქემა: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` მიმაგრებულია `1`-ზე; მანიფესტები განსხვავებული ვერსიით უარყოფილია
  ვალიდაციის დროს.
- თითოეული მარშრუტი აცხადებს:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama შესვლის წერტილის სახელი)
  - კოდეკის დაშვების სია (`codecs`)
  - TTL/გაზი/მოთხოვნა/პასუხის ქუდები (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - დეტერმინიზმის/შესრულების კლასი (`determinism`, `execution_class`)
  - SoraFS შესვლის/მოდელის აღწერები (`input_limits`, სურვილისამებრ `model`)
  - ფასების ოჯახი (`price_family`) + რესურსის პროფილი (`resource_profile`)
  - ავტორიზაციის პოლიტიკა (`auth`)
- Sandbox დამცავი მოაჯირები ცხოვრობს manifest `sandbox` ბლოკში და გაზიარებულია ყველასთვის
  მარშრუტები (რეჟიმი/შემთხვევითი/შენახვა და არადეტერმინისტული syscall-ის უარყოფა).

მაგალითი: `fixtures/compute/manifest_compute_payments.json`.

## ზარები, მოთხოვნები და ქვითრები

- სქემა: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` in
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` აწარმოებს მოთხოვნის კანონიკურ ჰეშს (სათაურები ინახება
  დეტერმინისტულ `BTreeMap`-ში და ტვირთამწეობა გადატანილია როგორც `payload_hash`).
- `ComputeCall` იჭერს სახელთა სივრცეს/მარშრუტს, კოდეკს, TTL/გაზს/პასუხის თავსახურს,
  რესურსის პროფილი + ფასების ოჯახი, ავტორიზაცია (`Public` ან UAID-შეკრული
  `ComputeAuthn`), დეტერმინიზმი (`Strict` vs `BestEffort`), შესრულების კლასი
  მინიშნებები (CPU/GPU/TEE), დეკლარირებული SoraFS შეყვანის ბაიტი/ნაწილი, სურვილისამებრ სპონსორი
  ბიუჯეტი და კანონიკური მოთხოვნის კონვერტი. მოთხოვნის ჰეში გამოიყენება
  განმეორებითი დაცვა და მარშრუტიზაცია.
- მარშრუტებში შეიძლება ჩაშენდეს არჩევითი SoraFS მოდელის მითითებები და შეყვანის ლიმიტები
  (inline/chunk caps); მანიფესტი sandbox წესების კარიბჭის GPU/TEE მინიშნებები.
- `ComputePriceWeights::charge_units` გარდაქმნის აღრიცხვის მონაცემებს დარიცხულ გამოთვლებად
  ერთეულები ჭერის გაყოფის მეშვეობით ციკლებზე და გასვლის ბაიტებზე.
- `ComputeOutcome` იუწყება `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted`, ან `InternalError` და სურვილისამებრ მოიცავს პასუხების ჰეშებს/
  ზომები/კოდეკი აუდიტისთვის.

მაგალითები:
- ზარი: `fixtures/compute/call_compute_payments.json`
- ქვითარი: `fixtures/compute/receipt_compute_payments.json`

## Sandbox და რესურსების პროფილები- `ComputeSandboxRules` ბლოკავს შესრულების რეჟიმს `IvmOnly`-ზე ნაგულისხმევად,
  ასახავს დეტერმინისტულ შემთხვევითობას მოთხოვნის ჰეშიდან, იძლევა მხოლოდ წაკითხვის საშუალებას SoraFS
  წვდომა და უარყოფს არადეტერმინისტულ syscal-ებს. GPU/TEE მინიშნებები შემოიფარგლება
  `allow_gpu_hints`/`allow_tee_hints` შესრულების განმსაზღვრელი შესანარჩუნებლად.
- `ComputeResourceBudget` აყენებს თითო პროფილის კაპიტებს ციკლებზე, ხაზოვან მეხსიერებაზე, დასტაზე
  ზომა, IO ბიუჯეტი და გამოსვლა, პლუს გადართვები GPU მინიშნებებისთვის და WASI-lite დამხმარეებისთვის.
- ნაგულისხმევი გაგზავნა ორი პროფილის (`cpu-small`, `cpu-balanced`) ქვემოთ
  `defaults::compute::resource_profiles` დეტერმინისტული ჩანაცვლებით.

## ფასები და ბილინგის ერთეული

- ფასების ოჯახები (`ComputePriceWeights`) ასახავს ციკლებს და ბაიტების გამოსვლას გამოთვლით
  ერთეულები; ნაგულისხმევი დამუხტვა `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` ერთად
  `unit_label = "cu"`. ოჯახები იკვრება `price_family`-ით მანიფესტებში და
  აღსრულდა მიღებისას.
- აღრიცხვის ჩანაწერები შეიცავს `charged_units` პლუს ნედლეულის ციკლს/შემოსვლას/გამოსვლას/ხანგრძლივობას
  ჯამები შერიგებისთვის. გადასახადები ძლიერდება აღსრულების კლასით და
  დეტერმინიზმის მულტიპლიკატორები (`ComputePriceAmplifiers`) და დახურულია
  `compute.economics.max_cu_per_call`; გასვლა არის დაჭერილი მიერ
  `compute.economics.max_amplification_ratio` შეკრული პასუხის გაძლიერება.
- სპონსორების ბიუჯეტები (`ComputeCall::sponsor_budget_cu`) აღსრულებულია
  ერთ ზარზე/დღიურ კაპიტალს; დარიცხული ერთეული არ უნდა აღემატებოდეს დეკლარირებულ სპონსორის ბიუჯეტს.
- მმართველობის ფასების განახლებები იყენებს რისკის კლასის საზღვრებს
  `compute.economics.price_bounds` და საბაზისო ოჯახები ჩაწერილი
  `compute.economics.price_family_baseline`; გამოყენება
  `ComputeEconomics::apply_price_update` დელტას გადამოწმების მიზნით განახლებამდე
  აქტიური ოჯახის რუკა. გამოიყენეთ Torii კონფიგურაციის განახლებები
  `ConfigUpdate::ComputePricing` და kiso იყენებს მას იგივე საზღვრებით
  შევინარჩუნოთ მმართველობის რედაქტირება განმსაზღვრელი.

## კონფიგურაცია

ახალი გამოთვლითი კონფიგურაცია მუშაობს `crates/iroha_config/src/parameters`-ში:

- მომხმარებლის ხედი: `Compute` (`user.rs`) env გადაფარვით:
  - `COMPUTE_ENABLED` (ნაგულისხმევი `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- ფასი/ეკონომიკა: `compute.economics` გადაღებები
  `max_cu_per_call`/`max_amplification_ratio`, საკომისიოს გაყოფა, სპონსორის კაპიტალი
  (თითო ზარზე და დღიურ CU), საოჯახო ფასების საბაზისო ხაზები + რისკის კლასები/საზღვრები
  მმართველობის განახლებები და შესრულების კლასის მულტიპლიკატორები (GPU/TEE/best-effort).
- ფაქტობრივი/ნაგულისხმევი: `actual.rs` / `defaults.rs::compute` ექსპოზიცია გაანალიზებული
  `Compute` პარამეტრები (სახელთა სივრცეები, პროფილები, ფასების ოჯახები, ქვიშის ყუთი).
- არასწორი კონფიგურაციები (ცარიელი სახელების სივრცეები, ნაგულისხმევი პროფილი/ოჯახი აკლია, TTL ქუდი
  ინვერსიები) ამოღებულია როგორც `InvalidComputeConfig` პარსინგის დროს.

## ტესტები და მოწყობილობები

- განმსაზღვრელი დამხმარეები (`request_hash`, ფასი) და სარემონტო ტურები ცხოვრობენ
  `crates/iroha_data_model/src/compute/mod.rs` (იხ. `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- JSON მოწყობილობები ცხოვრობს `fixtures/compute/`-ში და ხორციელდება მონაცემთა მოდელის მიერ
  ტესტები რეგრესიის დაფარვისთვის.

## SLO აღკაზმულობა და ბიუჯეტი- `compute.slo.*` კონფიგურაცია ავლენს კარიბჭის SLO ღილაკებს (ფრენის რიგში
  სიღრმე, RPS ქუდი და ლატენტური სამიზნეები) in
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. ნაგულისხმევი: 32
  ფრენის დროს, 512 რიგში ერთ მარშრუტზე, 200 RPS, p50 25ms, p95 75ms, p99 120ms.
- გაუშვით მსუბუქი სკამების აღკაზმულობა SLO-ს შეჯამებების და მოთხოვნის/გასვლის გადასაღებად
  Snapshot: `cargo run -p xtask --bin compute_gateway -- bench [მანიფესტის_გზა]
  [იტერაციები] [თანამურობა] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 გამეორება, კონკურენტულობა 16, გამომავალი ქვეშ
  `artifacts/compute_gateway/bench_summary.{json,md}`). სკამი იყენებს
  დეტერმინისტული დატვირთვები (`fixtures/compute/payload_compute_payments.json`) და
  თითოეული მოთხოვნის სათაურები, რათა თავიდან აიცილოთ განმეორებითი შეჯახება ვარჯიშის დროს
  `echo`/`uppercase`/`sha3` შესასვლელი წერტილები.

## SDK/CLI პარიტეტის მოწყობილობები

- კანონიკური მოწყობილობები მუშაობს `fixtures/compute/`-ის ქვეშ: მანიფესტი, ზარი, დატვირთვა და
  კარიბჭის სტილის პასუხის/მიღების განლაგება. Payload ჰეშები უნდა ემთხვეოდეს ზარს
  `request.payload_hash`; დამხმარე ტვირთი ცხოვრობს
  `fixtures/compute/payload_compute_payments.json`.
- CLI აგზავნის `iroha compute simulate` და `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` ცხოვრობს
  `javascript/iroha_js/src/compute.js` რეგრესიის ტესტებით ქვეშ
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` იტვირთება იგივე მოწყობილობები, ამოწმებს დატვირთვის ჰეშებს,
  და ახდენს შესვლის წერტილების სიმულაციას ტესტებით
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift დამხმარეები ყველა იზიარებს ერთსა და იმავე Norito მოწყობილობებს, რათა SDK-ებმა შეძლონ
  დაადასტურეთ მოთხოვნის კონსტრუქცია და ჰეშის დამუშავება ხაზგარეშე a-ს დარტყმის გარეშე
  გაშვებული კარიბჭე.