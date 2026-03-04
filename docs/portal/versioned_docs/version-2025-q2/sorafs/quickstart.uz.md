---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.997191+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Tez boshlash

Ushbu amaliy qo'llanma deterministik SF-1 chunker profili bo'ylab yuradi,
manifest imzosi va SoraFS ni asoslaydigan ko'p provayderli olib kelish oqimi
saqlash quvur liniyasi. Uni [manifest quvur liniyasiga chuqur sho'ng'ish](manifest-pipeline.md) bilan bog'lang
dizayn eslatmalari va CLI bayrog'i ma'lumotnomasi uchun.

## Old shartlar

- Rust asboblar zanjiri (`rustup update`), ish maydoni mahalliy klonlangan.
- Majburiy emas: [OpenSSL tomonidan yaratilgan Ed25519 klaviatura](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  manifestlarni imzolash uchun.
- Majburiy emas: Node.js ≥ 18, agar siz Docusaurus portalini oldindan ko'rishni rejalashtirmoqchi bo'lsangiz.

Foydali CLI xabarlarini ko'rsatish uchun tajriba o'tkazayotganda `export RUST_LOG=info` ni o'rnating.

## 1. Deterministik moslamalarni yangilang

Kanonik SF-1 bo'linish vektorlarini qayta tiklang. Buyruq shuningdek, imzolangan xabarni chiqaradi
`--signing-key` taqdim etilganda manifest konvertlari; `--allow-unsigned` dan foydalaning
faqat mahalliy rivojlanish davrida.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Chiqishlar:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (agar imzolangan bo'lsa)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Foydali yukni ajratib oling va rejani tekshiring

Ixtiyoriy fayl yoki arxivni qismlarga ajratish uchun `sorafs_chunker` dan foydalaning:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Asosiy maydonlar:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` parametrlarini tasdiqlaydi.
- `chunks[]` – buyurtma qilingan ofsetlar, uzunliklar va BLAKE3 parchalari.

Kattaroq moslamalar uchun oqim va oqimni ta'minlash uchun proptest tomonidan qo'llab-quvvatlanadigan regressiyani ishga tushiring
ommaviy yig'ish sinxronlashtiriladi:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Manifest tuzing va imzolang

Bo'lak rejasi, taxalluslar va boshqaruv imzolarini manifestga o'rash
`sorafs-manifest-stub`. Quyidagi buyruq bitta faylli foydali yukni ko'rsatadi; o'tish
daraxtni qadoqlash uchun katalog yo'li (CLI uni leksikografik jihatdan boshqaradi).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json`ni ko'rib chiqing:

- `chunking.chunk_digest_sha3_256` – ofset/uzunliklarning SHA3 dayjestiga mos keladi
  chunker armatura.
- `manifest.manifest_blake3` – manifest konvertda imzolangan BLAKE3 dayjesti.
- `chunk_fetch_specs[]` - orkestrlar uchun buyurtma qilingan yuklash ko'rsatmalari.

Haqiqiy imzolarni taqdim etishga tayyor bo'lgach, `--signing-key` va `--signer` qo'shing
argumentlar. Buyruq yozishdan oldin har bir Ed25519 imzosini tekshiradi
konvert.

## 4. Ko'p provayderlarni qidirishni taqlid qiling

Bir yoki bir nechta bo'laklar rejasini takrorlash uchun ishlab chiquvchining CLI fetchidan foydalaning
provayderlar. Bu CI tutun sinovlari va orkestr prototipi uchun ideal.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Taʼkidlar:

- `payload_digest_hex` manifest hisobotiga mos kelishi kerak.
- `provider_reports[]` har bir provayderning muvaffaqiyat/qobiliyatsizligini ko'rsatadi.
- Nol bo'lmagan `chunk_retry_total` orqa bosim sozlamalarini ta'kidlaydi.
- Ishga rejalashtirilgan provayderlar sonini cheklash uchun `--max-peers=<n>` dan o'ting
  va CI simulyatsiyalarini asosiy nomzodlarga qaratib turing.
- `--retry-budget=<n>` har bir parcha uchun standart qayta urinishlar sonini (3) bekor qiladi, shuning uchun siz
  nosozliklarni in'ektsiya qilishda orkestr regressiyalarini tezroq yuzaga chiqarishi mumkin.

Muvaffaqiyatsiz bo'lish uchun `--expect-payload-digest=<hex>` va `--expect-payload-len=<bytes>` qo'shing
rekonstruksiya qilingan foydali yuk manifestdan chetga chiqqanda tez.

## 5. Keyingi qadamlar

- **Boshqaruv integratsiyasi** – manifest dayjestini va
  `manifest_signatures.json` kengash ish oqimiga kirishi uchun Pin Registry mumkin
  mavjudligini e'lon qilish.
- **Ro‘yxatga olish bo‘yicha muzokaralar** – maslahatlashing [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  yangi profillarni ro'yxatdan o'tkazishdan oldin. Avtomatlashtirish kanonik tutqichlarni afzal ko'rishi kerak
  (`namespace.name@semver`) raqamli identifikatorlar ustida.
- **CI automation** – hujjatlarni chiqarish uchun yuqoridagi buyruqlarni qo‘shing.
  armatura va artefaktlar imzolanganlar bilan birga deterministik manifestlarni nashr etadi
  metadata.