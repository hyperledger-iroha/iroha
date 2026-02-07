---
lang: uz
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS KPI dashboard
description: Live Grafana panels that aggregate registrar, freeze, and revenue metrics for SN-8a.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sora Name Service KPI asboblar paneli

KPI boshqaruv paneli boshqaruvchilar, vasiylar va tartibga soluvchilarga yagona joy beradi
oylik ilova kadansidan oldin qabul qilish, xato va daromad signallarini ko'rib chiqing
(SN-8a). Grafana ta'rifi quyidagi manzilda saqlanadi
`dashboards/grafana/sns_suffix_analytics.json` va portal xuddi shunday aks etadi
o'rnatilgan iframe orqali panellar, shuning uchun tajriba ichki Grafana bilan mos keladi
misol.

## Filtrlar va ma'lumotlar manbalari

- **Suffiks filtri** - `sns_registrar_status_total{suffix}` so'rovlarini shunday boshqaradi
  `.sora`, `.nexus` va `.dao` mustaqil ravishda tekshirilishi mumkin.
- **Ommaviy chiqarish filtri** - `sns_bulk_release_payment_*` ko'rsatkichlarini qamrab oladi
  moliya ma'lum bir registrator manifestini yarashtirishi mumkin.
- **Metrics** – Torii (`sns_registrar_status_total`,
  `torii_request_duration_seconds`), qo'riqchi CLI (`guardian_freeze_active`),
  `sns_governance_activation_total` va ommaviy ishga tushirish yordamchi koʻrsatkichlari.

## Panellar

1. **Ro‘yxatdan o‘tishlar (oxirgi 24 soat)** – ro‘yxatga oluvchi tomonidan muvaffaqiyatli o‘tkazilgan tadbirlar soni
   tanlangan qo'shimcha.
2. **Boshqaruvni faollashtirish (30d)** – tashkilot tomonidan qayd etilgan ustav/qo‘shimchalar bo‘yicha takliflar
   CLI.
3. **Registratorning o‘tkazuvchanligi** – ro‘yxatga oluvchining muvaffaqiyatli harakatlarining har bir qo‘shimchasi darajasi.
4. **Ro‘yxatga oluvchining xatolik rejimlari** – xatolik belgilarining 5 daqiqalik tezligi
   `sns_registrar_status_total` hisoblagichlari.
5. **Guardian muzlatish oynalari** – `guardian_freeze_active` bo‘lgan jonli selektorlar
   ochiq muzlatish chiptasi haqida xabar beradi.
6. **Aktivlar bo‘yicha sof to‘lov birliklari** – hisobot bergan jami
   Har bir aktiv uchun `sns_bulk_release_payment_net_units`.
7. **Har bir qoʻshimcha uchun ommaviy soʻrovlar** – har bir qoʻshimcha identifikatoriga manifest hajmlari.
8. **So'rov bo'yicha aniq birliklar** - Chiqarishdan olingan ARPU uslubidagi hisoblash
   ko'rsatkichlar.

## Oylik KPI tekshiruv ro'yxati

Moliyaviy rahbar har oyning birinchi seshanbasida takroriy ko'rib chiqadi:

1. Portalning **Analitika → SNS KPI** sahifasini oching (yoki Grafana asboblar paneli `sns-kpis`).
2. Registratorning o'tkazish qobiliyati va daromad jadvallarining PDF/CSV eksportini suratga oling.
3. SLA buzilishi uchun qo'shimchalarni solishtiring (xato tezligi o'sishi, muzlatilgan selektorlar >72 soat,
   ARPU deltalari >10%).
4. Jurnal xulosalari + tegishli ilova yozuvidagi harakatlar elementlari
   `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. Eksport qilingan asboblar paneli artefaktlarini qo'shimcha majburiyatga biriktiring va ularni ulang
   kengash kun tartibi.

Ko'rib chiqish SLA buzilishini aniqlasa, zararlanganlar uchun PagerDuty hodisasini yozing
egasi (ro'yxatga olish organining navbatchi menejeri, chaqiruv bo'yicha vasiy yoki boshqaruvchi dastur rahbari) va
ilova jurnalida tuzatishni kuzatib boring.