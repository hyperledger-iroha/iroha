---
id: chunker-profile-authoring
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Profile Authoring Guide
sidebar_label: Chunker Authoring Guide
description: Checklist for proposing new SoraFS chunker profiles and fixtures.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

# SoraFS Chunker profili mualliflik bo'yicha qo'llanma

Ushbu qo'llanma SoraFS uchun yangi chunker profillarini qanday taklif qilish va nashr etishni tushuntiradi.
U RFC arxitekturasini (SF-1) va ro'yxatga olish kitobi ma'lumotnomasini (SF-2a) to'ldiradi.
aniq mualliflik talablari, tasdiqlash bosqichlari va taklif shablonlari bilan.
Kanonik misol uchun qarang
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
va unga hamroh bo'lgan quruq ishga kirish
`docs/source/sorafs/reports/sf1_determinism.md`.

## Umumiy ko'rinish

Ro'yxatga olish kitobiga kirgan har bir profil:

- deterministik CDC parametrlarini va bir xil multihash sozlamalarini reklama qiling
  arxitektura;
- qayta o'ynaladigan kemalar (Rust/Go/TS JSON + fuzz corpora + PoR guvohlari)
  quyi oqim SDKlar buyurtma asboblarisiz tekshirishi mumkin;
- boshqaruvga tayyor metadata (nom maydoni, nom, semver) va migratsiyani o'z ichiga oladi
- kengash ko'rib chiqishdan oldin deterministik farqlar to'plamidan o'tish.

Ushbu qoidalarga javob beradigan taklifni tayyorlash uchun quyidagi nazorat roʻyxatiga amal qiling.

## Registry Nizomining surati

Taklif loyihasini tayyorlashdan oldin uning ro'yxatga olish kitobi nizomiga muvofiqligini tasdiqlang
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` tomonidan:

- Profil identifikatorlari bo'shliqlarsiz monoton ravishda ortib boruvchi musbat butun sonlardir.
- Kanonik tutqich (`namespace.name@semver`) taxalluslar ro'yxatida paydo bo'lishi kerak
  va **kerak** birinchi yozuv bo'lishi kerak.
- Hech qanday taxallus boshqa kanonik tutqich bilan to'qnashmasligi yoki bir necha marta paydo bo'lishi mumkin emas.
- Taxalluslar bo'sh bo'lmasligi va bo'sh joy bilan kesilgan bo'lishi kerak.

Foydali CLI yordamchilari:

```bash
# JSON listing of all registered descriptors (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emit metadata for a candidate default profile (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Ushbu buyruqlar takliflarni ro'yxatga olish kitobi ustaviga muvofiqlashtiradi va taqdim etadi
boshqaruv muhokamalarida zarur bo'lgan kanonik metama'lumotlar.

## Kerakli metamaʼlumotlar

| Maydon | Tavsif | Misol (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Tegishli profillar uchun mantiqiy guruhlash. | `sorafs` |
| `name` | Odam o‘qiy oladigan yorliq. | `sf1` |
| `semver` | Parametrlar to'plami uchun semantik versiya qatori. | `1.0.0` |
| `profile_id` | Profil tushganda tayinlangan monoton raqamli identifikator. Keyingi identifikatorni zaxiralang, lekin mavjud raqamlarni qayta ishlatmang. | `1` |
| `profile_aliases` | Ixtiyoriy qo'shimcha tutqichlar muzokaralar paytida mijozlarga ta'sir qiladi. Har doim kanonik tutqichni birinchi yozuv sifatida kiriting. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Baytlardagi minimal parcha uzunligi. | `65536` |
| `profile.target_size` | Maqsadli qism uzunligi baytlarda. | `262144` |
| `profile.max_size` | Baytlardagi maksimal bo'lak uzunligi. | `524288` |
| `profile.break_mask` | Rolling hash (hex) tomonidan ishlatiladigan moslashtiruvchi niqob. | `0x0000ffff` |
| `profile.polynomial` | Tishli polinom doimiysi (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Urug' 64KiB tishli stolni olish uchun ishlatiladi. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Har bir parcha hazm qilish uchun multihash kodi. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Kanonik armatura to'plamining to'plami. | `13fa...c482` |
| `fixtures_root` | Qayta tiklangan moslamalarni o'z ichiga olgan nisbiy katalog. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Deterministik PoR namunalarini olish uchun urug' (`splitmix64`). | `0xfeedbeefcafebabe` (misol) |

Metadata ham taklif hujjatida, ham yaratilgan hujjat ichida ko'rinishi kerak
ro'yxatga olish kitobi, CLI asboblari va boshqaruvni avtomatlashtirish tasdiqlashi mumkin
qo'lda o'zaro havolalarsiz qiymatlar. Shubha tug'ilganda, chunk-do'konini ishga tushiring va
Hisoblangan metama'lumotlarni ko'rib chiqish uchun oqimlash uchun `--json-out=-` bilan manifest CLI'lar
eslatmalar.

### CLI & Registry aloqa nuqtalari

- `sorafs_manifest_chunk_store --profile=<handle>` - parcha metama'lumotlarini qayta ishga tushirish,
  manifest digest, PoR taklif qilingan parametrlar bilan tekshiradi.
- `sorafs_manifest_chunk_store --json-out=-` - chunk-do'kon hisobotini oqimlash
  Avtomatlashtirilgan taqqoslash uchun stdout.
- `sorafs_manifest_stub --chunker-profile=<handle>` - manifest va CARni tasdiqlang
  rejalar kanonik tutqichni va taxalluslarni joylashtiradi.
- `sorafs_manifest_stub --plan=-` - oldingi `chunk_fetch_specs` ni orqaga qaytaring
  o'zgarishlardan keyingi ofsetlarni/dijestlarni tekshirish uchun.

Taklifda buyruq chiqishini (dijestlar, PoR ildizlari, manifest xeshlari) yozib oling
shuning uchun sharhlovchilar ularni so'zma-so'z takrorlashlari mumkin.

## Aniqlash va tekshirish ro'yxati

1. **Asboblarni qayta tiklash**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Paritet to‘plamini ishga tushiring** – `cargo test -p sorafs_chunker` va
   tillararo farqlash moslamasi (`crates/sorafs_chunker/tests/vectors.rs`) bo'lishi kerak
   yangi jihozlar bilan yashil rangda.
3. **Fuzz/back-pressure corpora**-ni takrorlang – `cargo fuzz list` va
   qayta tiklangan aktivlarga qarshi oqim jabduqlari (`fuzz/sorafs_chunker`).
4. **Isbot-of-qayta olish guvohlarini tekshirish** – ishga tushirish
   `sorafs_manifest_chunk_store --por-sample=<n>` tavsiya etilgan profil yordamida va
   ildizlarning armatura manifestiga mos kelishini tasdiqlang.
5. **CI quruq ish** – mahalliy sifatida `ci/check_sorafs_fixtures.sh` ni chaqirish; skript
   yangi armatura va mavjud `manifest_signatures.json` bilan muvaffaqiyat qozonishi kerak.
6. **Oʻzaro ishlash vaqtini tasdiqlash** – Go/TS ulanishlari qayta tiklangan quvvatni isteʼmol qilishiga ishonch hosil qiling
   JSON va bir xil bo'lak chegaralari va digestlarni chiqaradi.

Taklifdagi buyruqlar va natijada olingan dayjestlarni hujjatlashtiring, shuning uchun Tooling WG
ularni taxmin qilmasdan qayta ishga tushirishi mumkin.

### Manifest / PoR tasdiqlash

Armatura qayta tiklangandan so'ng, CARni ta'minlash uchun to'liq manifest quvur liniyasini ishga tushiring
metadata va PoR dalillari izchil bo'lib qoladi:

```bash
# Validate chunk metadata + PoR with the new profile
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generate manifest + CAR and capture chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-run using the saved fetch plan (guards against stale offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Kirish faylini armaturangiz tomonidan ishlatiladigan har qanday vakili korpus bilan almashtiring
(masalan, 1GiB deterministik oqim) va natijada olingan digestlarni biriktiring
taklif.

## Taklif shabloni

Takliflar `ChunkerProfileProposalV1` Norito yozuvlari sifatida taqdim etiladi
`docs/source/sorafs/proposals/`. Quyidagi JSON shablonida kutilgan narsa tasvirlangan
shakl (kerak bo'lganda qiymatlaringizni almashtiring):


Tegishli Markdown hisobotini taqdim eting (`determinism_report`).
buyruq chiqishi, parcha hazm qilishlari va tekshirish vaqtida duch kelgan har qanday og'ishlar.

## Boshqaruv ish jarayoni

1. **Taklif + moslamalar bilan PR yuboring.** Yaratilgan aktivlarni,
   Norito taklifi va `chunker_registry_data.rs` yangilanishlari.
2. **Tooling WG tekshiruvi.** Tekshiruvchilar tekshirish ro'yxatini qayta ishga tushiradilar va tasdiqlaydilar
   taklif ro'yxatga olish qoidalariga mos keladi (identifikatorni qayta ishlatish yo'q, determinizm qoniqtiriladi).
3. **Kengash konverti.** Tasdiqlanganidan keyin kengash aʼzolari takliflar dayjestiga imzo chekadilar
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) va ularni qo'shing
   armatura bilan birga saqlangan profil konvertiga imzolar.
4. **Ro‘yxatga olish kitobini nashr qilish.** Birlashtirish ro‘yxatga olish kitobi, hujjatlar va moslamalarni to‘ntaradi. The
   Standart CLI boshqaruv e'lon qilmaguncha oldingi profilda qoladi
   migratsiya tayyor.
5. **O'chirishni kuzatish.** Migratsiya oynasidan so'ng registrni yangilang
   daftar.

## Mualliflik bo'yicha maslahatlar

- Kengaytirilgan qismlarni ajratish xatti-harakatlarini minimallashtirish uchun hatto ikkita quvvat chegarasiga ham ustunlik bering.
- Manifest va shlyuzni muvofiqlashtirmasdan multihash kodini o'zgartirishdan saqlaning
- Auditni soddalashtirish uchun tishli stol urug'larini odamlar o'qiy oladigan, ammo global miqyosda noyob bo'lib qoling
  izlar.
- Har qanday taqqoslash artefaktlarini (masalan, o'tkazish qobiliyatini taqqoslash) ostida saqlang
  Kelajakda ma'lumot olish uchun `docs/source/sorafs/reports/`.

Chiqarish paytida operatsion kutilmalar uchun migratsiya kitobiga qarang
(`docs/source/sorafs/migration_ledger.md`). Ish vaqtiga muvofiqlik qoidalari uchun qarang
`docs/source/sorafs/chunker_conformance.md`.