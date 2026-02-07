---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0d99cea6198ef7ea6c75d7823854237983f17c3341cea2b3e491bb03e54531f2
source_last_modified: "2026-01-22T14:35:36.797300+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
---

::: Каноник эх сурвалжийг анхаарна уу
`docs/source/sorafs/runbooks/sorafs_node_ops.md` толь. Сфинксийн багцыг ашиглах хүртэл хоёр хувилбарыг синхрончлолд байлга.
:::

## Тойм

Энэхүү runbook нь Torii дотор суулгагдсан `sorafs-node` суулгацыг баталгаажуулах замаар операторуудыг заадаг. Хэсэг бүр нь SF-3-ийн үр дүнг шууд харуулдаг: эргэх/татаж авах, сэргээх, дахин эхлүүлэх, квотоос татгалзах, PoR дээж авах.

## 1. Урьдчилсан нөхцөл

- `torii.sorafs.storage`-д хадгалагчийг идэвхжүүлэх:

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

- Torii процесс нь `data_dir` руу унших/бичих эрхтэй эсэхийг шалгаарай.
- Мэдэгдэл бичигдсэний дараа зангилаа хүлээгдэж буй хүчин чадлыг `GET /v1/sorafs/capacity/state`-ээр дамжуулан сурталчилж байгааг баталгаажуулна уу.
- Гөлгөржүүлэх идэвхжсэн үед хяналтын самбар нь спот утгын хажуугаар чичиргээгүй чиг хандлагыг тодотгохын тулд түүхий болон жигдрүүлсэн GiB·цаг/PoR тоолуурыг хоёуланг нь ил гаргадаг.

### CLI хуурай гүйлт (заавал биш)

HTTP төгсгөлийн цэгүүдийг харуулахын өмнө та багцалсан CLI-ийн тусламжтайгаар санах ойн арын хэсгийг эрүүл мэндийн байдлыг шалгаж болно.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Командууд нь Norito JSON-ийн хураангуйг хэвлэж, хэсэгчилсэн профайл эсвэл таарахгүй байхаас татгалзаж, Torii утсыг холбохоос өмнө CI утаа шалгахад тустай болгодог.【crates/sorafs_node/tests/cli.rs#L1】

### PoR Баталгаажсан давтлага

Операторууд одоо Torii-д байршуулахаас өмнө засаглалаас олгосон PoR олдворуудыг дахин тоглуулах боломжтой. CLI нь ижил `sorafs-node` залгих замыг дахин ашигладаг тул локал гүйлтүүд нь HTTP API-ийн буцаах баталгаажуулалтын алдааг гаргадаг.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Энэ тушаал нь JSON хураангуй (манифест дижест, үйлчилгээ үзүүлэгчийн ID, нотлох баримт, дээжийн тоо, нэмэлт шийдвэрийн үр дүн) гаргадаг. Хадгалсан манифест нь сорилтын тоймд таарч байгаа эсэхийг `--manifest-id=<hex>`, аудитын нотлох баримтын эх олдворын хамт хураангуйг архивлахыг хүсвэл `--json-out=<path>`-г оруулна уу. `--verdict`-г оруулснаар та HTTP API-г дуудахаасаа өмнө бүх сорилт → нотлох баримт → шийдвэрийн давталтыг офлайнаар давтах боломжийг олгоно.

Torii ажиллаж эхэлмэгц та HTTP-ээр дамжуулан ижил олдворуудыг татаж авах боломжтой:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Хоёр төгсгөлийн цэгийг суулгагдсан хадгалалтын ажилтан үйлчилдэг тул CLI утааны тест болон гарцын мэдрэгч нь синхрончлолд байдаг.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#1】L

## 2. Pin → Хоёр талын аялалыг татах

1. Манифест + ачааны багц (жишээ нь `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`) үүсгэнэ үү.
2. Манифестийг base64 кодчилолоор илгээнэ үү:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON хүсэлт нь `manifest_b64` болон `payload_b64` агуулсан байх ёстой. Амжилттай хариу үйлдэл нь `manifest_id_hex` болон ачааллын мэдээг буцаана.
3. Буулгасан өгөгдлийг дуудах:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-`data_b64` талбарыг тайлж, анхны байттай таарч байгаа эсэхийг шалгана уу.

## 3. Сэргээх дасгалыг дахин эхлүүлнэ үү

1. Дор хаяж нэг манифестийг дээрх шиг тогтооно.
2. Torii процессыг (эсвэл бүхэл бүтэн зангилааг) дахин эхлүүлнэ үү.
3. Татаж авах хүсэлтийг дахин илгээнэ үү. Ачааллыг сэргээх боломжтой хэвээр байх ёстой бөгөөд буцаасан мэдээ нь дахин эхлүүлэхийн өмнөх утгатай тохирч байх ёстой.
4. `bytes_used` дахин ачаалсны дараа үргэлжилсэн манифестуудыг тусгаж байгааг баталгаажуулахын тулд `GET /v1/sorafs/storage/state`-г шалгана уу.

## 4. Квотоос татгалзах шалгалт

1. `torii.sorafs.storage.max_capacity_bytes`-г бага утгаар (жишээ нь нэг манифестын хэмжээ) түр бууруулна уу.
2. Нэг манифест зүүх; хүсэлт амжилттай байх ёстой.
3. Ижил хэмжээтэй хоёр дахь манифестийг бэхлэхийг оролдох. Torii нь HTTP `400` болон `storage capacity exceeded` агуулсан алдааны мессеж бүхий хүсэлтээс татгалзах ёстой.
4. Дуусмагц хэвийн хүчин чадлын хязгаарыг сэргээнэ.

## 5. Хадгалах / GC Inspection (Зөвхөн унших)

1. Хадгалах лавлахын эсрэг локал хадгалах сканыг ажиллуулна уу:

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. Зөвхөн хугацаа нь дууссан манифестуудыг шалгана уу (зөвхөн хуурай, устгахгүй):

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. Хостууд болон тохиолдлуудын тайлангуудыг харьцуулахдаа `--now` эсвэл `--grace-secs` ашиглан үнэлгээний цонхыг тогтооно.

GC CLI нь зөвхөн уншихад зориулагдсан. Хадгалах эцсийн хугацаа болон аудитын мөрийн хугацаа дууссан манифестийн бараа материалыг авахын тулд үүнийг ашиглах; үйлдвэрлэлд өгөгдлийг гараар устгаж болохгүй.

## 6. PoR дээж авах датчик

1. Манифестыг бэхлэх.
2. PoR дээжийг хүсэх:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Хариулт нь хүссэн тоогоор `samples`-г агуулж байгаа бөгөөд нотлох баримт бүр хадгалагдсан манифест үндэстэй тулгаж байгаа эсэхийг шалгана уу.

## 7. Автоматжуулалтын дэгээ

- CI / утааны туршилтууд нь дараахь зүйлд нэмсэн зорилтот шалгалтуудыг дахин ашиглах боломжтой.

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  Энэ нь `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, `por_sampling_returns_verified_proofs`-ийг хамардаг.
- Хяналтын самбар нь дараахь зүйлийг хянах ёстой.
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` ба `torii_sorafs_storage_fetch_inflight`
  - PoR амжилт/бүтэлгүйтлийн тоолуур `/v1/sorafs/capacity/state`-ээр гарч ирэв
  - `sorafs_node_deal_publish_total{result=success|failure}`-ээр төлбөр тооцоо нийтлэх оролдлого

Эдгээр дасгалуудыг дагаснаар суулгагдсан хадгалалтын ажилтан нь датаг залгих, дахин эхлүүлэх үед амьд үлдэх, тохируулсан квотыг хүндэтгэх, зангилаа өргөн сүлжээнд хүчин чадлаа сурталчлахаас өмнө тодорхой PoR нотолгоо үүсгэх боломжтой болно.