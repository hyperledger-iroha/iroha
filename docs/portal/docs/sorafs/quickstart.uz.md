---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.908615+00:00"
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

Regenerate the canonical SF-1 chunking vectors. The command verifies the
existing council signature file, or appends a signature when `--signing-key` is
supplied.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --signing-key=<ed25519-private-key-hex>
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
`sorafs_manifest_builder`. Quyidagi buyruq bitta faylli foydali yukni ko'rsatadi; o'tish
daraxtni qadoqlash uchun katalog yo'li (CLI uni leksikografik jihatdan boshqaradi).

```bash
cargo run -p sorafs_car --bin sorafs_manifest_builder -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --council-signature=<signerhex>:<signaturehex>
```

`/tmp/docs.report.json`ni ko'rib chiqing:

- `chunking.chunk_digest_sha3_256` – ofset/uzunliklarning SHA3 dayjestiga mos keladi
  chunker armatura.
- `manifest.manifest_blake3` – manifest konvertda imzolangan BLAKE3 dayjesti.
- `chunk_fetch_specs[]` - orkestrlar uchun buyurtma qilingan yuklash ko'rsatmalari.

The `--council-signature` value must be a reviewed council signer public key and
Ed25519 signature pair. The command verifies every Ed25519 signature before
writing the envelope.

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