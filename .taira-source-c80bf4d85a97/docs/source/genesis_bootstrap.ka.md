---
lang: ka
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis Bootstrap სანდო თანატოლებისგან

Iroha თანატოლებს ადგილობრივი `genesis.file`-ის გარეშე შეუძლიათ მიიღონ ხელმოწერილი გენეზის ბლოკი სანდო თანატოლებისგან
Norito-ში კოდირებული bootstrap პროტოკოლის გამოყენებით.

- **პროტოკოლი:** თანატოლები ცვლიან `GenesisRequest` (`Preflight` მეტამონაცემებისთვის, `Fetch` დატვირთვისთვის) და
  `GenesisResponse` ჩარჩოები ჩასმული `request_id`-ით. რესპონდენტებში შედის ჯაჭვის ID, ხელმომწერი pubkey,
  ჰეში და სურვილისამებრ ზომის მინიშნება; ტვირთამწეობა ბრუნდება მხოლოდ `Fetch`-ზე და დუბლიკატი მოთხოვნის ID-ზე
  მიიღეთ `DuplicateRequest`.
- **მცველები:** მოპასუხეები აღასრულებენ უფლებათა სიას (`genesis.bootstrap_allowlist` ან სანდო თანატოლები
  ნაკრები), chain-id/pubkey/hash შესატყვისი, განაკვეთის ლიმიტები (`genesis.bootstrap_response_throttle`) და
  ზომის თავსახური (`genesis.bootstrap_max_bytes`). მოთხოვნები დაშვებული სიის გარეთ მიიღება `NotAllowed` და
  არასწორი გასაღებით ხელმოწერილი ტვირთი იღებს `MismatchedPubkey`.
- **მომთხოვნის ნაკადი:** როცა მეხსიერება ცარიელია და `genesis.file` არ არის დაყენებული (და
  `genesis.bootstrap_enabled=true`), კვანძი აგზავნის სანდო თანატოლებს სურვილისამებრ
  `genesis.expected_hash`, შემდეგ იღებს დატვირთვას, ამოწმებს ხელმოწერებს `validate_genesis_block`-ის საშუალებით,
  და რჩება `genesis.bootstrap.nrt` კურასთან ერთად ბლოკის გამოყენებამდე. Bootstrap ხელახლა ცდილობს
  პატივი `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval` და
  `genesis.bootstrap_max_attempts`.
- ** წარუმატებლობის რეჟიმები: ** მოთხოვნები უარყოფილია დაშვებული სიის გამოტოვებისთვის, ჯაჭვის/პუბლიკის/ჰეშის შეუსაბამობისთვის, ზომისთვის
  ლიმიტის დარღვევა, ტარიფების ლიმიტები, გამოტოვებული ლოკალური გენეზისი ან დუბლიკატი მოთხოვნის ID. კონფლიქტური ჰეშები
  თანატოლებს შორის შეწყვიტოს მიღება; არცერთი პასუხისმგებელი/დროის ამოწურვა არ უბრუნდება ადგილობრივ კონფიგურაციას.
- **ოპერატორის ნაბიჯები:** დარწმუნდით, რომ მინიმუმ ერთი სანდო თანატოლი ხელმისაწვდომია სწორი გენეზისით, კონფიგურაცია
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` და ხელახლა სცადეთ სახელურები, და
  სურვილისამებრ დაამაგრეთ `expected_hash`, რათა თავიდან აიცილოთ შეუსაბამო ტვირთის მიღება. მუდმივი დატვირთვა შეიძლება იყოს
  ხელახლა გამოიყენება მომდევნო ჩექმებზე `genesis.file`-ზე `genesis.bootstrap.nrt`-ზე მითითებით.