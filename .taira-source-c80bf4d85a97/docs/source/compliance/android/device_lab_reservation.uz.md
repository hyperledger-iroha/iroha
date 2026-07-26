---
lang: uz
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2025-12-29T18:16:35.924530+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android qurilma laboratoriyasini band qilish tartibi (AND6/AND7)

Bu oʻyin kitobida Android jamoasi qurilmani qanday band qilishi, tasdiqlashi va tekshirishi tasvirlangan
**AND6** (CI va muvofiqlikni mustahkamlash) va **AND7** bosqichlari uchun laboratoriya vaqti
(kuzatishga tayyorlik). Bu favqulodda tizimga kirishni to'ldiradi
Imkoniyatlarni ta'minlash orqali `docs/source/compliance/android/device_lab_contingency.md`
birinchi navbatda kamchiliklarning oldi olinadi.

## 1. Maqsadlar va qamrov

- StrongBox + umumiy qurilma hovuzlarini yo'l xaritasi tomonidan belgilangan 80% dan yuqori saqlang
  muzlatish oynalari bo'ylab sig'im maqsadi.
- CI, attestatsiya tekshiruvlari va tartibsizliklar uchun deterministik kalendarni taqdim eting
  mashqlar bir xil apparat uchun raqobat hech qachon.
- Taqdim etiladigan izni (so'rovlar, tasdiqlashlar, ishga tushirishdan keyingi eslatmalar) oling
  AND6 muvofiqligini tekshirish ro'yxati va dalillar jurnali.

Ushbu protsedura ajratilgan Pixel qatorlarini, umumiy zaxira hovuzini va
yo'l xaritasida havola qilingan tashqi StrongBox laboratoriya saqlovchisi. Ad-hoc emulyator
foydalanish doirasidan tashqarida.

## 2. Reservation Windows

| Hovuz / Lane | Uskuna | Standart Slot uzunligi | Buyurtma berish muddati | Egasi |
|-------------|----------|---------------------|-------------------|-------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4 soat | 3 ish kuni | Uskuna laboratoriyasi rahbari |
| `pixel8a-ci-b` | Pixel8a (CI umumiy) | 2 soat | 2 ish kuni | Android Foundations TL |
| `pixel7-fallback` | Pixel7 umumiy hovuz | 2 soat | 1 ish kuni | Release Engineering |
| `firebase-burst` | Firebase Test Lab tutun navbati | 1 soat | 1 ish kuni | Android Foundations TL |
| `strongbox-external` | Tashqi StrongBox laboratoriya ushlagichi | 8 soat | 7 kalendar kuni | Dastur rahbari |

Slotlar UTC da band qilinadi; bir-biriga o'xshash shartlar aniq tasdiqlashni talab qiladi
apparat laboratoriyasi rahbaridan.

## 3. Ish jarayonini so'rash

1. **Kontekst tayyorlang**
   - `docs/source/sdk/android/android_strongbox_device_matrix.md` bilan yangilang
     mashq qilishni rejalashtirgan qurilmalar va tayyorlik yorlig'i
     (`attestation`, `ci`, `chaos`, `partner`).
   - Eng so'nggi sig'imli suratni to'plang
     `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. **So‘rov yuborish**
   - Shablondan foydalanib, `_android-device-lab` navbatiga chipta yozing.
     `docs/examples/android_device_lab_request.md` (egasi, sanalar, ish yuklari,
     qayta tiklash talabi).
   - Har qanday tartibga soluvchi bog'liqliklarni biriktiring (masalan, AND6 attestatsiyasini tekshirish, AND7
     telemetriya mashqlari) va tegishli yo'l xaritasi yozuviga havola.
3. **Tasdiqlash**
   - Hardware Lab Lead bir ish kuni ichida ko'rib chiqadi, slotni tasdiqlaydi
     umumiy kalendar (`Android Device Lab – Reservations`) va yangilanadi
     `device_lab_capacity_pct` ustuni ichida
     `docs/source/compliance/android/evidence_log.csv`.
4. **Ijro etish**
   - rejalashtirilgan ishlarni bajarish; Buildkite ishga tushirish identifikatorlarini yoki asboblar jurnallarini yozib oling.
   - Har qanday og'ishlarga e'tibor bering (apparat almashinuvi, ortiqcha yuk).
5. **Yopish**
   - Artefaktlar/havolalar bilan chiptaga sharh bering.
   - Agar ishga tushirish muvofiqlik bilan bog'liq bo'lsa, yangilang
     `docs/source/compliance/android/and6_compliance_checklist.md` va qator qo'shing
     `evidence_log.csv` gacha.

Hamkorlar demolariga (AND8) taʼsir etuvchi soʻrovlar Partner Engineering cc boʻlishi kerak.

## 4. O'zgartirish va bekor qilish- **Qayta rejalashtirish:** original chiptani qayta oching, yangi joy taklif qiling va yangilang
  kalendar yozuvi. Agar yangi slot 24 soat ichida bo'lsa, Hardware Lab Lead + SRE ping
  bevosita.
- **Favqulodda vaziyatni bekor qilish:** favqulodda vaziyat rejasiga amal qiling
  (`device_lab_contingency.md`) va trigger/harakat/kuzatuv qatorlarini yozib oling.
- **Oshib ketish:** agar yugurish o'z vaqtidan >15 daqiqaga oshsa, yangilanishni joylashtiring va tasdiqlang
  keyingi bron qilish davom etishi mumkinmi; aks holda zaxiraga topshiring
  hovuz yoki Firebase burst lane.

## 5. Dalillar va audit

| Artefakt | Manzil | Eslatmalar |
|----------|----------|-------|
| Chiptalarni bron qilish | `_android-device-lab` navbat (Jira) | Eksport haftalik xulosasi; dalillar jurnalida chipta identifikatorlarini bog'lang. |
| Taqvim eksporti | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` | Har juma kuni `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` ishga tushiring; yordamchi filtrlangan `.ics` faylini hamda ISO haftasi uchun JSON xulosasini saqlaydi, shuning uchun audit har ikkala artefaktni ham qoʻlda yuklab olmasdan biriktira oladi. |
| Imkoniyatli suratlar | `docs/source/compliance/android/evidence_log.csv` | Har bronlash/yopilishdan keyin yangilang. |
| Ishdan keyingi qaydlar | `docs/source/compliance/android/device_lab_contingency.md` (agar kutilmagan holatda) yoki chipta sharhi | Audit uchun talab qilinadi. |

Har chorakda muvofiqlikni tekshirish paytida taqvim eksporti, chipta xulosasi,
va AND6 nazorat ro'yxatini topshirish uchun dalillar jurnalidan ko'chirma.

### Taqvim eksportini avtomatlashtirish

1. “Android Device Lab – Reservations” uchun ICS tasma URL manzilini oling (yoki `.ics` faylini yuklab oling).
2. Bajarmoq

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   Skript ikkala `artifacts/android/device_lab/<YYYY-WW>-calendar.ics` ni yozadi
   va `...-calendar.json`, tanlangan ISO haftasini suratga oladi.
3. Yaratilgan fayllarni haftalik dalillar paketi bilan yuklang va havola qiling
   `docs/source/compliance/android/evidence_log.csv` da JSON xulosasi qachon
   ro'yxatga olish qurilmasi-laboratoriya sig'imi.

## 6. Eskalatsiya zinapoyasi

1. Uskuna laboratoriyasi rahbari (asosiy)
2. Android Foundations TL
3. Dastur yetakchisi/reliz muhandisligi (oynalarni muzlatish uchun)
4. Tashqi StrongBox laboratoriya aloqasi (ushlagich chaqirilganda)

Eskalatsiyalar chiptaga kiritilishi va haftalik Android-da aks ettirilishi kerak
holat xati.

## 7. Tegishli hujjatlar

- `docs/source/compliance/android/device_lab_contingency.md` - hodisalar jurnali
  quvvatlarning etishmasligi.
- `docs/source/compliance/android/and6_compliance_checklist.md` - usta
  etkazib berishni nazorat qilish ro'yxati.
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — apparat
  qamrov kuzatuvchisi.
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` -
  AND6/AND7 tomonidan havola qilingan StrongBox sertifikati.

Ushbu band qilish tartibini saqlab qolish yo'l xaritasi "aniqlash" bandini qondiradi
qurilma-laboratoriyani band qilish tartib-qoidasi” va hamkorga tegishli muvofiqlik artefaktlarini saqlaydi
Android tayyorlik rejasining qolgan qismi bilan sinxronlashtiriladi.

## 8. Failover Drill Procedure & Contacts

“Yo‘l xaritasi”ning AND6 bandi, shuningdek, har chorakda bir martalik takroriy takroriy takrorlashni talab qiladi. To'liq,
bosqichma-bosqich ko'rsatmalar mavjud
`docs/source/compliance/android/device_lab_failover_runbook.md`, lekin yuqori
darajadagi ish jarayoni quyida umumlashtiriladi, shuning uchun so'rovchilar mashg'ulotlarni birga rejalashtirishlari mumkin
muntazam rezervasyonlar.1. **Matkapni rejalashtirish:** Ta'sir qilingan yo'llarni to'sib qo'ying (`pixel8pro-strongbox-a`,
   zaxira pul, `firebase-burst`, tashqi StrongBox saqlovchisi) umumiy
   taqvim va `_android-device-lab` matkapdan kamida 7 kun oldin navbat.
2. **To‘xtashga taqlid qiling:** Birlamchi qatorni ajrating, PagerDuty-ni ishga tushiring
   (`AND6-device-lab`) hodisasi va bog'liq Buildkite ishlariga izoh bering.
   ish kitobida qayd etilgan matkap identifikatori.
3. **Muvaffaqiyatsiz:** Pixel7 zaxira chizig‘ini targ‘ib qiling, Firebase portlashini boshlang
   to'plamini yarating va 6 soat ichida tashqi StrongBox hamkorini jalb qiling. Qo'lga olish
   Buildkite ishga tushirish URL manzillari, Firebase eksportlari va saqlovchi tasdiqlari.
4. **Tasdiqlash va tiklash:** Attestatsiya + CI ish vaqtlarini tekshiring, qayta tiklang
   original yo'llar, va yangilash `device_lab_contingency.md` plus dalillar jurnali
   to'plam yo'li + nazorat summalari bilan.

### Aloqa va eskalatsiya ma'lumotnomasi

| Rol | Asosiy aloqa | Kanal(lar) | Eskalatsiya tartibi |
|------|-----------------|------------|------------------|
| Uskuna laboratoriyasi rahbari | Priya Ramanathan | `@android-lab` Slack · +81-3-5550-1234 | 1 |
| Device Lab Operations | Mateo Kruz | `_android-device-lab` navbat | 2 |
| Android Foundations TL | Elena Vorobeva | `@android-foundations` Slack | 3 |
| Release Engineering | Aleksey Morozov | `release-eng@iroha.org` | 4 |
| Tashqi StrongBox laboratoriyasi | Sakura Instruments MOK | `noc@sakura.example` · +81-3-5550-9876 | 5 |

Agar matkap blokirovka bilan bog'liq muammolarni aniqlasa yoki biron bir nosozlik bo'lsa, ketma-ketlikni oshiring
qatorni 30 daqiqa ichida onlayn qilib bo'lmaydi. Har doim eskalatsiyani yozib oling
`_android-device-lab` chiptasidagi eslatmalar va ularni favqulodda vaziyatlar jurnalida aks ettiring.