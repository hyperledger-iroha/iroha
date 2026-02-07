---
lang: uz
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

# SoraFS CLI manifestining oxirigacha namunasi

Ushbu misol yordamida SoraFS ga o'rnatilgan hujjatlarni nashr etish orqali o'tadi.
`sorafs_manifest_stub` CLI deterministik bo'linish moslamalari bilan birga
SoraFS Arxitektura RFC da tasvirlangan. Oqim manifest avlodni qamrab oladi,
kutish tekshiruvlari, olib kelish rejasini tekshirish va izlanishni isbotlash mashqlari
jamoalar bir xil qadamlarni CIga kiritishlari mumkin.

## Old shartlar

- Ish maydoni klonlangan va asboblar zanjiri tayyor (`cargo`, `rustc`).
- `fixtures/sorafs_chunker` moslamalari mavjud, shuning uchun kutilgan qiymatlar bo'lishi mumkin
  olingan (ishlab chiqarish ishlari uchun, migratsiya kitobi yozuvidan qiymatlarni oling
  artefakt bilan bog'liq).
- Nashr qilish uchun namunaviy yuk katalogi (bu misolda `docs/book` ishlatiladi).

## 1-qadam - Manifest, CAR, imzolar va olish rejasini yarating

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

Buyruq:

- `ChunkProfile::DEFAULT` orqali foydali yukni uzatadi.
- CARv2 arxivini va chunk-fatch rejasini chiqaradi.
- `ManifestV1` yozuvini yaratadi, manifest imzolarni tasdiqlaydi (agar mavjud bo'lsa) va
  konvertni yozadi.
- Agar baytlar o'zgarib ketsa, ishga tushirish muvaffaqiyatsiz bo'lishi uchun kutish bayroqlarini qo'llaydi.

## 2-qadam - Chiqarishlarni parchalar do'koni + PoR repetisiyasi bilan tekshiring

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

Bu deterministik chunk do'koni orqali CAR-ni takrorlaydi, hosil qiladi
Qayta olish mumkinligini isbotlovchi namuna olish daraxti va unga mos keladigan manifest hisobotini chiqaradi
boshqaruvni tekshirish.

## 3-qadam - Ko'p provayderni qidirishni taqlid qiling

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI muhitlari uchun har bir provayder uchun alohida foydali yuk yoʻllarini taqdim eting (masalan, oʻrnatilgan
armatura) mashqlar oralig'ini rejalashtirish va nosozliklarni bartaraf etish uchun.

## 4-qadam - Buxgalteriya daftaridagi yozuvni yozib oling

Nashrni `docs/source/sorafs/migration_ledger.md` da yozib oling, yozib oling:

- Manifest CID, CAR dayjesti va kengash imzosi xesh.
- Holat (`Draft`, `Staging`, `Pinned`).
- CI yugurishlari yoki boshqaruv chiptalariga havolalar.

## 5-qadam - Boshqaruv vositalari orqali pin qilish (ro'yxatga olish kitobi jonli bo'lganda)

Pin registri o'rnatilgandan so'ng (migratsiya yo'l xaritasida Milestone M2),
manifestni CLI orqali yuboring:

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

Taklif identifikatori va keyingi tasdiqlash tranzaksiya xeshlari bo'lishi kerak
tekshirilishi mumkinligi uchun migratsiya kitobi yozuvida qayd etilgan.

## Tozalash

`target/sorafs/` ostidagi artefaktlar arxivlanishi yoki bosqichma-bosqich tugunlarga yuklanishi mumkin.
Manifest, imzolar, CAR va olib kelish rejasini shunday quyida birga saqlang
operatorlar va SDK guruhlari joylashtirishni aniq tasdiqlashi mumkin.