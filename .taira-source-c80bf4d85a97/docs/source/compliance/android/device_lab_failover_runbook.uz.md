---
lang: uz
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2025-12-29T18:16:35.923579+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device Lab Failover Drill Runbook (AND6/AND7)

Ushbu ish kitobida protsedura, dalillar talablari va aloqa matritsasi mavjud
havola qilingan **qurilma-laboratoriya favqulodda vaziyat rejasini** bajarishda foydalaniladi
`roadmap.md` (§“Reglamentar artefaktlarni tasdiqlash va laboratoriya holatlari”). To'ldiradi
band qilish ish jarayoni (`device_lab_reservation.md`) va hodisalar jurnali
(`device_lab_contingency.md`) shuning uchun muvofiqlikni tekshiruvchilar, yuridik maslahatchilar va SRE
muvaffaqiyatsizlikka tayyorligini qanday tekshirishimiz uchun yagona haqiqat manbasiga ega bo'ling.

## Maqsad va tezlik

- Android StrongBox + umumiy qurilma pullari ishlamay qolishi mumkinligini ko'rsating
  zaxira Pixel qatorlariga, umumiy hovuzga, Firebase Test Lab yorilish navbatiga va
  AND6/AND7 SLA'larini yo'qotmasdan tashqi StrongBox ushlagichi.
- ETSI/FISC taqdimnomalariga qonuniy biriktirilishi mumkin bo'lgan dalillar to'plamini yarating
  fevraldagi muvofiqlikni tekshirishdan oldin.
- Har chorakda kamida bir marta, shuningdek, laboratoriya apparatlari ro'yxati o'zgarganda bajaring
  (yangi qurilmalar, nafaqaga chiqish yoki 24 soatdan ortiq texnik xizmat ko'rsatish).

| Matkap ID | Sana | Ssenariy | Dalillar to'plami | Holati |
|----------|------|----------|-----------------|--------|
| DR-2026-02-1-chorak | 2026-02-20 | Simulyatsiya qilingan Pixel8Pro yo'lidagi uzilish + AND7 telemetriya mashqlari bilan attestatsiyadan kechikish | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ Bajarildi — `docs/source/compliance/android/evidence_log.csv` da yozilgan toʻplam xeshlari. |
| DR-2026-05-2-chorak | 2026-05-22 (rejalashtirilgan) | StrongBox texnik xizmat ko'rsatish o'xshashligi + Nexus mashq | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(kutishda)* — `_android-device-lab` chiptasi **AND6-DR-202605** bandlovlarga ega; to'plam burg'ulashdan keyin to'ldiriladi. | 🗓 Rejalashtirilgan — kalendar bloki “Android Device Lab – Reservations” ga AND6 kadensiga qo‘shildi. |

## Jarayon

### 1. Matkapdan oldingi tayyorgarlik

1. `docs/source/sdk/android/android_strongbox_capture_status.md` da asosiy quvvatni tasdiqlang.
2. Maqsadli ISO haftasi uchun zahira taqvimini orqali eksport qiling
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. Fayl `_android-device-lab` chiptasi
   `AND6-DR-<YYYYMM>` ko‘lami (“to‘xtatib turish matkapi”), rejalashtirilgan slotlar va ta’sirlangan
   ish yuklari (attestatsiya, CI tutuni, telemetriya xaos).
4. `device_lab_contingency.md` da favqulodda vaziyatlar jurnali shablonini yangilang
   burg'ulash sanasi uchun to'ldiruvchi qator.

### 2. Muvaffaqiyatsizlik holatlarini simulyatsiya qilish

1. Laboratoriya ichidagi asosiy qatorni (`pixel8pro-strongbox-a`) o'chiring yoki zaxiralang
   rejalashtiruvchi va bron yozuvini "matkap" deb belgilang.
2. PagerDuty (`AND6-device-lab` xizmati) da soxta uzilish ogohlantirishini ishga tushiring va
   dalillar to'plami uchun bildirishnoma eksportini yozib oling.
3. Odatda chiziqni iste'mol qiladigan Buildkite ishlariga izoh bering
   (`android-strongbox-attestation`, `android-ci-e2e`) matkap identifikatori bilan.

### 3. Failover bajarilishi1. Qo'shimcha Pixel7 qatorini asosiy CI maqsadiga ko'taring va uni rejalashtiring
   unga qarshi rejalashtirilgan ish yuklari.
2. `firebase-burst` qatori orqali Firebase Test Lab portlash to‘plamini ishga tushiring.
   StrongBox qamrovi birgalikda harakatlanayotganda chakana hamyonning tutuni sinovdan o'tkaziladi
   qator. Audit chiptasida CLI chaqiruvini (yoki konsol eksportini) yozib oling
   paritet.
3. Qisqa muddatli attestatsiyani tekshirish uchun tashqi StrongBox laboratoriya saqlovchisini ishga tushiring;
   quyida ta'riflanganidek, kontaktni tasdiqlang.
4. Barcha Buildkite ishga tushirish identifikatorlari, Firebase ish URL manzillari va saqlovchi transkriptlarini yozib oling.
   `_android-device-lab` chiptasi va dalillar to'plami manifest.

### 4. Tasdiqlash va qaytarish

1. Attestatsiya/CI ish vaqtlarini boshlang'ich bilan solishtiring; bayroq deltalari >10% gacha
   Uskuna laboratoriyasi rahbari.
2. Asosiy chiziqni tiklang va sig'imning suratini va tayyorlikni yangilang
   tasdiqlashdan o'tgandan keyin matritsa.
3. Yakuniy qatorni `device_lab_contingency.md` ga trigger, amallar,
   va kuzatuvlar.
4. `docs/source/compliance/android/evidence_log.csv` ni quyidagi bilan yangilang:
   to'plam yo'li, SHA-256 manifesti, Buildkite ishga tushirish identifikatorlari, PagerDuty eksport xeshi va
   sharhlovchining ro'yxatdan o'tishi.

## Dalillar toʻplami tartibi

| Fayl | Tavsif |
|------|-------------|
| `README.md` | Xulosa (mashq identifikatori, ko'lami, egalari, vaqt jadvali). |
| `bundle-manifest.json` | To'plamdagi har bir fayl uchun SHA-256 xaritasi. |
| `calendar-export.{ics,json}` | Eksport skriptidan ISO haftalik bandlov taqvimi. |
| `pagerduty/incident_<id>.json` | PagerDuty hodisasi eksporti ogohlantirish + tasdiqlash vaqt jadvalini ko'rsatadi. |
| `buildkite/<job>.txt` | Buildkite ishga tushirilgan URL manzillari va ta'sirlangan ishlar uchun jurnallar. |
| `firebase/burst_report.json` | Firebase Test Lab portlash ijrosi haqida xulosa. |
| `retainer/acknowledgement.eml` | Tashqi StrongBox laboratoriyasidan tasdiq. |
| `photos/` | Uskuna qayta kabellangan bo'lsa, laboratoriya topologiyasining ixtiyoriy fotosuratlari/skrinshotlari. |

To'plamni quyidagi manzilda saqlang
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` va yozib oling
dalillar jurnalidagi manifest nazorat summasi va AND6 muvofiqligini tekshirish ro'yxati.

## Aloqa va eskalatsiya matritsasi

| Rol | Asosiy aloqa | Kanal(lar) | Eslatmalar |
|------|-----------------|------------|-------|
| Uskuna laboratoriyasi rahbari | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | Saytdagi harakatlar va taqvim yangilanishlariga egalik qiladi. |
| Device Lab Operations | Mateo Kruz | `_android-device-lab` navbat | Chiptalarni bron qilish + yuklarni yuklashni muvofiqlashtiradi. |
| Release Engineering | Aleksey Morozov | Eng Slackni chiqaring · `release-eng@iroha.org` | Buildkite dalillarini tasdiqlaydi + xeshlarni nashr etadi. |
| Tashqi StrongBox laboratoriyasi | Sakura Instruments MOK | `noc@sakura.example` · +81-3-5550-9876 | Tutuvchi bilan aloqa; 6 soat ichida mavjudligini tasdiqlang. |
| Firebase Burst Koordinatori | Tessa Rayt | `@android-ci` Slack | Qayta tiklash zarur bo'lganda Firebase Test Lab avtomatizatsiyasini ishga tushiradi. |

Agar mashq blokirovka bilan bog'liq muammolarni aniqlasa, quyidagi tartibda oshiring:
1. Uskuna laboratoriyasi rahbari
2. Android Foundations TL
3. Dastur rahbari / reliz muhandisligi
4. Muvofiqlik bo'yicha yetakchi + yuridik maslahatchi (agar mashq tartibga solish xavfini aniqlasa)

## Hisobot va kuzatish- Har doim havola qilganda, ushbu ish kitobini bron qilish tartibi bilan bog'lang
  `roadmap.md`, `status.md` va boshqaruv paketlarida uzilishga tayyorlik.
- Har choraklik mashg'ulot xulosasini dalillar to'plami bilan Muvofiqlik + Huquqiy elektron pochta manziliga yuboring
  hash jadvali va `_android-device-lab` chipta eksportini biriktiring.
- Asosiy ko'rsatkichlarni aks ettiring (to'ldirilish vaqti, tiklangan ish yuklari, ajoyib harakatlar)
  `status.md` va AND7 hot-list trekeri ichida sharhlovchilar kuzatishi mumkin
  aniq mashqga bog'liqlik.