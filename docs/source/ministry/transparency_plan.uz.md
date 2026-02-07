---
lang: uz
direction: ltr
source: docs/source/ministry/transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2639f4f7692e13ed61cc6246c87b047dc7415a6c9243ca7c046e6ccea8b55e9a
source_last_modified: "2025-12-29T18:16:35.983537+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency & Audit Plan
summary: Implementation plan for roadmap item MINFO-8 covering quarterly transparency reports, privacy guardrails, dashboards, and automation.
translator: machine-google-reviewed
---

# Shaffoflik va audit hisobotlari (MINFO-8)

“Yoʻl xaritasi” maʼlumotnomasi: **MINFO-8 — Shaffoflik va audit hisobotlari** va **MINFO-8a — Maxfiylikni saqlash jarayoni**

Axborot vazirligi deterministik shaffoflik artefaktlarini nashr etishi kerak, shunda hamjamiyat moderatsiya samaradorligini, murojaatlarni ko'rib chiqishni va qora ro'yxatni buzishni tekshirishi mumkin. Ushbu hujjat MINFO-8-ni Q32026 maqsadidan oldin yopish uchun zarur bo'lgan qamrov, artefaktlar, maxfiylik nazorati va operatsion ish jarayonini belgilaydi.

## Maqsadlar va bajariladigan narsalar

- Har chorakda AI moderatsiyasining aniqligi, apellyatsiya natijalari, rad etishlar ro'yxati, ko'ngillilar guruhi faoliyati va MINFO byudjetlari bilan bog'liq g'aznachilik harakati umumlashtiriladigan shaffoflik paketlarini ishlab chiqing.
- Xom-ma'lumotlar to'plamlari (Norito JSON + CSV) va asboblar panelini yuboring, shunda fuqarolar statik PDF-fayllarni kutmasdan o'lchovlarni kesishlari mumkin.
- Har qanday ma'lumotlar to'plami nashr etilishidan oldin maxfiylik kafolatlarini (differensial maxfiylik + minimal hisoblash qoidalari) va imzolangan sertifikatlarni amalga oshiring.
- Har bir nashrni DAG va SoraFS boshqaruvida yozib oling, shunda tarixiy artefaktlar o'zgarmas va mustaqil ravishda tekshirilishi mumkin.

### Artefakt matritsasi

| Artefakt | Tavsif | Format | Saqlash |
|----------|-------------|--------|---------|
| Shaffoflik haqida xulosa | Ijroiya xulosasi, diqqatga sazovor joylari, xavf-xatarli bandlari bilan inson o'qiy oladigan hisobot | Markdown → PDF | `docs/source/ministry/reports/<YYYY-Q>.md` → `artifacts/ministry/transparency/<YYYY-Q>/summary.pdf` |
| Ma'lumotlar ilovasi | Kanonik Norito to'plami tozalangan jadvallar (`ModerationLedgerBlockV1`, murojaatlar, qora ro'yxat deltalari) | `.norito` + `.json` | `artifacts/ministry/transparency/<YYYY-Q>/data` (SoraFS CID ga aks ettirilgan) |
| Ko'rsatkichlar CSV | Boshqaruv panellari uchun qulaylik CSV eksporti (AI FP/FN, murojaat SLA, rad etish roʻyxati) | `.csv` | Xuddi shu katalog, xeshlangan va imzolangan |
| Boshqaruv panelining surati | `ministry_transparency_overview` panellarining Grafana JSON eksporti + ogohlantirish qoidalari | `.json` | `dashboards/grafana/ministry_transparency_overview.json` / `dashboards/alerts/ministry_transparency_rules.yml` |
| Manifest kelib chiqishi | Norito manifest bog'lovchi dayjestlar, SoraFS CID, imzolar, chiqarish vaqt tamg'asi | `.json` + ajratilgan imzo | `artifacts/ministry/transparency/<YYYY-Q>/manifest.json(.sig)` (shuningdek, boshqaruv ovoziga biriktirilgan) |

## Ma'lumot manbalari va quvur liniyasi

| Manba | Tasma | Eslatmalar |
|--------|------|-------|
| Moderatsiya kitobi (`docs/source/sorafs_transparency_plan.md`) | CAR fayllarida saqlanadigan soatlik `ModerationLedgerBlockV1` eksportlari | SFM-4c uchun allaqachon jonli; choraklik yig'ish uchun qayta foydalaniladi. |
| AI kalibrlash + noto'g'ri-musbat stavkalar | `docs/source/sorafs_ai_moderation_plan.md` armatura + kalibrlash manifestlari (`docs/examples/ai_moderation_calibration_*.json`) | Ko‘rsatkichlar siyosat, mintaqa va model profili bo‘yicha jamlangan. |
| Apellyatsiya reestri | MINFO-7 xazina asboblari tomonidan chiqarilgan Norito `AppealCaseV1` hodisalari | Tarkibida ulush o‘tkazmalari, panel ro‘yxati, SLA taymerlari mavjud. |
| Rad etish ro'yxati | Merkle registridagi `MinistryDenylistChangeV1` voqealari (MINFO-6) | Xesh oilalari, TTL, favqulodda vaziyat bayroqlarini o'z ichiga oladi. |
| G'aznachilik oqimlari | `MinistryTreasuryTransferV1` voqealari (apellyatsiya depozitlari, panel mukofotlari) | `finance/mminfo_gl.csv` ga nisbatan muvozanatlangan. |Favqulodda kanon boshqaruvi, TTL cheklovlari va ko'rib chiqish talablari hozirda mavjud
[`docs/source/ministry/emergency_canon_policy.md`](emergency_canon_policy.md), taʼminlash
ajralish ko'rsatkichlari darajani (`standard`, `emergency`, `permanent`), kanon identifikatorini qamrab oladi.
va Torii yuklash vaqtida amalga oshiradigan muddatlarni ko'rib chiqing.

Qayta ishlash bosqichlari:
1. **Ingest** xom hodisalarini `ministry_transparency_ingest` (shaffoflik daftarini qabul qiluvchini aks ettiruvchi Rust xizmati). Kechasi ishlaydi, idempotent.
2. `ministry_transparency_builder` bilan har chorakda **Agregat**. Maxfiylik filtrlaridan oldin Norito maʼlumotlar ilovasini va har bir metrik jadvalni chiqaradi.
3. `cargo xtask ministry-transparency sanitize` (yoki `scripts/ministry/dp_sanitizer.py`) orqali **Sanitizatsiya** koʻrsatkichlarini va metamaʼlumotlar bilan CSV/JSON boʻlaklarini chiqaring.
4. **Paket** artefaktlari, ularni `ministry_release_signer` bilan imzolang va SoraFS + boshqaruv DAG ga yuklang.

## 2026-3-chorak ma'lumotnoma nashri

- Boshqaruvga asoslangan inauguratsion paket (2026-3-chorak) 2026-10-07 da `make check-ministry-transparency` orqali ishlab chiqarilgan. Artefaktlar `artifacts/ministry/transparency/2026-Q3/` da yashaydi, jumladan, `sanitized_metrics.json`, `dp_report.json`, `summary.md`, `checksums.sha256`, `transparency_manifest.json` va I101X va koʻzguda aks ettirilgan. SoraFS CID `7f4c2d81a6b13579ccddeeff00112233`.
- Nashr ma'lumotlari, ko'rsatkichlar jadvallari va tasdiqlashlar `docs/source/ministry/reports/2026-Q3.md` da qayd etilgan bo'lib, u endi Q3 oynasini ko'rib chiqayotgan auditorlar uchun kanonik ma'lumotnoma bo'lib xizmat qiladi.
- CI relizlar sahnalashtirishdan oldin, artefakt digestlari, Grafana/ogohlantirish xeshlari va manifest metamaʼlumotlarini tekshirishdan oldin `ci/check_ministry_transparency.sh` / `make check-ministry-transparency` ni qoʻllaydi, shuning uchun har chorakda bir xil dalillar izi kuzatiladi.

## Ko'rsatkichlar va asboblar paneli

Grafana asboblar paneli (`dashboards/grafana/ministry_transparency_overview.json`) quyidagi panellarni ko'rsatadi:

- AI moderatsiyasining aniqligi: har bir modeldagi FP/FN tezligi, drift va kalibrlash maqsadi va `docs/source/sorafs/reports/ai_moderation_calibration_*.md` bilan bog'langan ogohlantirish chegaralari.
- Apellyatsiya muddati: taqdim etishlar, SLA muvofiqligi, bekor qilishlar, obligatsiyalarni yoqib yuborish, har bir bosqichda kechikish.
- Rad etish ro'yxati: har bir xesh oilasiga qo'shimchalar/o'chirishlar, TTL muddati tugashi, favqulodda kanon chaqiruvlari.
- Ko'ngillilar brifinglari va panellarning xilma-xilligi: har bir til bo'yicha taqdimotlar, manfaatlar to'qnashuvini oshkor qilish, nashr etishda kechikish. Balanslangan qisqacha maydonlar `docs/source/ministry/volunteer_brief_template.md` da ko'rsatilgan, bu faktlar jadvallari va moderatsiya teglari mashinada o'qilishini ta'minlaydi.
- G'aznachilik qoldiqlari: depozitlar, to'lovlar, to'lanmagan majburiyatlar (MINFO-7 ta'minoti).

Ogohlantirish qoidalari (`dashboards/alerts/ministry_transparency_rules.yml` da kodlangan) quyidagilarni o'z ichiga oladi:
- FP/FN og'ishi kalibrlashning asosiy chizig'iga nisbatan >25%.
- Apellyatsiya SLA o'tkazib yuborish darajasi har chorakda >5%.
- Favqulodda kanon TTLlari siyosatdan eski.
- Chorak yopilganidan keyin nashrning kechikishi >14 kun.

## Maxfiylik va ozodlik himoyasi (MINFO-8a)| Metrik sinf | Mexanizm | Parametrlar | Qo'shimcha soqchilar |
|-------------|-----------|------------|-------------------|
| Hisoblar (apellyatsiyalar, qora ro'yxatga o'zgartirishlar, ixtiyoriy brifinglar) | Laplas shovqini | har chorakda e=0,75, d=1e-6 | Shovqindan keyingi qiymati <5 bo'lgan chelaklarni bostirish; har chorakda aktyor uchun 1 ta klip hissasi. |
| AI aniqligi | Numerator/maxrajdagi Gauss shovqini | e=0,5, d=1e-6 | Namuna soni ≥50 (`min_accuracy_samples` qavat) sanitarizatsiya qilinganidan keyin chiqaring va ishonch oralig'ini e'lon qiling. |
| G'aznachilik oqimlari | Shovqin yo'q (allaqachon ommaviy tarmoqda) | — | G'aznachilik identifikatorlaridan tashqari niqob nomlari; Merkle dalillarini o'z ichiga oladi. |

Chiqarish talablari:
- Differentsial maxfiylik hisobotlari epsilon/delta kitobi va RNG urug'lik majburiyatini (`blake3(seed)`) o'z ichiga oladi.
- Nozik misollar (dalil xeshlari), agar allaqachon ommaviy Merkle kvitansiyalari bo'lmasa, tahrirlangan.
- Barcha olib tashlangan maydonlarni va asoslashni tavsiflovchi xulosaga ilova qilingan tahrirlash jurnali.

## Ish jarayoni va vaqt jadvalini nashr qilish

| T-Oyna | Vazifa | Ega(lar)i | Dalil |
|----------|------|----------|----------|
| Chorak yopilgandan keyin T+3d | Xom eksportni muzlatish, yig'ish ishini ishga tushirish | Vazirlikning kuzatuv qobiliyati TL | `ministry_transparency_ingest.log`, quvur liniyasi ish identifikatori |
| T+7d | Xom o'lchovlarni ko'rib chiqing, DP sanitizer quruq ishini ishlating | Data Trust jamoasi | Sanitizer hisoboti (`artifacts/.../dp_report.json`) |
| T+10d | Xulosa loyihasi + ma'lumotlar ilovasi | Docs/DevRel + Siyosat tahlilchisi | `docs/source/ministry/reports/<YYYY-Q>.md` |
| T+12d | Artefaktlarni imzolang, manifest tayyorlang, SoraFS | ga yuklang Operatsiyalar / Boshqaruv kotibiyati | `manifest.json(.sig)`, SoraFS CID |
| T+14d | Boshqaruv panellari + ogohlantirishlarni, boshqaruvdan keyingi e'lonlarni nashr qilish | Kuzatish qobiliyati + Comms | Grafana eksporti, ogohlantirish qoidasi xeshi, boshqaruv ovozi havolasi |

Har bir nashr quyidagi tomonidan tasdiqlanishi kerak:
1. Vazirlikning kuzatuv qobiliyati TL (maʼlumotlar yaxlitligi)
2. Boshqaruv kengashi bilan aloqa (siyosat)
3. Docs/Comms yetakchisi (ommaviy matn)

## Avtomatlashtirish va dalillarni saqlash- `cargo xtask ministry-transparency ingest` dan choraklik suratni xom tasmalardan (buxgalteriya kitobi, murojaatlar, rad etish roʻyxati, xazina, koʻngilli) yaratish uchun foydalaning. Nashr qilishdan oldin JSON ko'rsatkichlarini va imzolangan manifestni chiqarish uchun `cargo xtask ministry-transparency build` bilan kuzatib boring.
- Qizil jamoa aloqasi: bir yoki bir nechta `--red-team-report docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` fayllarini qabul qilish bosqichiga o'tkazing, shunda shaffoflik surati va tozalangan ko'rsatkichlar daftar/apellyatsiya/inkor ro'yxati ma'lumotlari bilan bir qatorda matkap identifikatorlari, stsenariy sinflari, dalillar to'plami yo'llari va asboblar panelidagi SHA-larni olib yuradi. Bu MINFO-9 matkap kadansini qo'lda tahrirlarsiz har bir shaffoflik paketida aks ettiradi.
- Ko'ngillilar tomonidan taqdim etilgan arizalar `docs/source/ministry/volunteer_brief_template.md` ga muvofiq bo'lishi kerak (misol: `docs/examples/ministry/volunteer_brief_template.json`). Qabul qilish bosqichi ushbu ob'ektlarning JSON massivini kutadi, `moderation.off_topic` yozuvlarini avtomatik ravishda filtrlaydi, oshkor qilish sertifikatlarini qo'llaydi va asboblar paneli etishmayotgan iqtiboslarni ajratib ko'rsatishi uchun faktlar jadvalini qamrab oladi.
- Qo'shimcha avtomatlashtirish `scripts/ministry/` ostida ishlaydi. `dp_sanitizer.py` `cargo xtask ministry-transparency sanitize` buyrug'ini o'rab oladi, `transparency_release.py` esa artefaktlarni paketlaydi, SoraFS CIDni Norito yoki tushuntirishdan oladi. `--sorafs-cid`) va `transparency_manifest.json` va `transparency_release_action.json` (manifest dayjest, SoraFS CID va SHA panellarini qamrab oluvchi `TransparencyReleaseV1` boshqaruv yuki) ham yozadi. `--governance-dir <path>` ni `transparency_release.py` ga o'tkazing (yoki `cargo xtask ministry-transparency anchor --action artifacts/.../transparency_release_action.json --governance-dir <path>` ni ishga tushiring) Norito foydali yukini kodlash va uni (plyus JSON xulosasini) nashr qilishdan oldin boshqaruv DAG katalogiga tashlang. Xuddi shu bayroq `MinistryTransparencyHeadUpdateV1` so'rovini `<governance-dir>/publisher/head_requests/ministry_transparency/` ostida ham chiqaradi, chorak, SoraFS CID, manifest yo'llari va IPNS kalit taxalluslari (`--ipns-key` orqali bekor qilinadi). `--auto-head-update` ni `publisher_head_updater.py` orqali darhol ko'rib chiqish uchun `--auto-head-update` ni taqdim eting, agar IPNS bir vaqtning o'zida chop etilishi kerak bo'lsa, `--head-update-ipns-template '/usr/local/bin/ipfs name publish --key {ipns_key} /ipfs/{cid}'` orqali o'ting. Aks holda, navbatni to'kib tashlash, `publisher/head_updates.log` qo'shish, `publisher/ipns_heads/<key>.json`ni yangilash va qayta ishlangan JSON faylini `head_requests/ministry_transparency/processed/` ostida arxivlash uchun `scripts/ministry/publisher_head_updater.py --governance-dir <path>` dasturini keyinroq ishga tushiring (agar kerak bo'lsa, xuddi shu andoza bilan).
- Chiqarish kaliti bilan imzolangan `checksums.sha256` fayli bilan `artifacts/ministry/transparency/<YYYY-Q>/` ostida saqlangan artefaktlar. Daraxt hozirda `artifacts/ministry/transparency/2026-Q3/` (sanitizatsiya qilingan koʻrsatkichlar, DP hisoboti, xulosa, manifest, boshqaruv harakati) da mos yozuvlar toʻplamini olib yuradi, shuning uchun muhandislar asbobni oflayn rejimda sinab koʻrishlari mumkin va `scripts/ministry/check_transparency_release.py` dayjestlar/chorak metamaʼlumotlarini mahalliy ravishda tekshiradi, Grafana da bir xil dalillar mavjud. yuklangan. Tekshiruvchi endi hujjatlashtirilgan DP budjetlarini (hisoblash uchun e≤0,75, aniqlik uchun e≤0,5, d≤1e−6) amalga oshiradi va `min_accuracy_samples` yoki bostirish chegarasi drifti yoki chelak o‘sha qavatdan past bo‘lmagan qiymatlar bo‘lsa, qurishni muvaffaqiyatsiz tugatadi. Skriptni yo‘l xaritasi (MINFO‑8) va CI o‘rtasidagi shartnoma sifatida ko‘ring: agar maxfiylik parametrlari o‘zgarsa, yuqoridagi jadval va tekshirgichni birgalikda sozlang.- Boshqaruv langari: manifest dayjestiga, SoraFS CID va asboblar paneliga git SHA (`iroha_data_model::ministry::TransparencyReleaseV1` kanonik foydali yukni belgilaydi) havolasi bilan `TransparencyReleaseV1` harakatini yarating.

## Vazifalar va keyingi qadamlarni oching

| Vazifa | Holati | Eslatmalar |
|------|--------|-------|
| `ministry_transparency_ingest` + quruvchi ish o'rinlarini amalga oshirish | 🈺 Davom etmoqda | `cargo xtask ministry-transparency ingest|build` endi daftar/apellyatsiya/inkor ro'yxati/g'aznachilik manbalarini tikadi; qolgan ish simlari DP sanitizer + reliz skript quvur liniyasi. |
| Grafana boshqaruv paneli + ogohlantirish paketini nashr qilish | 🈴 Tugallandi | Boshqaruv paneli + ogohlantirish fayllari `dashboards/grafana/ministry_transparency_overview.json` va `dashboards/alerts/ministry_transparency_rules.yml` ostida ishlaydi; ishga tushirish vaqtida ularni PagerDuty `ministry-transparency` ga ulang. |
| DP sanitizer + kelib chiqish manifestini avtomatlashtirish | 🈴 Tugallandi | `cargo xtask ministry-transparency sanitize` (oʻram: `scripts/ministry/dp_sanitizer.py`) tozalangan koʻrsatkichlar + DP hisobotini chiqaradi va `scripts/ministry/transparency_release.py` endi kelib chiqishi uchun `checksums.sha256` va `transparency_manifest.json` yozadi. |
| Har choraklik hisobot shablonini yaratish (`reports/<YYYY-Q>.md`) | 🈴 Tugallandi | `docs/source/ministry/reports/2026-Q3-template.md` da qo'shilgan shablon; har chorakda nusxa ko'chiring/nomini o'zgartiring va nashr qilishdan oldin `{{...}}` tokenlarini almashtiring. |
| Simli boshqaruv DAG ankraj | 🈴 Tugallandi | `TransparencyReleaseV1` `iroha_data_model::ministry` da yashaydi, `scripts/ministry/transparency_release.py` JSON foydali yukini chiqaradi va `cargo xtask ministry-transparency anchor` `.to` artefaktini DAG nashri direktorini avtomatik ravishda chiqarish uchun tuzilgan konfiguratsiyaga kodlaydi. |

Hujjatni, asboblar panelini va ish jarayonini yetkazib berish MINFO-8 ni 🈳 dan 🈺 ga o'tkazadi. Qolgan muhandislik vazifalari (ishlar, skriptlar, ogohlantirish kabellari) yuqoridagi jadvalda kuzatilgan va birinchi Q32026 nashridan oldin yopilishi kerak.