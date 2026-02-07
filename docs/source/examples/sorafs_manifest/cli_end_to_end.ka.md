---
lang: ka
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS მანიფესტის CLI ბოლოდან ბოლომდე მაგალითი

ეს მაგალითი განიხილავს დოკუმენტაციის build-ის გამოქვეყნებას SoraFS-ზე, გამოყენებით
`sorafs_manifest_stub` CLI განმსაზღვრელ სქელ დანამატებთან ერთად
აღწერილია SoraFS Architecture RFC-ში. ნაკადი მოიცავს მანიფესტ თაობას,
მოლოდინების შემოწმება, მოპოვების გეგმის ვალიდაცია და დადასტურების მოძიების რეპეტიცია
გუნდებს შეუძლიათ იგივე ნაბიჯების ჩასმა CI-ში.

## წინაპირობები

- სამუშაო სივრცე კლონირებული და ხელსაწყოების ჯაჭვის მზადყოფნა (`cargo`, `rustc`).
- მოწყობილობები `fixtures/sorafs_chunker`-დან ხელმისაწვდომია, რათა მოლოდინის მნიშვნელობები იყოს
  მიღებული (წარმოების გაშვებებისთვის, ამოიღეთ მნიშვნელობები მიგრაციის წიგნის ჩანაწერიდან
  ასოცირდება არტეფაქტთან).
- სანიმუშო payload დირექტორია გამოქვეყნებისთვის (ეს მაგალითი იყენებს `docs/book`).

## ნაბიჯი 1 - შექმენით მანიფესტი, CAR, ხელმოწერები და მიიღეთ გეგმა

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

ბრძანება:

- გადასცემს დატვირთვას `ChunkProfile::DEFAULT`-ის მეშვეობით.
- ავრცელებს CARv2 არქივს, პლუს ამოღების გეგმას.
- აშენებს `ManifestV1` ჩანაწერს, ამოწმებს მანიფესტის ხელმოწერებს (თუ მოწოდებულია) და
  წერს კონვერტში.
- ახორციელებს მოლოდინის დროშებს ისე, რომ გაშვება ჩავარდება, თუ ბაიტები გადაინაცვლებს.

## ნაბიჯი 2 - გადაამოწმეთ შედეგები chunk store + PoR რეპეტიციით

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

ეს იმეორებს CAR-ს დეტერმინისტული ბლოკის მაღაზიის მეშვეობით, გამომდინარეობს
აღდგენის დამადასტურებელი ნიმუშის ხე და ავრცელებს მანიფესტ ანგარიშს, რომელიც შესაფერისია
მმართველობის მიმოხილვა.

## ნაბიჯი 3 - მრავალპროვაიდერის მოძიების სიმულაცია

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI გარემოებისთვის, მიაწოდეთ ცალკეული დატვირთვის ბილიკები თითო პროვაიდერზე (მაგ., დამონტაჟებული
მოწყობილობები) სავარჯიშო დიაპაზონის დაგეგმვა და წარუმატებლობის დამუშავება.

## ნაბიჯი 4 - ჩაწერეთ წიგნის ჩანაწერი

ჩაწერეთ პუბლიკაცია `docs/source/sorafs/migration_ledger.md`-ში, აღბეჭდეთ:

- მანიფესტი CID, CAR დაიჯესტი და საბჭოს ხელმოწერების ჰეში.
- სტატუსი (`Draft`, `Staging`, `Pinned`).
- CI გაშვებების ან მმართველობის ბილეთების ბმულები.

## ნაბიჯი 5 - ჩამაგრება მართვის ინსტრუმენტების მეშვეობით (როდესაც რეესტრი ცოცხალია)

პინის რეესტრის განლაგების შემდეგ (მიგრაციის საგზაო რუკაში Milestone M2),
გაგზავნეთ მანიფესტი CLI-ის მეშვეობით:

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

წინადადების იდენტიფიკატორი და შემდგომი დამტკიცების ტრანზაქციის ჰეშები უნდა იყოს
აღბეჭდილია მიგრაციის წიგნის ჩანაწერში აუდიტის შესამოწმებლად.

## დასუფთავება

`target/sorafs/` ქვეშ არსებული არტეფაქტები შეიძლება დაარქივდეს ან აიტვირთოს დადგმულ კვანძებში.
შეინახეთ მანიფესტი, ხელმოწერები, CAR და ამოღების გეგმა ერთად ასე ქვემოთ
ოპერატორებს და SDK გუნდებს შეუძლიათ დაადასტურონ განლაგება დეტერმინისტულად.