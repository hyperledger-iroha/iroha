---
id: node-operations
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
სარკეები `docs/source/sorafs/runbooks/sorafs_node_ops.md`. შეინახეთ ორივე ვერსია სინქრონიზებული სფინქსის ნაკრების ამოღებამდე.
:::

## მიმოხილვა

ეს სახელმძღვანელო ოპერატორებს ამოწმებს ჩაშენებული `sorafs-node` განლაგების ვალიდაციაში Torii-ში. თითოეული განყოფილება პირდაპირ ასახავს SF-3 მიწოდებას: ორმხრივი მოგზაურობის დამაგრება/მოტანა, აღდგენა, კვოტის უარყოფა და PoR-ის შერჩევა.

## 1. წინაპირობები

- ჩართეთ შენახვის მუშაკი `torii.sorafs.storage`-ში:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- დარწმუნდით, რომ Torii პროცესს აქვს წაკითხვის/ჩაწერის წვდომა `data_dir`-ზე.
- დეკლარაციის ჩაწერის შემდეგ დაადასტურეთ, რომ კვანძი აცხადებს მოსალოდნელ სიმძლავრეს `GET /v2/sorafs/capacity/state`-ის საშუალებით.
- როდესაც გამარტივება ჩართულია, საინფორმაციო დაფები ავლენს როგორც ნედლეულ, ისე გათლილ GiB·hour/PoR მრიცხველებს, რათა ხაზი გაუსვას უძრაო ტენდენციებს წერტილოვან მნიშვნელობებთან ერთად.

### CLI მშრალი გაშვება (სურვილისამებრ)

HTTP ბოლო წერტილების გამოვლენამდე, თქვენ შეგიძლიათ შეამოწმოთ შენახვის საფონდო სისტემა შეფუთული CLI-ით.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

ბრძანებები ბეჭდავს Norito JSON-ის შეჯამებებს და უარს იტყვის ნაწილ-პროფილის ან დაიჯესტის შეუსაბამობებზე, რაც მათ გამოსადეგს ხდის CI კვამლის შემოწმებისთვის Torii გაყვანილობის წინ.【crates/sorafs_node/tests/cli.rs#L1】

### PoR მტკიცებულების რეპეტიცია

ოპერატორებს ახლა შეუძლიათ გაიმეორონ მმართველობით გამოშვებული PoR არტეფაქტები ადგილობრივად, სანამ ატვირთავენ მათ Torii-ზე. CLI ხელახლა იყენებს იმავე `sorafs-node` გადატანის გზას, ამიტომ ადგილობრივი გაშვებები ასახავს ვალიდაციის ზუსტ შეცდომებს, რომლებსაც დააბრუნებს HTTP API.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

ბრძანება გამოსცემს JSON შეჯამებას (მანიფესტის შეჯამება, პროვაიდერის id, მტკიცებულების შეჯამება, ნიმუშების რაოდენობა, არჩევითი ვერდიქტის შედეგი). მიაწოდეთ `--manifest-id=<hex>`, რათა დარწმუნდეთ, რომ შენახული მანიფესტი ემთხვევა გამოწვევის შეჯამებას, და `--json-out=<path>`, როდესაც გსურთ დაარქივოთ რეზიუმე ორიგინალური არტეფაქტებით აუდიტის მტკიცებულებისთვის. `--verdict`-ის ჩათვლით საშუალებას გაძლევთ გაიმეოროთ მთელი გამოწვევა → მტკიცებულება → განაჩენის ციკლი ხაზგარეშე HTTP API-ზე დარეკვამდე.

მას შემდეგ, რაც Torii ცოცხალი იქნება, შეგიძლიათ იგივე არტეფაქტების მოძიება HTTP-ის საშუალებით:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

ორივე ბოლო წერტილს ემსახურება ჩაშენებული შენახვის მუშაკი, ამიტომ CLI კვამლის ტესტები და კარიბჭის ზონდები სინქრონიზებული რჩება.

## 2. ჩამაგრება → ორმხრივი მოგზაურობის მიღება

1. შექმენით manifest + payload პაკეტი (მაგალითად, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`-ით).
2. გაგზავნეთ manifest base64 კოდირებით:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   მოთხოვნა JSON უნდა შეიცავდეს `manifest_b64` და `payload_b64`. წარმატებული პასუხი აბრუნებს `manifest_id_hex`-ს და დატვირთვის დაჯესტს.
3. მიიღეთ ჩამაგრებული მონაცემები:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-გაშიფრეთ `data_b64` ველი და შეამოწმეთ, რომ იგი ემთხვევა თავდაპირველ ბაიტებს.

## 3. გადატვირთეთ აღდგენის საბურღი

1. დაამაგრეთ მინიმუმ ერთი მანიფესტი, როგორც ზემოთ.
2. გადატვირთეთ Torii პროცესი (ან მთელი კვანძი).
3. ხელახლა გაგზავნეთ მოთხოვნის მიღება. ტვირთამწეობა კვლავ უნდა იყოს აღდგენილი და დაბრუნებული დაიჯესტი უნდა ემთხვეოდეს წინასწარ გადატვირთვის მნიშვნელობას.
4. შეამოწმეთ `GET /v2/sorafs/storage/state`, რათა დაადასტუროთ, რომ `bytes_used` ასახავს მუდმივ მანიფესტებს გადატვირთვის შემდეგ.

## 4. კვოტის უარყოფის ტესტი

1. დროებით შეამცირეთ `torii.sorafs.storage.max_capacity_bytes` მცირე მნიშვნელობამდე (მაგალითად, ერთი მანიფესტის ზომა).
2. დაამაგრეთ ერთი მანიფესტი; მოთხოვნა უნდა შესრულდეს.
3. შეეცადეთ დაამაგროთ მსგავსი ზომის მეორე მანიფესტი. Torii-მა უნდა უარყოს მოთხოვნა HTTP `400`-ით და შეცდომის შეტყობინება, რომელიც შეიცავს `storage capacity exceeded`.
4. აღადგინეთ ნორმალური სიმძლავრის ლიმიტი დასრულების შემდეგ.

## 5. შეკავება / GC ინსპექტირება (მხოლოდ წაკითხვადი)

1. გაუშვით ლოკალური შეკავების სკანირება შენახვის დირექტორიაში:

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. შეამოწმეთ მხოლოდ ვადაგასული მანიფესტები (მხოლოდ მშრალი გაშვება, წაშლის გარეშე):

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. გამოიყენეთ `--now` ან `--grace-secs` შეფასების ფანჯრის დასამაგრებლად მასპინძლების ან ინციდენტების ანგარიშების შედარებისას.

GC CLI არის განზრახ მხოლოდ წაკითხვადი. გამოიყენეთ იგი აუდიტის ბილიკების შენახვის ვადების და ვადაგასული მანიფესტების აღრიცხვისთვის; არ წაშალოთ მონაცემები ხელით წარმოებაში.

## 6. PoR ნიმუშის ზონდი

1. დაამაგრეთ მანიფესტი.
2. მოითხოვეთ PoR ნიმუში:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. გადაამოწმეთ, რომ პასუხი შეიცავს `samples` მოთხოვნილ რაოდენობას და რომ თითოეული მტკიცებულება დადასტურებულია შენახული მანიფესტის ფესვთან მიმართებაში.

## 7. ავტომატიზაციის კაკვები

- CI / კვამლის ტესტებს შეუძლიათ ხელახლა გამოიყენონ დამატებული მიზნობრივი შემოწმებები:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  რომელიც მოიცავს `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` და `por_sampling_returns_verified_proofs`.
- დაფები უნდა აკონტროლონ:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` და `torii_sorafs_storage_fetch_inflight`
  - PoR წარმატების/მარცხის მრიცხველები გამოჩნდა `/v2/sorafs/capacity/state`-ის საშუალებით
  - ანგარიშსწორების გამოქვეყნების მცდელობები `sorafs_node_deal_publish_total{result=success|failure}`-ის საშუალებით

ამ წვრთნების შემდეგ, ჩაშენებული შენახვის მუშაკს შეუძლია მიიღოს მონაცემები, გადარჩეს გადატვირთვა, პატივი სცეს კონფიგურირებულ კვოტებს და გამოიმუშაოს დეტერმინისტული PoR მტკიცებულებები, სანამ კვანძი განაცხადებს სიმძლავრეს ფართო ქსელში.