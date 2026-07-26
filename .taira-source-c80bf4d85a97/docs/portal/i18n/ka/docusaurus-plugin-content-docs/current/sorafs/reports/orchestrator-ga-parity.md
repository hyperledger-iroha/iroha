---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS ორკესტრი GA პარიტეტული ანგარიში

დეტერმინისტული მრავალჯერადი პარიტეტი ახლა თვალყურს ადევნებს SDK-ზე, ასე რომ, გამოშვების ინჟინრებს შეუძლიათ ამის დადასტურება
დატვირთვის ბაიტები, ქვითრები, პროვაიდერის ანგარიშები და შედეგები დაფიქსირდა
განხორციელებები. ყველა აღკაზმულობა მოიხმარს კანონიკურ მრავალპროვაიდერთა პაკეტს ქვეშ
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, რომელიც ავსებს SF1 გეგმას, პროვაიდერს
მეტამონაცემები, ტელემეტრიის სნეპშოტი და ორკესტრის ვარიანტები.

## ჟანგის საბაზისო ხაზი

- **ბრძანება:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **ფარგლები:** ახორციელებს `MultiPeerFixture` გეგმას ორჯერ მიმდინარე ორკესტრატორის მეშვეობით, ამოწმებს
  აწყობილი დატვირთვის ბაიტები, ქვითრები, პროვაიდერის ანგარიშები და შედეგების დაფის შედეგები. ინსტრუმენტაცია
  ასევე აკონტროლებს პიკს კონკურენტულობას და სამუშაო კომპლექტის ეფექტურ ზომას (`max_parallel × max_chunk_length`).
- ** შესრულების მცველი: ** ყოველი გაშვება უნდა დასრულდეს 2 წამში CI აპარატურაზე.
- **სამუშაო კომპლექტის ჭერი:** SF1 პროფილით აღკაზმულობა აძლიერებს `max_parallel = 3`-ს, რაც იძლევა
  ≤196608 ბაიტი ფანჯარა.

ჟურნალის ნიმუშის გამომავალი:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK აღკაზმულობა

- **ბრძანება:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- ** ფარგლები: ** იმეორებს იგივე მოწყობილობას `iroha_js_host::sorafsMultiFetchLocal`-ის საშუალებით, ადარებს დატვირთვას,
  ქვითრები, პროვაიდერის ანგარიშები და შედეგების დაფის სნეპშოტები ზედიზედ გაშვებებზე.
- ** შესრულების მცველი: ** თითოეული შესრულება უნდა დასრულდეს 2 წამში; აღკაზმულობა ბეჭდავს გაზომილს
  ხანგრძლივობა და რეზერვირებული ბაიტი ჭერი (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

შემაჯამებელი ხაზის მაგალითი:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK აღკაზმულობა

- **ბრძანება:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **ფარგლები:** აწარმოებს `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`-ში განსაზღვრულ პარიტეტულ კომპლექტს,
  SF1 მოწყობილობის ხელახლა დაკვრა ორჯერ Norito ხიდის მეშვეობით (`sorafsLocalFetch`). აღკაზმულობა ამოწმებს
  დატვირთვის ბაიტები, ქვითრები, პროვაიდერის ანგარიშები და ჩანაწერები დაფაზე იმავე დეტერმინისტიკის გამოყენებით
  პროვაიდერის მეტამონაცემები და ტელემეტრიის სნეპშოტები, როგორც Rust/JS კომპლექტები.
- **ხიდის ჩამტვირთავი:** აღკაზმულობა ხსნის `dist/NoritoBridge.xcframework.zip`-ს მოთხოვნისა და დატვირთვის შემთხვევაში
  macOS ნაჭერი `dlopen`-ის საშუალებით. როდესაც xcframework აკლია ან აკლია SoraFS აკინძვები, ის
  უბრუნდება `cargo build -p connect_norito_bridge --release`-ს და აკავშირებს წინააღმდეგ
  `target/release/libconnect_norito_bridge.dylib`, ამიტომ CI-ში ხელით დაყენება არ არის საჭირო.
- ** შესრულების მცველი: ** ყოველი შესრულება უნდა დასრულდეს 2 წამში CI აპარატურაზე; აღკაზმულობა ბეჭდავს
  გაზომილი ხანგრძლივობა და რეზერვირებული ბაიტი ჭერი (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

შემაჯამებელი ხაზის მაგალითი:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings აღკაზმულობა

- **ბრძანება:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **ფარგლები:** ახორციელებს მაღალი დონის `iroha_python.sorafs.multi_fetch_local` შეფუთვას და მის აკრეფას
  მონაცემთა კლასები ისე, რომ კანონიკური მოწყობილობა მიედინება იმავე API-ში, რომელსაც ბორბლების მომხმარებლები უწოდებენ. ტესტი
  აღადგენს პროვაიდერის მეტამონაცემებს `providers.json`-დან, ახდენს ტელემეტრიის კადრს და ამოწმებს
  დატვირთვის ბაიტები, ქვითრები, პროვაიდერის ანგარიშები და ქულების შინაარსი, ისევე როგორც Rust/JS/Swift
  ლუქსი.
- ** წინასწარი მოთხოვნა: ** გაუშვით `maturin develop --release` (ან დააინსტალირეთ საჭე), რათა `_crypto` გამოავლინოს
  `sorafs_multi_fetch_local` სავალდებულოა pytest-ის გამოძახებამდე; აღკაზმულობა ავტომატურად გამოტოვებს, როდესაც სავალდებულოა
  მიუწვდომელია.
- ** შესრულების მცველი: ** იგივე ≤2s ბიუჯეტი, როგორც Rust Suite; pytest აღრიცხავს აწყობილი ბაიტების რაოდენობას
  და პროვაიდერის მონაწილეობის შეჯამება გამოშვების არტეფაქტისთვის.

გამოშვების კარიბჭე უნდა ასახავდეს შემაჯამებელ გამომავალს ყველა აღკაზმიდან (Rust, Python, JS, Swift), ასე რომ
დაარქივებულ ანგარიშს შეუძლია ერთნაირად განასხვავოს ტვირთამწეობის ქვითრები და მეტრიკა, სანამ აწყობთ კონსტრუქციას. გაიქეცი
`ci/sdk_sorafs_orchestrator.sh` ყველა პარიტეტული კომპლექტის შესასრულებლად (Rust, Python bindings, JS, Swift)
ერთი საშვი; CI არტეფაქტებმა უნდა დაურთოს ჟურნალის ამონაწერი ამ დამხმარედან, პლუს გენერირებული
`matrix.md` (SDK/სტატუსის/ხანგრძლივობის ცხრილი) გამოშვების ბილეთზე, რათა მიმომხილველებმა შეძლონ პარიტეტის შემოწმება
მატრიცა ლუქსის ადგილობრივად განმეორების გარეშე.