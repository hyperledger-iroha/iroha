---
id: chunker-conformance
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

Ushbu qo'llanma har bir dastur qolishi uchun bajarilishi kerak bo'lgan talablarni kodlaydi
SoraFS deterministik chunker profili (SF1) bilan moslangan. Bundan tashqari
regeneratsiya ish jarayonini, imzolash siyosatini va tekshirish bosqichlarini hujjatlashtiradi
SDKlar bo'ylab armatura iste'molchilari sinxronlashtiriladi.

## Kanonik profil

- Profil tutqichi: `sorafs.sf1@1.0.0`
- Kirish urug'i (hex): `0000000000dec0ded`
- Maqsad hajmi: 262144 bayt (256KiB)
- Minimal hajmi: 65536 bayt (64KiB)
- Maksimal hajmi: 524288 bayt (512KiB)
- Rolling polinomi: `0x3DA3358B4DC173`
- Tishli stol urug'i: `sorafs-v1-gear`
- Tanaffus niqobi: `0x0000FFFF`

Malumotni amalga oshirish: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Har qanday SIMD tezlashuvi bir xil chegaralarni va hazm qilishni keltirib chiqarishi kerak.

## Armatura to'plami

`cargo run --locked -p sorafs_chunker --bin export_vectors` ni qayta tiklaydi
armatura o'rnatadi va `fixtures/sorafs_chunker/` ostida quyidagi fayllarni chiqaradi:

- `sf1_profile_v1.{json,rs,ts,go}` - Rust uchun kanonik bo'lak chegaralari,
  TypeScript va Go iste'molchilari. Har bir fayl kanonik tutqichni e'lon qiladi
  `profile_aliases` da birinchi (va yagona) kirish. Buyurtma tomonidan amalga oshiriladi
  `ensure_charter_compliance` va o'zgartirilmasligi kerak.
- `manifest_blake3.json` - har bir armatura faylini qamrab olgan BLAKE3 tomonidan tasdiqlangan manifest.
- `manifest_signatures.json` - manifest ustidan Kengash imzolari (Ed25519)
  hazm qilish.
- `sf1_profile_v1_backpressure.json` va `fuzz/` ichidagi xom korpus -
  chunker orqa bosim sinovlari tomonidan ishlatiladigan deterministik oqim stsenariylari.

### Imzolash siyosati

Armatura regeneratsiyasi **kerak** amaldagi kengash imzosini o'z ichiga olishi kerak. Generator
`--allow-unsigned` aniq o'tkazilmasa (mo'ljallangan) imzosiz chiqishni rad etadi
faqat mahalliy tajriba uchun). Imzo konvertlari faqat ilova qilinadi va
har bir imzolovchi uchun nusxalangan.

Kengash imzosini qo'shish uchun:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Tasdiqlash

`ci/check_sorafs_fixtures.sh` CI yordamchisi generatorni qayta o'ynaydi
`--locked`. Agar armatura siljishi yoki imzolar etishmayotgan bo'lsa, ish bajarilmaydi. Foydalanish
bu skriptni tungi ish oqimlarida va armatura o'zgarishlarini yuborishdan oldin.

Qo'lda tekshirish bosqichlari:

1. `cargo test -p sorafs_chunker` ni ishga tushiring.
2. `ci/check_sorafs_fixtures.sh` ni mahalliy sifatida chaqiring.
3. `git status -- fixtures/sorafs_chunker` toza ekanligini tasdiqlang.

## O'yin kitobini yangilang

Yangi chunker profilini taklif qilish yoki SF1ni yangilashda:

Shuningdek qarang: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) uchun
metadata talablari, taklif shablonlari va tekshirish roʻyxatlari.

1. Yangi parametrlar bilan `ChunkProfileUpgradeProposalV1` loyihasini tuzing (qarang: RFC SF‑1).
2. `export_vectors` orqali armaturalarni qayta tiklang va yangi manifest dayjestini yozib oling.
3. Kerakli kengash kvorum bilan manifestni imzolang. Barcha imzolar bo'lishi kerak
   `manifest_signatures.json` ga ilova qilingan.
4. Ta'sir qilingan SDK moslamalarini yangilang (Rust/Go/TS) va o'zaro ish vaqti paritetini ta'minlang.
5. Parametrlar o'zgarsa, fuzz korpusini qayta tiklang.
6. Ushbu qo'llanmani yangi profil tutqichi, urug'lar va hazm qilish bilan yangilang.
7. O'zgartirishni yangilangan testlar va yo'l xaritasi yangilanishlari bilan birga yuboring.

Ushbu jarayonga rioya qilmasdan bo'lak chegaralariga yoki hazm qilishga ta'sir qiluvchi o'zgarishlar
yaroqsiz va birlashtirilmasligi kerak.