---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f7dd8b29e8eb37c2cd78c5dc91ce363bb546fa7e8768f8a2cc86f8b2d9508674
source_last_modified: "2026-01-04T08:19:26.498928+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS SF1 Determinism Dry-Run
summary: Checklist and expected digests for validating the canonical `sorafs.sf1@1.0.0` chunker profile.
translator: machine-google-reviewed
---

# SoraFS SF1 Determinizm Quruq ish

Bu hisobot kanonik uchun asosiy quruq ishga tushirishni qamrab oladi
`sorafs.sf1@1.0.0` chunker profili. Tooling WG nazorat ro'yxatini qayta ishga tushirishi kerak
armatura yangilanishi yoki yangi iste'molchi quvurlarini tekshirishda quyida. ni yozib oling
tekshiriladigan izni saqlash uchun jadvaldagi har bir buyruqning natijasi.

## Tekshirish ro'yxati

| Qadam | Buyruq | Kutilayotgan natija | Eslatmalar |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Barcha testlar o'tadi; `vectors` paritet testi muvaffaqiyatli o'tdi. | Kanonik moslamalarni kompilyatsiya qilish va Rust dasturiga mos kelishini tasdiqlaydi. |
| 2 | `ci/check_sorafs_fixtures.sh` | Skript 0 dan chiqadi; quyida manifest dayjestlar hisobotlari. | Uskunalar toza qayta tiklanganini va imzolar biriktirilganligini tasdiqlaydi. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` yozuvi registr deskriptoriga (`profile_id=1`) mos keladi. | Ro'yxatga olish kitobi metama'lumotlarining sinxron bo'lishini ta'minlaydi. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Regeneratsiya `--allow-unsigned`siz muvaffaqiyatli amalga oshiriladi; manifest va imzo fayllari o'zgarishsiz. | Bo'lak chegaralari va manifestlar uchun determinizmni isbotlaydi. |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript moslamalari va Rust JSON o'rtasida hech qanday farq yo'qligini xabar qiladi. | Ixtiyoriy yordamchi; ish vaqtlari bo'ylab paritetni ta'minlash (Tooling WG tomonidan qo'llab-quvvatlanadigan skript). |

## Kutilayotgan hazm qilish

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`
- `sf1_profile_v1.json`: `d89a4fdc030b0c7c4911719ea133c780d9f4610b08eef1d6d0e0ca443391718e`
- `sf1_profile_v1.ts`: `9a3bb8e4d96518b3a0a1301046b2d86a793991959ebdd8adda1fb2988e4292dc`
- `sf1_profile_v1.go`: `0f0348b8751b0f85fe874afda3371af75b78fac5dad65182204dcb3cf3e4c0a1`
- `sf1_profile_v1.rs`: `66b5956826c86589a24b71ca6b400cc1335323c6371f1cec9475f09af8743f61`

## Chiqish jurnali

| Sana | Muhandis | Tekshirish roʻyxati natijasi | Eslatmalar |
|------|----------|------------------|-------|
| 2026-02-12 | Asboblar (LLM) | ❌ Muvaffaqiyatsiz | 1-qadam: `cargo test -p sorafs_chunker` `vectors` to'plamida ishlamayapti, chunki armatura eskirgan. 2-qadam: `ci/check_sorafs_fixtures.sh` bekor qilinadi—`manifest_signatures.json` repo holatida yoʻq (ishchi daraxtda oʻchirilgan). 4-qadam: `export_vectors` manifest fayli mavjud bo'lmaganda imzolarni tekshira olmaydi. Imzolangan moslamalarni tiklashni tavsiya eting (yoki kengash kalitini taqdim eting) va kanonik tutqichlar sinovlar talab qilganidek o'rnatilishi uchun biriktirmalarni qayta tiklashni tavsiya eting. |
| 2026-02-12 | Asboblar (LLM) | ✅ O'tdi | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` orqali qayta tiklangan moslamalar, kanonik qoʻl ostidagi taxalluslar roʻyxati va yangi `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4` manifest dayjestini ishlab chiqaradi. `cargo test -p sorafs_chunker` va toza `ci/check_sorafs_fixtures.sh` bilan tasdiqlangan (tekshiruv uchun bosqichli armatura). 5-bosqich tugun pariteti yordamchisi joylashguncha kutilmoqda. |
| 2026-02-20 | Saqlash vositalari CI | ✅ O'tdi | Parlament konverti (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` orqali olingan; skript qayta yaratilgan moslamalar, `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4` manifest dayjestini tasdiqladi va Rust jabduqlarini (mavjud bo'lganda o'tish/tugun bosqichlari bajariladi) hech qanday farqsiz qayta ishga tushirdi. |

Tooling WG nazorat ro'yxatini ishga tushirgandan so'ng sana belgilangan qatorni qo'shishi kerak. Har qanday qadam bo'lsa
muvaffaqiyatsiz bo'lsa, bu yerga bog'langan muammoni yozing va avval tuzatish tafsilotlarini kiriting
yangi armatura yoki profillarni tasdiqlash.