---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.908615+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS სწრაფი გაშვება

ეს პრაქტიკული სახელმძღვანელო გადის დეტერმინისტულ SF-1 ცუნკერის პროფილში,
მანიფესტის ხელმოწერა და მრავალ პროვაიდერთან მოპოვების ნაკადი, რომელიც ემყარება SoraFS-ს
შენახვის მილსადენი. დააწყვილეთ იგი [მანიფესტური მილსადენის ღრმა ჩაყვინთვის] (manifest-pipeline.md)
დიზაინის შენიშვნებისთვის და CLI დროშის საცნობარო მასალისთვის.

## წინაპირობები

- Rust toolchain (`rustup update`), სამუშაო ადგილი ადგილობრივად კლონირებული.
- სურვილისამებრ: [OpenSSL გენერირებული Ed25519 გასაღებების წყვილი] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  მანიფესტების ხელმოწერისთვის.
- არასავალდებულო: Node.js ≥ 18 თუ გეგმავთ Docusaurus პორტალის წინასწარ გადახედვას.

დააყენეთ `export RUST_LOG=info` ექსპერიმენტების დროს გამოსადეგ CLI შეტყობინებებზე.

## 1. განაახლეთ დეტერმინისტული მოწყობილობები

აღადგინეთ კანონიკური SF-1 დაქუცმაცებული ვექტორები. ბრძანება ასევე გამოსცემს ხელმოწერილს
მანიფესტის კონვერტები `--signing-key`-ის მიწოდებისას; გამოიყენეთ `--allow-unsigned`
მხოლოდ ადგილობრივი განვითარების დროს.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

შედეგები:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (თუ ხელმოწერილია)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. დაჭერით ტვირთი და შეამოწმეთ გეგმა

გამოიყენეთ `sorafs_chunker` თვითნებური ფაილის ან არქივის დასაჭრელად:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

ძირითადი ველები:

- `profile` / `break_mask` - ადასტურებს `sorafs.sf1@1.0.0` პარამეტრებს.
- `chunks[]` – უბრძანა ოფსეტები, სიგრძე და BLAKE3-ის ნაწილაკები.

უფრო დიდი მოწყობილობებისთვის, აწარმოეთ პროტესტით მხარდაჭერილი რეგრესია, რათა უზრუნველყოთ ნაკადი და
სერიული დანაწევრება სინქრონში რჩება:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. შექმენით და მოაწერეთ ხელი მანიფესტს

შეფუთეთ ბლოკის გეგმა, მეტსახელები და მმართველობის ხელმოწერები მანიფესტში, გამოყენებით
`sorafs-manifest-stub`. ქვემოთ მოყვანილი ბრძანება გვიჩვენებს ერთი ფაილის დატვირთვას; გაივლის
დირექტორია გზა ხის შესაფუთად (CLI მას ლექსიკოგრაფიულად უვლის).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

გადახედეთ `/tmp/docs.report.json`:

- `chunking.chunk_digest_sha3_256` - SHA3 ოფსეტების/სიგრძეების შეჯამება, ემთხვევა
  ჩუნკერის მოწყობილობები.
- `manifest.manifest_blake3` – BLAKE3 დაიჯესტი ხელმოწერილია მანიფესტის კონვერტში.
- `chunk_fetch_specs[]` – შეუკვეთა მოტანის ინსტრუქციები ორკესტრებისთვის.

როდესაც მზად ხართ რეალური ხელმოწერების მიწოდებისთვის, დაამატეთ `--signing-key` და `--signer`
არგუმენტები. ბრძანება ამოწმებს ყველა Ed25519 ხელმოწერას დაწერამდე
კონვერტი.

## 4. მრავალპროვაიდერის მოძიების სიმულაცია

გამოიყენეთ დეველოპერი, რომ მოიტანოს CLI, რათა ხელახლა დაუკრათ ნაწილის გეგმა ერთი ან მეტის წინააღმდეგ
პროვაიდერები. ეს იდეალურია CI კვამლის ტესტებისა და ორკესტრის პროტოტიპებისთვის.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

განცხადებები:

- `payload_digest_hex` უნდა ემთხვეოდეს manifest-ის ანგარიშს.
- `provider_reports[]` ზედაპირების წარმატება/მარცხი ითვლის თითო პროვაიდერს.
- არა-ნულოვანი `chunk_retry_total` ხაზს უსვამს უკანა წნევის კორექტირებას.
- გაიარეთ `--max-peers=<n>`, რათა შეზღუდოთ პროვაიდერების რაოდენობა, რომლებიც დაგეგმილია გასაშვებად
  და შეინახეთ CI სიმულაციები ფოკუსირებული პირველად კანდიდატებზე.
- `--retry-budget=<n>` უგულებელყოფს ნაგულისხმევი ნაგულისხმევი განმეორებითი ცდის რაოდენობას (3), ასე რომ თქვენ
  შეუძლია ორკესტრის რეგრესია უფრო სწრაფად მოახდინოს წარუმატებლობის ინექციისას.

მარცხისთვის დაამატეთ `--expect-payload-digest=<hex>` და `--expect-payload-len=<bytes>`
სწრაფი, როდესაც რეკონსტრუქციული დატვირთვა გადახრის მანიფესტს.

## 5. შემდეგი ნაბიჯები

- **მმართველობის ინტეგრაცია** - მიიტანეთ მანიფესტის დაიჯესტი და
  `manifest_signatures.json` შევიდა საბჭოს სამუშაო პროცესში, რათა Pin Registry-მა შეძლოს
  ხელმისაწვდომობის რეკლამა.
- **რეესტრის შესახებ მოლაპარაკება** – გაიარეთ კონსულტაცია [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  ახალი პროფილების რეგისტრაციამდე. ავტომატიზაციას უპირატესობას ანიჭებს კანონიკურ სახელურებს
  (`namespace.name@semver`) რიცხვით ID-ებზე.
- **CI ავტომატიზაცია** - დაამატეთ ზემოთ მოცემული ბრძანებები მილსადენების გასათავისუფლებლად, რათა დოკუმენტები,
  მოწყობილობები და არტეფაქტები ხელმოწერილებთან ერთად აქვეყნებენ დეტერმინისტულ მანიფესტებს
  მეტამონაცემები.