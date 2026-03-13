---
lang: ka
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

სტატუსი: განხორციელდა და განხორციელდა Torii, CLI და ძირითადი დაშვების ტესტებით (ნოე. 2025).

## მიმოხილვა

- განათავსეთ კომპილირებული IVM ბაიტეკოდი (`.to`) Torii-ზე გაგზავნით ან გაცემით
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` ინსტრუქციები
  პირდაპირ.
- კვანძები ხელახლა გამოთვლიან `code_hash` და კანონიკურ ABI ჰეშს ადგილობრივად; შეუსაბამობები
  უარყოფენ დეტერმინისტულად.
- შენახული არტეფაქტები ცხოვრობენ `contract_manifests`-ის ჯაჭვის ქვეშ და
  `contract_code` რეესტრები. ავლენს მხოლოდ საცნობარო ჰეშებს და რჩება მცირე;
  კოდის ბაიტი ჩაწერილია `code_hash`-ით.
- დაცულ სახელთა სივრცეებს შეიძლება მოითხოვონ ამოქმედებული მმართველობის წინადადება ა
  განლაგება დასაშვებია. დაშვების გზა ეძებს წინადადების დატვირთვას და
  ახორციელებს `(namespace, contract_id, code_hash, abi_hash)` თანასწორობას, როდესაც
  სახელთა სივრცე დაცულია.

## შენახული არტეფაქტები და შენახვა

- `RegisterSmartContractCode` აყენებს/ჩაწერს მანიფესტს მოცემულისთვის
  `code_hash`. როდესაც იგივე ჰეში უკვე არსებობს, ის იცვლება ახლით
  მანიფესტი.
- `RegisterSmartContractBytes` ინახავს შედგენილ პროგრამას ქვეშ
  `contract_code[code_hash]`. თუ ჰეშის ბაიტები უკვე არსებობს, ისინი უნდა ემთხვეოდეს
  ზუსტად; განსხვავებული ბაიტი იწვევს უცვლელ დარღვევას.
- კოდის ზომა იფარება მორგებული პარამეტრით `max_contract_code_bytes`
  (ნაგულისხმევი 16 MiB). გააუქმეთ იგი ადრე `SetParameter(Custom)` ტრანზაქციის საშუალებით
  უფრო დიდი არტეფაქტების რეგისტრაცია.
- შენახვა შეუზღუდავია: მანიფესტები და კოდი ხელმისაწვდომი რჩება ცალსახად
  ამოღებულია სამომავლო მმართველობის პროცესში. არ არსებობს TTL ან ავტომატური GC.

## მისაღები მილსადენი

- ვალიდატორი აანალიზებს IVM სათაურს, ახორციელებს `version_major == 1`-ს და ამოწმებს
  `abi_version == 1`. უცნობი ვერსიები დაუყოვნებლივ უარყოფენ; არ არის გაშვების დრო
  გადართვა.
- როდესაც მანიფესტი უკვე არსებობს `code_hash`-ისთვის, ვალიდაცია უზრუნველყოფს
  შენახული `code_hash`/`abi_hash` უდრის გამოთვლილ მნიშვნელობებს წარმოდგენილიდან
  პროგრამა. შეუსაბამობა წარმოშობს `Manifest{Code,Abi}HashMismatch` შეცდომებს.
- დაცულ სახელთა სივრცეებზე გათვლილი ტრანზაქციები უნდა შეიცავდეს მეტამონაცემების გასაღებებს
  `gov_namespace` და `gov_contract_id`. მისაღები გზა ადარებს მათ
  ამოქმედებული `DeployContract` წინადადებების წინააღმდეგ; თუ შესატყვისი წინადადება არ არსებობს
  ტრანზაქცია უარყოფილია `NotPermitted`-ით.

## Torii საბოლოო წერტილები (მახასიათებელი `app_api`)- `POST /v2/contracts/deploy`
  - მოთხოვნის ტექსტი: `DeployContractDto` (იხილეთ `docs/source/torii_contracts_api.md` ველის დეტალებისთვის).
  - Torii დეკოდირებს base64 დატვირთვას, ითვლის ორივე ჰეშს, აშენებს მანიფესტს,
    და წარუდგენს `RegisterSmartContractCode` პლუსს
    `RegisterSmartContractBytes` ხელმოწერილი ტრანზაქციის სახელით
    გამრეკელი.
  - პასუხი: `{ ok, code_hash_hex, abi_hash_hex }`.
  - შეცდომები: არასწორი base64, მხარდაჭერილი ABI ვერსია, აკლია ნებართვა
    (`CanRegisterSmartContractCode`), ზომის ლიმიტის გადაჭარბება, მართვის კარიბჭე.
- `POST /v2/contracts/code`
  - იღებს `RegisterContractCodeDto` (ავტორიტეტი, პირადი გასაღები, მანიფესტი) და წარადგენს მხოლოდ
    `RegisterSmartContractCode`. გამოიყენეთ, როდესაც მანიფესტები დადგმულია ცალკე
    ბაიტიკოდი.
- `POST /v2/contracts/instance`
  - იღებს `DeployAndActivateInstanceDto` (ავტორიტეტი, პირადი გასაღები, სახელთა სივრცე/contract_id, `code_b64`, არჩევითი მანიფესტის უგულებელყოფა) და ავრცელებს + ააქტიურებს ატომურად.
- `POST /v2/contracts/instance/activate`
  - იღებს `ActivateInstanceDto` (ავტორიტეტი, პირადი გასაღები, სახელების სივრცე, contract_id, `code_hash`) და წარუდგენს მხოლოდ აქტივაციის ინსტრუქციას.
- `GET /v2/contracts/code/{code_hash}`
  - აბრუნებს `{ manifest: { code_hash, abi_hash } }`.
    დამატებითი მანიფესტის ველები შენახულია შინაგანად, მაგრამ აქ გამოტოვებული a
    სტაბილური API.
- `GET /v2/contracts/code-bytes/{code_hash}`
  - აბრუნებს `{ code_b64 }` შენახული `.to` გამოსახულებით, რომელიც დაშიფრულია როგორც base64.

კონტრაქტის სასიცოცხლო ციკლის ყველა საბოლოო წერტილი იზიარებს სპეციალურ განლაგების შემზღუდველს, რომელიც კონფიგურირებულია მეშვეობით
`torii.deploy_rate_per_origin_per_sec` (ჟეტონები წამში) და
`torii.deploy_burst_per_origin` (ადიდებული ჟეტონები). ნაგულისხმევი არის 4 req/s ერთად ადიდებული
8 `X-API-Token`-დან, დისტანციური IP-დან ან ბოლო წერტილის მინიშნებიდან მიღებული თითოეული ტოკენისთვის/გასაღებისთვის.
დააყენეთ რომელიმე ველი `null`, რათა გამორთოთ ლიმიტერი სანდო ოპერატორებისთვის. როცა
ლიმიტი ირთვება, Torii მატულობს
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` ტელემეტრიული მრიცხველი და
აბრუნებს HTTP 429; დამმუშავებლის შეცდომის ნებისმიერი ზრდა
`torii_contract_errors_total{endpoint=…}` გაფრთხილებისთვის.

## მმართველობის ინტეგრაცია და დაცული სახელების სივრცეები- დააყენეთ მორგებული პარამეტრი `gov_protected_namespaces` (სახელთა სივრცის JSON მასივი
  strings) დაშვების კარიბჭის გასააქტიურებლად. Torii ავლენს დამხმარეებს ქვეშ
  `/v2/gov/protected-namespaces` და CLI ასახავს მათ მეშვეობით
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- წინადადებები შექმნილი `ProposeDeployContract`-ით (ან Torii-ით
  `/v2/gov/proposals/deploy-contract` საბოლოო წერტილი) დაჭერა
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- რეფერენდუმის გავლის შემდეგ, `EnactReferendum` აღნიშნავს წინადადებას ამოქმედდა და
  დაშვება მიიღებს განლაგებას, რომელიც შეიცავს შესაბამის მეტამონაცემებს და კოდს.
- ტრანზაქციები უნდა მოიცავდეს მეტამონაცემების წყვილს `gov_namespace=a namespace` და
  `gov_contract_id=an identifier` (და უნდა დააყენოთ `contract_namespace` /
  `contract_id` ზარის დროის შესასრულებლად). CLI დამხმარეები ავსებენ მათ
  ავტომატურად, როდესაც გაივლით `--namespace`/`--contract-id`.
- როდესაც დაცული სახელთა სივრცეები ჩართულია, რიგში დაშვება უარყოფს მცდელობებს
  არსებული `contract_id` ხელახლა დააკავშირეთ სხვა სახელთა სივრცეში; გამოიყენეთ ამოქმედებული
  შესთავაზეთ ან გააუქმეთ წინა სავალდებულო მოქმედება სხვაგან განლაგებამდე.
- თუ ხაზის მანიფესტმა დაადგინა ვალიდატორის კვორუმი ერთზე მეტი, ჩართეთ
  `gov_manifest_approvers` (დამოწმების ანგარიშის ID-ების JSON მასივი), რათა რიგის დათვლა შეძლოს
  დამატებითი დამტკიცებები გარიგების ორგანოსთან ერთად. შესახვევებიც უარყოფენ
  მეტამონაცემები, რომლებიც მიმართავს სახელთა სივრცეებს, რომლებიც არ არის წარმოდგენილი manifest-ში
  `protected_namespaces` კომპლექტი.

## CLI დამხმარეები

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  წარუდგენს Torii განლაგების მოთხოვნას (ჰეშების გამოთვლა ფრენაზე).
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  აშენებს მანიფესტს (ხელმოწერილი მოწოდებული გასაღებით), აღრიცხავს ბაიტებს + მანიფესტს,
  და ააქტიურებს `(namespace, contract_id)` დაკავშირებას ერთ ტრანზაქციაში. გამოყენება
  `--dry-run` გამოთვლილი ჰეშებისა და ინსტრუქციების რაოდენობის დასაბეჭდად
  გაგზავნა და `--manifest-out` ხელმოწერილი მანიფესტის JSON შესანახად.
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` ითვლის
  `code_hash`/`abi_hash` კომპილირებული `.to`-ისთვის და სურვილისამებრ ხელს აწერს მანიფესტს,
  ბეჭდვა JSON ან წერა `--out`-ზე.
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  აწარმოებს ოფლაინ VM საშვს და აცნობებს ABI/hash მეტამონაცემებს პლუს რიგში მდგომ ISI-ებს
  (თვლები და ინსტრუქციის ID) ქსელთან შეხების გარეშე. მიამაგრეთ
  `--namespace/--contract-id` ზარის დროის მეტამონაცემების ასახვის მიზნით.
- `iroha_cli app contracts manifest get --code-hash <hex>` იღებს მანიფესტს Torii-ით
  და სურვილისამებრ წერს დისკზე.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` ჩამოტვირთვები
  შენახული `.to` სურათი.
- გააქტიურებულია `iroha_cli app contracts instances --namespace <ns> [--table]` სიები
  კონტრაქტის შემთხვევები (მანიფესტი + მეტამონაცემების საფუძველზე).
- მმართველობის დამხმარეები (`iroha_cli app gov deploy propose`, `iroha_cli app gov enact`,
  `iroha_cli app gov protected set/get`) ორგანიზებას უწევს დაცული სახელების სივრცის სამუშაო პროცესს და
  გამოავლინეთ JSON არტეფაქტები აუდიტისთვის.

## ტესტირება და გაშუქება

- ერთეულის ტესტები `crates/iroha_core/tests/contract_code_bytes.rs` საფარის კოდით
  შენახვა, იმპოტენცია და ზომის ქუდი.
- `crates/iroha_core/tests/gov_enact_deploy.rs` ადასტურებს მანიფესტის ჩასმას მეშვეობით
  ამოქმედება და `crates/iroha_core/tests/gov_protected_gate.rs` სავარჯიშოები
  დაცული სახელთა სივრცის დაშვება ბოლოდან ბოლომდე.
- Torii მარშრუტები მოიცავს მოთხოვნის/პასუხის ერთეულის ტესტებს და CLI ბრძანებებს აქვს
  ინტეგრაციის ტესტები უზრუნველყოფს JSON ორმხრივი მგზავრობის სტაბილურობას.

იხილეთ `docs/source/governance_api.md` რეფერენდუმის დეტალური დატვირთვისთვის და
კენჭისყრის სამუშაო პროცესები.