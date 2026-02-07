---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Chunking → Manifest Pipeline

Хурдан эхлэлийн энэ хамтрагч нь түүхий болж хувирдаг төгсгөлөөс төгсгөл хүртэлх дамжуулах хоолойг мөрддөг
SoraFS Pin Бүртгэлд тохирох Norito манифест руу байт. Агуулга нь
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)-аас тохируулсан;
Каноник тодорхойлолт болон өөрчлөлтийн бүртгэлийг авахын тулд тэр баримтаас лавлана уу.

## 1. Тодорхойлолттой хэсэг

SoraFS нь SF-1 (`sorafs.sf1@1.0.0`) профайлыг ашигладаг: FastCDC-аас санаа авсан гулсмал.
64KiB хамгийн бага хэмжээтэй хэш, зорилтот 256KiB, дээд тал нь 512KiB,
`0x0000ffff` эвдэрсэн маск. Профайл бүртгэлтэй байна
`sorafs_manifest::chunker_registry`.

### Зэвэнд туслагч

- `sorafs_car::CarBuildPlan::single_file` – Хэсэг офсет, урт, мөн ялгаруулдаг
  BLAKE3 нь CAR мета өгөгдлийг бэлтгэх явцад боловсруулдаг.
- `sorafs_car::ChunkStore` – Ачааллыг дамжуулж, хэсэгчилсэн мета өгөгдлийг хадгалах, мөн
  64KiB / 4KiB Retrievability Proof-of-Retrievability (PoR) түүврийн модыг гаргаж авдаг.
- `sorafs_chunker::chunk_bytes_with_digests` – Хоёр CLI-ийн ард байгаа номын сангийн туслах.

### CLI хэрэгсэл

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON нь эрэмбэлэгдсэн офсет, урт, хэсэгчилсэн хураангуйг агуулна. -г үргэлжлүүл
манифест байгуулахдаа төлөвлөх эсвэл найруулагч авчрах техникийн үзүүлэлтүүд.

### PoR гэрчүүд

`ChunkStore` нь `--por-proof=<chunk>:<segment>:<leaf>` ба
`--por-sample=<count>` тул аудиторууд тодорхой гэрчийн багцыг хүсэх боломжтой. Хос
JSON-г бичихийн тулд `--por-proof-out` эсвэл `--por-sample-out` бүхий эдгээр тугуудыг ашиглана уу.

## 2. Манифестийг боох

`ManifestBuilder` нь хэсэгчилсэн мета өгөгдлийг засаглалын хавсралттай хослуулсан:

- Root CID (dag-cbor) болон CAR амлалтууд.
- Нэрийн нотолгоо болон үйлчилгээ үзүүлэгчийн чадамжийн нэхэмжлэл.
- Зөвлөлийн гарын үсэг болон нэмэлт мета өгөгдөл (жишээ нь, бүтээх ID).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Чухал үр дүн:

- `payload.manifest` – Norito кодлогдсон манифест байт.
- `payload.report.json` – Хүн/автоматаар унших боломжтой хураангуй, үүнд
  `chunk_fetch_specs`, `payload_digest_hex`, CAR мэдээ болон бусад нэр мета өгөгдөл.
- `payload.manifest_signatures.json` – BLAKE3 манифест агуулсан дугтуй
  digest, SHA3 digest-ийг хэсэгчлэн төлөвлөх, Ed25519 гарын үсгийг эрэмбэлэх.

Гаднаас нийлүүлсэн дугтуйг шалгахын тулд `--manifest-signatures-in` ашиглана уу
гарын үсэг зурсан хүмүүс тэдгээрийг буцааж бичихээс өмнө, мөн `--chunker-profile-id` эсвэл
Бүртгэлийн сонголтыг түгжихийн тулд `--chunker-profile=<handle>`.

## 3. Нийтэлж, бэхлэх

1. **Засаглалын танилцуулга** – Манифест тойм болон гарын үсэг өгнө
   дугтуйг зөвлөлд илгээж, зүүг хүлээн авах боломжтой. Гадны аудиторууд хийх ёстой
   SHA3-ийн багц төлөвлөгөөг манифест дижесттэй хамт хадгална.
2. **Ачааллыг зүүх** – Ашигласан CAR архивыг (болон нэмэлт CAR индекс) байршуулах
   Pin бүртгэлийн манифест дотор. Манифест болон CAR-г хуваалцаж байгаа эсэхийг шалгаарай
   ижил үндэс CID.
3. **Телеметрийн бичлэг хийх** – JSON тайлан, PoR гэрчүүд болон аливаа татан авалтыг үргэлжлүүлэх
   хувилбарын олдвор дахь хэмжүүр. Эдгээр бичлэгүүд нь операторын хяналтын самбар болон
   их хэмжээний ачааллыг татаж авахгүйгээр асуудлыг дахин боловсруулахад тусална.

## 4. Олон үйлчилгээ үзүүлэгчийн татан авалтын симуляци

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` үйлчилгээ үзүүлэгчийн параллелизмыг нэмэгдүүлдэг (дээрх `#4`).
- `@<weight>` хуваарийн хазайлтыг тааруулдаг; өгөгдмөл нь 1.
- `--max-peers=<n>` нь хэзээ ажиллуулахаар төлөвлөсөн үйлчилгээ үзүүлэгчдийн тоог хязгаарладаг
  нээлт нь хүссэнээс илүү олон нэр дэвшигчийг авчирдаг.
- `--expect-payload-digest` ба `--expect-payload-len` чимээгүй байдлаас хамгаална
  авлига.
- `--provider-advert=name=advert.to` үйлчилгээ үзүүлэгчийн чадавхийг өмнө нь шалгадаг
  тэдгээрийг симуляцид ашиглах.
- `--retry-budget=<n>` хэсэг болгон дахин оролдох тоог (өгөгдмөл: 3) хүчингүй болгодог тул CI
  бүтэлгүйтлийн хувилбаруудыг турших үед регрессийг илүү хурдан гаргаж чадна.

`fetch_report.json` гадаргууг нэгтгэсэн хэмжигдэхүүнүүд (`chunk_retry_total`,
`provider_failure_rate` гэх мэт) CI баталгаажуулалт болон ажиглалтад тохиромжтой.

## 5. Бүртгэлийн шинэчлэлт ба засаглал

Шинэ chunker профайлыг санал болгохдоо:

1. `sorafs_manifest::chunker_registry_data`-д тодорхойлогчийг зохиогч.
2. `docs/source/sorafs/chunker_registry.md` болон холбогдох дүрмийг шинэчлэх.
3. Бэхэлгээг (`export_vectors`) сэргээж, гарын үсэг зурсан манифестуудыг аваарай.
4. Дүрмийн хэрэгжилтийн тайланг засаглалын гарын үсэгтэй хамт ирүүлнэ.

Автоматжуулалт нь каноник бариулыг (`namespace.name@semver`) илүүд үзэж, унах ёстой