---
lang: uz
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2025-12-29T18:16:35.952009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Docs/DevRel oylik sinxronlash kun tartibi

Ushbu kun tartibi bo'ylab havola qilingan oylik Docs/DevRel sinxronlashni rasmiylashtiradi
`roadmap.md` (qarang: “Oylik Docs/DevRel-ga mahalliylashtirish xodimlarini tekshirishni qo‘shish
sinxronlash”) va Android AND5 i18n rejasi. Uni kanonik nazorat ro'yxati sifatida foydalaning va
yo'l xaritasi natijalari kun tartibiga qo'shilgan yoki bekor qilinganda uni yangilang.

## Kadens va logistika

- **Chastotasi:** oylik (odatda ikkinchi payshanba, 16:00 UTC)
- ** Davomiyligi:** 45 daqiqa + chuqur sho'ng'in uchun ixtiyoriy 15 daqiqa orqaga surish
- **Joylashuv:** Kattalashtirish (`https://meet.sora.dev/docs-devrel-sync`) bilan birgalikda
  HackMD yoki `docs/source/docs_devrel/minutes/<yyyy-mm>.md` da qaydlar
- **Auditoriya:** Docs/DevRel menejeri (kafedra), Docs muhandislari, mahalliylashtirish
  dastur menejeri, SDK DX TLs (Android, Swift, JS), Mahsulot hujjatlari, Relizlar
  Muhandislik delegati, Yordam/QA kuzatuvchilari
- **Fasilitator:** Docs/DevRel menejeri; aylanuvchi kotib tayinlaydi
  daqiqalarni 24 soat ichida repoga topshiring

## Ishdan oldingi nazorat ro'yxati

| Egasi | Vazifa | Artefakt |
|-------|------|----------|
| Scribe | Quyidagi shablondan foydalanib, oylik qaydlar faylini (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`) yarating. | Eslatmalar fayli |
| Mahalliylashtirish PM | `docs/source/sdk/android/i18n_plan.md#translation-status` va xodimlar jurnalini yangilang; taklif qilingan qarorlarni oldindan to'ldirish. | i18n rejasi |
| DX TLs | `ci/check_android_docs_i18n.sh` yoki `scripts/sync_docs_i18n.py --dry-run` ni ishga tushiring va muhokama qilish uchun dayjestlarni biriktiring. | CI artefaktlari |
| Hujjatlar vositalari | `docs/i18n/manifest.json` dayjestlarini + `docs/source/sdk/android/i18n_requests/` dan ajoyib chiptalar roʻyxatini eksport qiling. | Manifest va chipta xulosasi |
| Qo'llab-quvvatlash/chiqarish | Docs/DevRel amalini talab qiladigan har qanday eskalatsiyalarni to‘plang (masalan, oldindan ko‘rish kutilayotgan takliflar, sharhlovchining fikr-mulohazalarini bloklash). | Status.md yoki eskalatsiya hujjati |

## Kun tartibi bloklari1. **Telefon va maqsadlar (5 daqiqa)**
   - Kvorumni, kotibni va logistikani tasdiqlang.
   - Shoshilinch hodisalarni ajratib ko'rsatish (hujjatlarni oldindan ko'rishning uzilishi, mahalliylashtirish bloki).
2. **Mahalliy xodimlarni tekshirish (15 daqiqa)**
   - Xodimlar to'g'risidagi qaror jurnalini ko'rib chiqing
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`.
   - Ochiq PO (`DOCS-L10N-*`) va oraliq qamrov holatini tasdiqlang.
   - Tarjima holati jadvali bilan CI yangiligi chiqishini solishtiring; istalganini chaqiring
     mahalliy SLA (>5 ish kuni) keyingisidan oldin buzilgan doc
     sinxronlash.
   - Eskalatsiya zarur yoki yo'qligini hal qiling (Mahsulot operatsiyalari, moliya, pudratchi
     boshqaruv). Qarorni ham xodimlar jurnaliga, ham oylik jurnalga yozib qo'ying
     daqiqalar, shu jumladan egasi + muddati.
   - Agar xodimlar sog'lom bo'lsa, yo'l xaritasi amal qilishi mumkin bo'lgan tasdiqni hujjatlashtiring
     dalil bilan 🈺/🈴 ga qayting.
3. **Hujjatlar/yo‘l xaritasi yangilanishi (10 daqiqa)**
   - DOCS-SORA portal ishining holati, Try-It proksi-server va SoraFS nashri
     tayyorlik.
   - Joriy chiqarilgan poezdlar uchun zarur bo'lgan hujjat qarzini yoki sharhlovchilarni ajratib ko'rsatish.
4. **SDK diqqatga sazovor joylari (10 daqiqa)**
   - Android AND5/AND7 hujjat tayyorligi, Swift IOS5 pariteti, JS GA jarayoni.
   - Hujjatlarga ta'sir qiladigan umumiy moslamalar yoki sxema farqlarini yozib oling.
5. **Harakatni ko‘rib chiqish va to‘xtash joyi (5 daqiqa)**
   - Oldingi sinxronlashdagi ochiq elementlarni qayta ko'rib chiqing; yopilishini tasdiqlang.
   - Eslatmalar faylida aniq egalari va muddatlari bilan yangi harakatlarni yozib oling.

## Mahalliylashtirish xodimlarini ko'rib chiqish shabloni

Har oyning daqiqalariga quyidagi jadvalni kiriting:

| Mahalliy | Imkoniyatlar (FTE) | Majburiyatlar va PO'lar | Risklar / Eskalatsiyalar | Qaror va Egasi |
|--------|----------------|-------------------|--------------------|------------------|
| JP | masalan, 0,5 pudratchi + 0,1 Hujjat zaxirasi | PO `DOCS-L10N-4901` (imzoni kutmoqda) | “Shartnoma 2026-03-04 gacha imzolanmagan” | “Mahsulot operatsiyalariga oʻtish — @docs-devrel, 2026-03-02 tugaydi” |
| U | masalan, 0,1 Hujjatlar muhandisi | Aylanish PTO ga kiradi 2026-03-18 | “Zaxira tekshiruvchisi kerak” | “@docs-lead 2026-03-05 gacha zaxira nusxasini aniqlashga yordam beradi” |

Shuningdek, qisqa hikoyani yozib oling:

- **SLA prognozi:** Har qanday hujjat besh ish kunilik SLA va
  yumshatish (almashtirish ustuvorligi, zaxira sotuvchini jalb qilish va boshqalar).
- **Chipta va aktivlar salomatligi:** Eng yaxshi yozuvlar
  `docs/source/sdk/android/i18n_requests/` va skrinshotlar/aktivlar mavjudligi
  tarjimonlar uchun tayyor.

### Mahalliylashtirish Xodimlarni ko'rib chiqish jurnali

- **Daqiqa:** Xodimlar jadvalini + hikoyani nusxalang
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md` (barcha mahalliy tillar
  Xuddi shu katalog ostidagi mahalliylashtirilgan fayllar orqali inglizcha daqiqalar). Kirishni bog'lang
  kun tartibiga qaytish (`docs/source/docs_devrel/monthly_sync_agenda.md`) shunday
  boshqaruv dalillarni kuzatishi mumkin.
- **i18n rejasi:** Xodimlar bo'yicha qarorlar jurnali va tarjima holati jadvalini yangilang
  Uchrashuvdan so'ng darhol `docs/source/sdk/android/i18n_plan.md` da.
- **Holat:** Xodimlar bo'yicha qarorlar yo'l xaritasi darvozalariga ta'sir qilganda, qisqacha yozuv qo'shing
  `status.md` (Hujjatlar/DevRel bo'limi) daqiqali fayl va i18n rejasiga havola
  yangilash.

## Daqiqa shabloni

Ushbu skeletni `docs/source/docs_devrel/minutes/<yyyy-mm>.md` ga nusxalash:

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```Uchrashuvdan so'ng darhol PR orqali eslatmalarni nashr qiling va ularni `status.md` dan bog'lang
xavf yoki kadrlar bo'yicha qarorlarga murojaat qilganda.

## Kutish kutilmalari

1. **Qo'yilgan daqiqalar:** 24 soat ichida (`docs/source/docs_devrel/minutes/`).
2. **i18n rejasi yangilandi:** kadrlar jurnali va tarjimalar jadvalini sozlang
   yangi majburiyatlar yoki kuchayishlarni aks ettiradi.
3. **Status.md yozuvi:** yo‘l xaritasini saqlab qolish uchun har qanday yuqori xavfli qarorlarni umumlashtiring
   sinxronlashda.
4. **Eskalatsiyalar topshirildi:** ko‘rib chiqish ko‘paytirishni talab qilganda, yarating/yangilang
   tegishli chipta (masalan, mahsulot operatsiyalari, moliyani tasdiqlash, sotuvchini ishga tushirish)
   va uni daqiqalarda ham, i18n rejasida ham havola qiling.

Ushbu kun tartibiga rioya qilgan holda, yo'l xaritasi talabi mahalliylashtirishni o'z ichiga oladi
Docs/DevRel oylik sinxronlash tizimidagi kadrlar boʻyicha sharhlar tekshirilishi mumkin boʻlib qoladi
jamoalar har doim dalillarni qaerdan topishni bilishadi.