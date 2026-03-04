---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-05T09:28:11.879581+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Chunking → Manifest Pipeline

სწრაფი სტარტის ეს კომპანიონი ადევნებს მილსადენს ბოლოდან ბოლომდე, რომელიც ნედლეულია
ბაიტები Norito მანიფესტებში, რომლებიც შესაფერისია SoraFS პინის რეესტრისთვის. შინაარსი არის
ადაპტირებულია [`docs/source/sorafs/manifest_pipeline.md`]-დან (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
იხილეთ ეს დოკუმენტი კანონიკური სპეციფიკაციისა და ცვლილებების ჟურნალისთვის.

## 1. ბლოკი დეტერმინისტულად

SoraFS იყენებს SF-1 (`sorafs.sf1@1.0.0`) პროფილს: FastCDC ინსპირირებული მოძრავი
ჰეში 64 KiB მინიმალური ბლოკის ზომით, 256 KiB სამიზნე, 512 KiB მაქსიმალური და
`0x0000ffff` შესვენების ნიღაბი. პროფილი რეგისტრირებულია
`sorafs_manifest::chunker_registry`.

### ჟანგის დამხმარეები

- `sorafs_car::CarBuildPlan::single_file` - ასხივებს ნაწილაკების ოფსეტებს, სიგრძეებს და
  BLAKE3 შეიმუშავებს CAR მეტამონაცემების მომზადებისას.
- `sorafs_car::ChunkStore` – გადასცემს დატვირთვას, აგრძელებს მეტამონაცემების ნაწილს და
  იღებს 64KiB / 4KiB Proof-of-Retrievability (PoR) შერჩევის ხეს.
- `sorafs_chunker::chunk_bytes_with_digests` – ბიბლიოთეკის დამხმარე ორივე CLI-ის უკან.

### CLI ინსტრუმენტები

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON შეიცავს მოწესრიგებულ ოფსეტებს, სიგრძეებს და ნაწილაკებს. გაგრძელდეს
დაგეგმეთ მანიფესტების აგებისას ან ორკესტრის მოპოვების სპეციფიკაციების დროს.

### PoR მოწმეები

`ChunkStore` ამხელს `--por-proof=<chunk>:<segment>:<leaf>` და
`--por-sample=<count>`, რათა აუდიტორებმა მოითხოვონ დეტერმინისტული მოწმეების ნაკრები. წყვილი
იმ დროშებით `--por-proof-out` ან `--por-sample-out` JSON-ის ჩასაწერად.

## 2. შეფუთეთ მანიფესტი

`ManifestBuilder` აერთიანებს მეტამონაცემებს მართვის დანართებთან:

- Root CID (dag-cbor) და CAR ვალდებულებები.
- მეტსახელის მტკიცებულებები და პროვაიდერის შესაძლებლობების პრეტენზიები.
- საბჭოს ხელმოწერები და არჩევითი მეტამონაცემები (მაგ., build ID).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

მნიშვნელოვანი შედეგები:

- `payload.manifest` – Norito-ში კოდირებული მანიფესტის ბაიტი.
- `payload.report.json` – ადამიანის/ავტომატიზაციის წაკითხვადი შეჯამება, მათ შორის
  `chunk_fetch_specs`, `payload_digest_hex`, CAR დაიჯესტები და მეტამონაცემების მეტამონაცემები.
- `payload.manifest_signatures.json` – კონვერტი, რომელიც შეიცავს მანიფესტ BLAKE3-ს
  დაიჯესტი, chunk-plan SHA3 დაიჯესტი და დალაგებულია Ed25519 ხელმოწერები.

გამოიყენეთ `--manifest-signatures-in` გარედან მოწოდებული კონვერტების შესამოწმებლად
ხელმომწერები მათ უკან დაწერამდე და `--chunker-profile-id` ან
`--chunker-profile=<handle>` რეესტრის არჩევანის დასაბლოკად.

## 3. გამოაქვეყნეთ და დაამაგრეთ

1. **მმართველობის წარდგენა ** – მიაწოდეთ მანიფესტის შეჯამება და ხელმოწერა
   კონვერტი საბჭოში, რათა ქინძისთავის დაშვება მოხდეს. გარე აუდიტორებმა უნდა
   შეინახეთ chunk-plan SHA3 დაიჯესტი მანიფესტ დიჯესთან ერთად.
2. **Pin payloads** – ატვირთეთ CAR არქივი (და სურვილისამებრ CAR ინდექსი) მითითებულ
   მანიფესტში პინის რეესტრში. დარწმუნდით, რომ მანიფესტი და CAR გაზიარებულია
   იგივე root CID.
3. **ჩაწერეთ ტელემეტრია** – შეასრულეთ JSON ანგარიში, PoR მოწმეები და ნებისმიერი მიღება
   მეტრიკა გამოშვების არტეფაქტებში. ეს ჩანაწერები კვებავს ოპერატორის დაფებს და
   დაეხმარეთ პრობლემების რეპროდუცირებას დიდი დატვირთვის ჩამოტვირთვის გარეშე.

## 4. მრავალპროვაიდერის მოპოვების სიმულაცია

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` ზრდის თითო პროვაიდერის პარალელიზმს (`#4` ზემოთ).
- `@<weight>` მელოდიების დაგეგმვის მიკერძოება; ნაგულისხმევად არის 1.
- `--max-peers=<n>` ზღუდავს პროვაიდერების რაოდენობას, რომლებიც დაგეგმილია გაშვებისთვის, როდესაც
  აღმოჩენა სასურველზე მეტ კანდიდატს მოაქვს.
- `--expect-payload-digest` და `--expect-payload-len` იცავენ ჩუმისგან
  კორუფცია.
- `--provider-advert=name=advert.to` ადრე ამოწმებს პროვაიდერის შესაძლებლობებს
  მათი გამოყენება სიმულაციაში.
- `--retry-budget=<n>` უგულებელყოფს თითო ნაწილზე ხელახალი ცდის რაოდენობას (ნაგულისხმევი: 3), ამიტომ CI
  წარუმატებლობის სცენარების ტესტირებისას შეუძლია რეგრესია უფრო სწრაფად წარმოაჩინოს.

`fetch_report.json` ზედაპირების აგრეგირებული მეტრიკა (`chunk_retry_total`,
`provider_failure_rate` და ა.შ.) შესაფერისია CI მტკიცებებისა და დაკვირვებისთვის.

## 5. რეესტრის განახლებები და მართვა

ჩუნკერის ახალი პროფილების შემოთავაზებისას:

1. აღწერის ავტორი `sorafs_manifest::chunker_registry_data`-ში.
2. განაახლეთ `docs/source/sorafs/chunker_registry.md` და მასთან დაკავშირებული წესდება.
3. მოწყობილობების რეგენერაცია (`export_vectors`) და აღბეჭდეთ ხელმოწერილი მანიფესტები.
4. წარადგინეთ წესდების შესაბამისობის ანგარიში მმართველობის ხელმოწერებით.

ავტომატიზაციას უნდა ურჩევნია კანონიკური სახელურები (`namespace.name@semver`) და დაეცემა