---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b84ca0af25021c9ef883fb207c2af7226569cb8e750e05edbb15384474f86d0
source_last_modified: "2026-01-05T09:28:11.858804+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry-charter
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

# SoraFS Chunker reestrini boshqarish xartiyasi

> **Ratifikatsiya qilingan:** 2025-10-29 Sora parlamenti infratuzilma kengashi tomonidan (qarang.
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Har qanday tuzatishlar a talab qiladi
> rasmiy boshqaruv bo'yicha ovoz berish; Amalga oshirish guruhlari ushbu hujjatga shunday munosabatda bo'lishlari kerak
> o'rnini bosuvchi nizom tasdiqlanmaguncha normativ.

Ushbu nizom SoraFS chunkerini rivojlantirish jarayoni va rollarini belgilaydi.
ro'yxatga olish kitobi. U [Chunker profilining mualliflik boʻyicha qoʻllanmasini](./chunker-profile-authoring.md) qanday yangi ekanligini tasvirlab toʻldiradi.

## Qo'llash doirasi

Nizom `sorafs_manifest::chunker_registry` va har bir yozuv uchun amal qiladi
registrni iste'mol qiladigan har qanday vositaga (manifest CLI, provayder-reklama CLI,
SDK). U taxallusni va tekshirilgan invariantlarni ishlatadi
`chunker_registry::ensure_charter_compliance()`:

- Profil identifikatorlari monoton ravishda ortib boruvchi musbat butun sonlardir.
- `namespace.name@semver` kanonik tutqichi birinchi bo'lib paydo bo'lishi **kerak**
- Taxallus satrlari kesilgan, noyob va kanonik tutqichlar bilan to'qnashmaydi
  boshqa yozuvlar.

## Rollar

- **Muallif(lar)** – taklifni tayyorlash, jihozlarni qayta tiklash va yig'ish
  determinizm dalilidir.
- **Tooling Working Group (TWG)** – eʼlon qilinganlardan foydalangan holda taklifni tasdiqlaydi
  nazorat ro'yxatlarini tuzadi va registrning o'zgarmasligini ta'minlaydi.
- **Boshqaruv kengashi (GC)** – TWG hisobotini ko'rib chiqadi, taklifni imzolaydi
  konvertga soladi va nashr/eskirish muddatlarini tasdiqlaydi.
- **Storage Team** – registrni amalga oshirish va nashr etishni amalga oshiradi
  hujjatlar yangilanishi.

## Hayot aylanish jarayoni

1. **Taklifni yuborish**
   - Muallif mualliflik qoʻllanmasidan tekshirish roʻyxatini ishga tushiradi va yaratadi
     ostida `ChunkerProfileProposalV1` JSON
     `docs/source/sorafs/proposals/`.
   - Quyidagidan CLI chiqishini qo'shing:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Armatura, taklif, determinizm hisoboti va reestrni o'z ichiga olgan PRni yuboring
     yangilanishlar.

2. **Asboblarni ko'rib chiqish (TWG)**
   - Tekshiruv roʻyxatini takrorlang (fiksatorlar, fuzz, manifest/PoR quvur liniyasi).
   - `cargo test -p sorafs_car --chunker-registry` ni ishga tushiring va ishonch hosil qiling
     `ensure_charter_compliance()` yangi yozuv bilan o'tadi.
   - CLI harakatini tekshiring (`--list-profiles`, `--promote-profile`, oqim
     `--json-out=-`) yangilangan taxalluslar va tutqichlarni aks ettiradi.
   - Topilmalar va o'tish/qobiliyatsiz holatini umumlashtiruvchi qisqa hisobot tayyorlang.

3. **Kengash roziligi (KK)**
   - TWG hisoboti va taklif metama'lumotlarini ko'rib chiqing.
   - Taklif dayjestini imzolang (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     Kengash konvertiga imzo qo'shing va imzo qo'shing
     armatura.
   - ovoz berish natijalarini boshqaruv bayonnomasiga yozib qo'ying.

4. **Nashr**
   - PRni birlashtirish, yangilash:
     - `sorafs_manifest::chunker_registry_data`.
     - Hujjatlar (`chunker_registry.md`, mualliflik/muvofiqlik qo'llanmalari).
     - Fiksturlar va determinizm hisobotlari.
   - Operatorlar va SDK guruhlarini yangi profil va rejalashtirilgan ishga tushirish haqida xabardor qiling.

5. **Depretsiya / Quyosh botishi**
   - Mavjud profil o'rnini bosadigan takliflar ikki marta nashr qilishni o'z ichiga olishi kerak
     oyna (imtiyozli davrlar) va yangilash rejasi.
     ro'yxatga olish kitobida va migratsiya kitobini yangilang.

6. **Favqulodda oʻzgarishlar**
   - O'chirish yoki tuzatishlar ko'pchilikning roziligi bilan kengash ovozini talab qiladi.
   - TWG xavfni kamaytirish bosqichlarini hujjatlashtirishi va hodisalar jurnalini yangilashi kerak.

## Asboblarni kutish

- `sorafs_manifest_chunk_store` va `sorafs_manifest_stub`:
  - Ro'yxatga olish kitobini tekshirish uchun `--list-profiles`.
  - ishlatiladigan kanonik metama'lumotlar blokini yaratish uchun `--promote-profile=<handle>`
    profilni targ'ib qilishda.
  - `--json-out=-` hisobotlarni stdout-ga o'tkazish, takroriy ko'rib chiqish imkonini beradi
    jurnallar.
- `ensure_charter_compliance()` tegishli ikkilik fayllarda ishga tushirilganda chaqiriladi
  (`manifest_chunk_store`, `provider_advert_stub`). Agar yangi bo'lsa, CI testlari muvaffaqiyatsiz bo'lishi kerak
  yozuvlar nizomni buzadi.

## Yozuvni yuritish

- Barcha determinizm hisobotlarini `docs/source/sorafs/reports/` da saqlang.
- Kengash bayonnomalari chunker qarorlariga havola qilinadi
  `docs/source/sorafs/migration_ledger.md`.
- Har bir registr o'zgarishidan keyin `roadmap.md` va `status.md` ni yangilang.

## Ma'lumotnomalar

- Mualliflik qoʻllanmasi: [Chunker profili mualliflik qoʻllanmasi](./chunker-profile-authoring.md)
- Muvofiqlikni tekshirish ro'yxati: `docs/source/sorafs/chunker_conformance.md`
- Ro'yxatga olish kitobi ma'lumotnomasi: [Chunker Profile Registry](./chunker-registry.md)