---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a117889e81f876c00129ade76a9a04aa39181add2378ef5c19110b7be30f9d6f
source_last_modified: "2026-01-05T09:28:11.859335+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry-rollout-checklist
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

# SoraFS Ro'yxatga olish kitobi ro'yxati

Ushbu nazorat ro'yxati yangi chunker profilini targ'ib qilish uchun zarur bo'lgan qadamlarni qamrab oladi yoki
boshqaruvdan so'ng ko'rib chiqishdan ishlab chiqarishgacha provayderni qabul qilish to'plami
nizomi ratifikatsiya qilindi.

> **Qoʻl:** Oʻzgartiruvchi barcha relizlar uchun amal qiladi
> `sorafs_manifest::chunker_registry`, provayder qabul qilish konvertlari yoki
> kanonik armatura to'plamlari (`fixtures/sorafs_chunker/*`).

## 1. Parvoz oldidan tekshirish

1. Armaturalarni qayta tiklang va determinizmni tekshiring:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Determinizm xeshlarini tasdiqlang
   `docs/source/sorafs/reports/sf1_determinism.md` (yoki tegishli profil
   hisobot) qayta tiklangan artefaktlarga mos keladi.
3. `sorafs_manifest::chunker_registry` bilan kompilyatsiya qilinishiga ishonch hosil qiling
   Ishlash orqali `ensure_charter_compliance()`:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Taklif faylini yangilang:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` ostida kengash bayonnomasi
   - Determinizm hisoboti

## 2. Boshqaruvni imzolash

1. Sora uchun Asboblar Ishchi Guruhi hisoboti va takliflar dayjestini taqdim eting
   Parlament infratuzilmasi paneli.
2. Tasdiqlash tafsilotlarini quyidagiga yozing
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Parlament imzolagan konvertni jihozlar bilan birga chop eting:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Konvertga boshqaruvni yuklash yordamchisi orqali kirish mumkinligini tekshiring:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Bosqichli ishlab chiqarish

Buning uchun [staging manifest playbook](./staging-manifest-playbook) ga qarang.
ushbu bosqichlarni batafsil ko'rib chiqish.

1. Torii ni `torii.sorafs` kashfiyoti yoqilgan va qabul qilib oʻrnating
   majburlash yoqildi (`enforce_admission = true`).
2. Tasdiqlangan provayder qabul qilish konvertlarini bosqich registriga suring
   `torii.sorafs.discovery.admission.envelopes_dir` tomonidan havola qilingan katalog.
3. Provayder reklamalari Discovery API orqali tarqalishini tekshiring:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Boshqaruv sarlavhalari bilan manifest/reja yakuniy nuqtalarini mashq qiling:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Telemetriya asboblar panelini tasdiqlang (`torii_sorafs_*`) va ogohlantirish qoidalari
   xatosiz yangi profil.

## 4. Ishlab chiqarishni yo'lga qo'yish

1. Ishlab chiqarish Torii tugunlariga qarshi bosqichma-bosqich bosqichlarni takrorlang.
2. Faollashtirish oynasini (sana/vaqt, imtiyozli davr, qaytarish rejasi) e'lon qiling.
   operator va SDK kanallari.
3. Quyidagilarni o'z ichiga olgan nashr PRni birlashtiring:
   - Yangilangan armatura va konvert
   - Hujjatlarga o'zgartirishlar (nizom ma'lumotlari, determinizm hisoboti)
   - Yo'l xaritasi/holatni yangilash
4. Chiqarishni belgilang va imzolangan artefaktlarni kelib chiqishi uchun arxivlang.

## 5. Chiqarishdan keyingi audit

1. Yakuniy ko‘rsatkichlarni yozib oling (kashfiyotlar soni, olish muvaffaqiyati darajasi, xato
   gistogrammalar) ishga tushirilgandan keyin 24 soat o'tgach.
2. `status.md` ni qisqacha xulosa va determinizm hisobotiga havola bilan yangilang.
3. Har qanday keyingi topshiriqlarni (masalan, profilni yaratish boʻyicha qoʻshimcha yoʻriqnoma) faylga kiriting
   `roadmap.md`.