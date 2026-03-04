---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-05T09:28:11.996979+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Chunking → Manifest quvur liniyasi

Tez ishga tushirishning ushbu hamkori xom-ashyoga aylanib ketadigan quvur liniyasini kuzatib boradi
SoraFS Pin registriga mos keladigan Norito manifestiga baytlarni kiriting. Tarkib shunday
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) dan moslashtirilgan;
kanonik spetsifikatsiya va o'zgarishlar jurnali uchun ushbu hujjatga murojaat qiling.

## 1. Deterministik ravishda bo'lak

SoraFS SF-1 (`sorafs.sf1@1.0.0`) profilidan foydalanadi: FastCDC-dan ilhomlangan prokat
64KiB minimal boʻlak hajmi, 256KiB maqsadli, maksimal 512KiB va
`0x0000ffff` sindirish niqobi. Profil ro'yxatdan o'tgan
`sorafs_manifest::chunker_registry`.

### Zangga qarshi yordamchilar

- `sorafs_car::CarBuildPlan::single_file` - bo'laklarni ofsetlarni, uzunliklarni va
  BLAKE3 CAR metamaʼlumotlarini tayyorlashda hazm qiladi.
- `sorafs_car::ChunkStore` - foydali yuklarni uzatadi, parcha metama'lumotlarini saqlaydi va
  64KiB / 4KiB Retrievability Proof-of-Retrievability (PoR) namuna olish daraxtini oladi.
- `sorafs_chunker::chunk_bytes_with_digests` - Ikkala CLI ortidagi kutubxona yordamchisi.

### CLI vositalari

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON buyurtma qilingan ofsetlarni, uzunliklarni va parcha dayjestlarini o'z ichiga oladi. Davom eting
manifestlarni qurishda rejalashtirish yoki orkestrni olib kelish spetsifikatsiyalari.

### PoR guvohlari

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` va
`--por-sample=<count>`, shuning uchun auditorlar deterministik guvohlar to'plamini so'rashlari mumkin. Juftlash
JSONni yozib olish uchun `--por-proof-out` yoki `--por-sample-out` bayroqlari.

## 2. Manifestni o'rash

`ManifestBuilder` boshqaruv qo'shimchalari bilan parcha metama'lumotlarini birlashtiradi:

- Root CID (dag-cbor) va CAR majburiyatlari.
- Taxallus dalillari va provayder qobiliyatiga oid da'volar.
- Kengash imzolari va ixtiyoriy metama'lumotlar (masalan, qurish identifikatorlari).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Muhim natijalar:

- `payload.manifest` – Norito kodlangan manifest baytlari.
- `payload.report.json` - Inson/avtomatlashtirish o'qilishi mumkin bo'lgan xulosa, shu jumladan
  `chunk_fetch_specs`, `payload_digest_hex`, CAR dayjestlari va taxallus metamaʼlumotlari.
- `payload.manifest_signatures.json` - BLAKE3 manifestini o'z ichiga olgan konvert
  dayjest, parcha-reja SHA3 dayjest va tartiblangan Ed25519 imzo.

Tashqi konvertlar tomonidan taqdim etilgan konvertlarni tekshirish uchun `--manifest-signatures-in` dan foydalaning
ularni qayta yozishdan oldin imzolaganlar va `--chunker-profile-id` yoki
Ro'yxatga olish kitobini tanlashni bloklash uchun `--chunker-profile=<handle>`.

## 3. Nashr qiling va mahkamlang

1. **Boshqaruvni taqdim etish** – Manifest dayjest va imzoni taqdim eting
   kengashga konvert, shuning uchun pinni qabul qilish mumkin. Tashqi auditorlar kerak
   manifest dayjest bilan bir qatorda bo'lak rejasi SHA3 dayjestini saqlang.
2. **Foydali yuklarni pin qilish** – havola qilingan CAR arxivini (va ixtiyoriy CAR indeksini) yuklang
   Pin registriga manifestda. Manifest va CARni baham ko'rishiga ishonch hosil qiling
   bir xil ildiz CID.
3. **Telemetriyani yozib oling** – JSON hisoboti, PoR guvohlari va har qanday yuklanishni davom ettirish
   reliz artefaktlaridagi ko'rsatkichlar. Bu yozuvlar operator asboblar panelini va
   katta yuklarni yuklab olmasdan muammolarni qayta ishlab chiqarishga yordam beradi.

## 4. Ko'p provayderni olish simulyatsiyasi

`yuk tashish -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` provayder uchun parallellikni oshiradi (yuqorida `#4`).
- `@<weight>` sozlamalarni rejalashtirish tarafkashligi; sukut bo'yicha 1.
- `--max-peers=<n>` qachon ishga tushirish uchun rejalashtirilgan provayderlar sonini cheklaydi
  kashfiyot kutilganidan ko'ra ko'proq nomzodlarni beradi.
- `--expect-payload-digest` va `--expect-payload-len` jimlikdan himoya qiladi
  korruptsiya.
- `--provider-advert=name=advert.to` oldin provayder imkoniyatlarini tekshiradi
  simulyatsiyada ulardan foydalanish.
- `--retry-budget=<n>` har bir parcha qayta urinishlar sonini bekor qiladi (standart: 3), shuning uchun CI
  muvaffaqiyatsizlik stsenariylarini sinab ko'rishda regressiyalarni tezroq ko'rsatishi mumkin.

`fetch_report.json` jamlangan ko'rsatkichlarni ko'rsatadi (`chunk_retry_total`,
`provider_failure_rate` va boshqalar) CI tasdiqlari va kuzatilishi uchun mos.

## 5. Ro'yxatga olish kitobini yangilash va boshqarish

Yangi chunker profillarini taklif qilishda:

1. `sorafs_manifest::chunker_registry_data` da deskriptor muallifi.
2. `docs/source/sorafs/chunker_registry.md` va tegishli nizomlarni yangilang.
3. Armaturalarni qayta tiklang (`export_vectors`) va imzolangan manifestlarni yozib oling.
4. Ustavga muvofiqlik hisobotini boshqaruv imzolari bilan taqdim eting.

Avtomatlashtirish kanonik tutqichlarni (`namespace.name@semver`) afzal ko'rishi va tushishi kerak