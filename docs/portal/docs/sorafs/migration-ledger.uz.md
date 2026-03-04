---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ae38d9ff7f10a14e63f6d47490dbbe56c9d3b207a30a5899e63414cb726a88f7
source_last_modified: "2026-01-05T09:28:11.880522+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Migration Ledger"
description: "Canonical change log tracking every migration milestone, owners, and required follow-ups."
translator: machine-google-reviewed
---

> [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md) dan moslashtirilgan.

# SoraFS Migratsiya kitobi

Ushbu daftar SoraFS da olingan migratsiya o'zgarishlar jurnalini aks ettiradi.
Arxitektura RFC. Yozuvlar muhim bosqich bo'yicha guruhlangan va samarali ro'yxatga olingan
oyna, ta'sirlangan jamoalar va kerakli harakatlar. Migratsiya rejasiga yangilanishlar
Bu sahifani ham, RFC (`docs/source/sorafs_architecture_rfc.md`) ni ham oʻzgartirishi KERAK
quyi oqim iste'molchilarini moslashtirish uchun.

| Muhim bosqich | Samarali oyna | Xulosa o'zgartirish | Ta'sirli jamoalar | Harakat elementlari | Holati |
|----------|------------------|----------------|----------------|--------------|--------|
| M1 | 7–12 haftalar | CI deterministik moslamalarni qo'llaydi; sahnalashtirishda mavjud taxallus isbotlari; asboblar aniq kutish bayroqlarini ochib beradi. | Hujjatlar, saqlash, boshqaruv | Armatura imzolanganligini ta'minlang, taxalluslarni staging registrida ro'yxatdan o'tkazing, `--car-digest/--root-cid` ijrosi bilan relizlar nazorat ro'yxatini yangilang. | ⏳ Kutilmoqda |

Boshqaruv nazorati samolyotining ushbu bosqichlarga ishora qiluvchi daqiqalari ostida yashaydi
`docs/source/sorafs/`. Jamoalar har bir qator ostida sana ko'rsatilgan nuqtalarni qo'shishlari kerak
muhim voqealar sodir bo'lganda (masalan, yangi taxallus ro'yxatga olish, ro'yxatga olish hodisasi
retrospektivlar) tekshirilishi mumkin bo'lgan qog'oz izini taqdim etish.

## Oxirgi yangilanishlar

- 2025-11-01 - `migration_roadmap.md` boshqaruv kengashiga yuborildi va
  ko'rib chiqish uchun operator ro'yxatlari; Kengashning navbatdagi sessiyasida imzolanishini kutmoqda
  (ref: `docs/source/sorafs/council_minutes_2025-10-29.md` kuzatuvi).
- 2025-11-02 - PIN reestri registrlari ISI endi umumiy chunker/siyosatni qo'llaydi
  `sorafs_manifest` yordamchilari orqali tekshirish, zanjirdagi yo'llarni hizalangan holda saqlash
  Torii tekshiruvlari bilan.
- 2026-02-13 - Buxgalteriya daftariga provayder reklamasini chiqarish bosqichlari (R0–R3) qo'shildi va
  tegishli boshqaruv paneli va operator yo'riqnomasini nashr etdi
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).