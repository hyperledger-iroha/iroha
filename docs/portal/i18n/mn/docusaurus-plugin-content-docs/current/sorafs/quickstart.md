---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Түргэн эхлүүлэх

Энэхүү практик гарын авлага нь тодорхойлогч SF-1 chunker профайлаар дамждаг.
SoraFS-ийн үндэс болсон манифест гарын үсэг, олон үйлчилгээ үзүүлэгчийн татах урсгал
хадгалах дамжуулах хоолой. Үүнийг [манифест дамжуулах хоолойн гүнд шумбах](manifest-pipeline.md)-тай хослуул.
дизайны тэмдэглэл болон CLI тугны лавлах материалын хувьд.

## Урьдчилсан нөхцөл

- Rust toolchain (`rustup update`), ажлын талбарыг орон нутагт хуваасан.
- Нэмэлт: [OpenSSL үүсгэсэн Ed25519 товчлуур](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  манифестэд гарын үсэг зурахад.
- Нэмэлт: Хэрэв та Docusaurus порталыг урьдчилан үзэхээр төлөвлөж байгаа бол Node.js ≥ 18.

Хэрэгтэй CLI мессежүүдийг гаргахын тулд туршилт хийж байхдаа `export RUST_LOG=info`-г тохируулна уу.

## 1. Тодорхойлогч бэхэлгээг сэргээ

Каноник SF-1 ангилах векторуудыг дахин үүсгэнэ. Тушаал нь мөн гарын үсэг зурдаг
`--signing-key` нийлүүлэх үед манифест дугтуй; `--allow-unsigned` ашиглах
зөвхөн орон нутгийн хөгжлийн явцад.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Гаралт:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (хэрэв гарын үсэг зурсан бол)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Ачаатай ачааг хувааж, төлөвлөгөөг шалга

Дурын файл эсвэл архивыг хуваахын тулд `sorafs_chunker` ашиглана уу:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Түлхүүр талбарууд:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` параметрүүдийг баталгаажуулна.
- `chunks[]` – захиалгат офсет, урт, хэсэгчилсэн BLAKE3 дижест.

Томоохон бэхэлгээний хувьд дамжуулалтыг баталгаажуулахын тулд тестээр баталгаажсан регрессийг ажиллуул
багцын хэсэг нь синхрончлолд байх болно:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Манифест байгуулж, гарын үсэг зурах

Төлөвлөгөө, нэр, засаглалын гарын үсгийг ашиглан манифест болгон боож өгнө
`sorafs-manifest-stub`. Доорх тушаал нь нэг файлын ачааллыг харуулж байна; нэвтрүүлэх
модыг багцлах лавлах зам (CLI үүнийг үг зүйгээр явуулдаг).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json`-г хянан үзэх:

- `chunking.chunk_digest_sha3_256` – офсет/уртыг харуулсан SHA3 тоймтой таарч байна
  chunker бэхэлгээ.
- `manifest.manifest_blake3` – Манифестийн дугтуйнд гарын үсэг зурсан BLAKE3 дижест.
- `chunk_fetch_specs[]` – найрал хөгжимчдийн захиалсан дуудах заавар.

Бодит гарын үсгийг нийлүүлэхэд бэлэн болмогц `--signing-key` болон `--signer` нэмнэ үү.
аргументууд. Тушаал нь бичихээсээ өмнө Ed25519 гарын үсэг бүрийг шалгадаг
дугтуй.

## 4. Олон үйлчилгээ үзүүлэгчийн хайлтыг дуурайлгана

Нэг буюу түүнээс дээш тооны багц төлөвлөгөөг дахин тоглуулахын тулд хөгжүүлэгчийн CLI-г ашиглана уу
үйлчилгээ үзүүлэгчид. Энэ нь CI утааны туршилт болон найруулагчийн прототип хийхэд тохиромжтой.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Баталгаажуулалт:

- `payload_digest_hex` манифест тайлантай тохирч байх ёстой.
- `provider_reports[]` нь үйлчилгээ үзүүлэгч бүрийн амжилт/бүтэлгүйтлийн тоог харуулдаг.
- Тэг биш `chunk_retry_total` нь буцах даралтын тохируулгыг онцолж өгдөг.
- Ажиллуулахаар төлөвлөж буй үйлчилгээ үзүүлэгчийн тоог хязгаарлахын тулд `--max-peers=<n>`-г дамжуулна уу
  мөн CI симуляцийг үндсэн нэр дэвшигчид анхаарлаа хандуулаарай.
- `--retry-budget=<n>` хэсэг болгон дахин оролдох өгөгдмөл тоог (3) хүчингүй болгодог тул та
  бүтэлгүйтлийг шахах үед найруулагчийн регрессийг илүү хурдан гаргаж чадна.

Амжилтгүй болтол `--expect-payload-digest=<hex>` болон `--expect-payload-len=<bytes>` нэмэх
сэргээн босгосон ачаалал манифестээс хазайсан үед хурдан.

## 5. Дараагийн алхамууд

- **Засаглалын интеграцчилал** – манифест тойм болон
  `manifest_signatures.json` зөвлөлийн ажлын урсгалд оруулснаар Pin бүртгэлийн газар боломжтой
  бэлэн байдлыг сурталчлах.
- **Бүртгэлийн хэлэлцээр** – зөвлөлдөх [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  шинэ профайл бүртгүүлэхээс өмнө. Автоматжуулалт нь каноник бариулыг илүүд үзэх ёстой
  (`namespace.name@semver`) тоон ID дээр.
- **CI автоматжуулалт** – дамжуулах шугамыг гаргахын тулд дээрх тушаалуудыг нэмж, баримт бичиг,
  бэхэлгээ, олдворууд нь гарын үсэгтэй хамт детерминист манифестийг нийтэлдэг
  мета өгөгдөл.