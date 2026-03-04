---
lang: mn
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

# SoraFS Manifest CLI Төгсгөл хүртэлх жишээ

Энэ жишээг ашиглан SoraFS хүртэлх баримт бичгийг нийтлэх үйл явцыг харуулсан.
`sorafs_manifest_stub` CLI нь тодорхойлогч бэхэлгээний хамт
SoraFS Architecture RFC-д тайлбарласан. Урсгал нь илэрхий үеийг хамардаг,
хүлээлтийг шалгах, авчрах төлөвлөгөөний баталгаажуулалт, нотлох баримтын давтлага гэх мэт
багууд ижил алхмуудыг CI-д оруулах боломжтой.

## Урьдчилсан нөхцөл

- Ажлын талбарыг клонжуулж, багажны гинж бэлэн болсон (`cargo`, `rustc`).
- `fixtures/sorafs_chunker`-ийн бэхэлгээ байгаа тул хүлээгдэж буй утгууд байж болно
  үүсэлтэй (үйлдвэрлэлийн ажлын хувьд шилжилт хөдөлгөөний дэвтэрийн бичилтээс утгуудыг татаж авна уу
  олдвортой холбоотой).
- Нийтлэх ачааллын лавлах жишээ (энэ жишээнд `docs/book` ашигладаг).

## Алхам 1 - Манифест, CAR, гарын үсэг үүсгэх, авах төлөвлөгөө

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

Тушаал:

- Ачааны ачааллыг `ChunkProfile::DEFAULT`-ээр дамжуулдаг.
- CARv2 архив болон бөөгнөрөл татах төлөвлөгөө гаргадаг.
- `ManifestV1` бичлэгийг үүсгэж, ил тод гарын үсгийг (хэрэв өгсөн бол) баталгаажуулдаг.
  дугтуйг бичдэг.
- Хүлээгдэж буй тугуудыг мөрддөг тул байт дамжих тохиолдолд гүйлт амжилтгүй болно.

## Алхам 2 — Гаралтыг бөөгнөрөл + PoR давтлага ашиглан баталгаажуулна уу

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

Энэ нь детерминист бөөгнөрөл дэлгүүрээр дамжуулан CAR-г дахин тоглуулж, гаргаж авдаг
Дээж авах боломжтой нотлох мод, тохиромжтой манифест тайлан гаргадаг
засаглалын тойм.

## Алхам 3 — Олон үйлчилгээ үзүүлэгчийн хайлтыг дуурай

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI орчны хувьд үйлчилгээ үзүүлэгч тус бүрд тус тусад нь ачааллын замыг өгнө үү (жишээ нь, холбогдсон
бэхэлгээ) дасгалын хүрээний хуваарь гаргах, бүтэлгүйтлийг зохицуулах.

## Алхам 4 — Бүртгэлийн дэвтэрт бичилт хийх

Нийтлэлийг `docs/source/sorafs/migration_ledger.md`-д бүртгэж, дараахыг бичнэ:

- Манифест CID, CAR дижест, зөвлөлийн гарын үсгийн хэш.
- Статус (`Draft`, `Staging`, `Pinned`).
- CI гүйлтүүд эсвэл засаглалын тасалбаруудын холбоосууд.

## Алхам 5 — Засаглалын хэрэглүүрээр бэхлэх (бүртгэл ажиллаж байх үед)

Бүртгэлийн бүртгэлийг байрлуулсны дараа (шилжилтийн замын зураг дээрх чухал үе M2),
CLI-ээр дамжуулан манифест илгээх:

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

Саналын танигч болон дараагийн зөвшөөрлийн гүйлгээний хэш нь байх ёстой
аудит хийх боломжтой болгох үүднээс шилжилт хөдөлгөөний дэвтэрт оруулсан.

## Цэвэрлэгээ

`target/sorafs/` доорх олдворуудыг архивлах эсвэл үе шатны зангилаа руу байршуулах боломжтой.
Манифест, гарын үсэг, АВТОМАШИН, татан авалтын төлөвлөгөөг нэг дор байлга
операторууд болон SDK багууд байршуулалтыг тодорхойлон баталгаажуулж чадна.