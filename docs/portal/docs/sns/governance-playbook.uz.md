---
lang: uz
direction: ltr
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa4560c51d066ce5c63581dd102aef4e70786d140790fb157323df2553b15f4b
source_last_modified: "2026-01-28T17:11:30.700959+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: boshqaruv-o'yin kitobi
sarlavha: Sora Name Service Governance Playbook
sidebar_label: Boshqaruv kitobi
Tavsif: Kengash, vasiy, boshqaruvchi va ro'yxatga oluvchining ish jarayonlari uchun SN-1/SN-6 tomonidan havola qilingan.
---

::: Eslatma Kanonik manba
Ushbu sahifa `docs/source/sns/governance_playbook.md`-ni aks ettiradi va hozir sifatida xizmat qiladi
kanonik portal nusxasi. Manba fayli tarjima PRlari uchun saqlanib qoladi.
:::

# Sora Name Service Governance Playbook (SN-6)

**Holat:** 2026-03-24-da tuzilgan - SN-1/SN-6 tayyorligi uchun jonli ma'lumotnoma  
**Yo‘l xaritasi havolalari:** SN-6 “Muvofiqlik va nizolarni hal qilish”, SN-7 “Resolver & Gateway Sync”, ADDR-1/ADDR-5 manzil siyosati  
**Talablar:** [`registry-schema.md`](./registry-schema.md) da registr sxemasi, [`registrar-api.md`](./registrar-api.md) da registrator API shartnomasi, UX boʻyicha yoʻriqnoma manzili [`address-display-guidelines.md`](./address-display-guidelines.md) va hisob tuzilmasi qoidalari [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Ushbu kitobda Sora Name Service (SNS) boshqaruv organlari qanday qabul qilinishini tasvirlaydi
nizomlar, ro'yxatga olishlarni tasdiqlash, nizolarni kuchaytirish va bu hal qiluvchi va isbotlash
shlyuz holatlari sinxron bo'lib qoladi. Bu yo'l xaritasi talabini bajaradi
`sns governance ...` CLI, Norito manifestlari va audit artefaktlari bittadan foydalanadi
N1 dan oldin operatorga tegishli ma'lumotnoma (ommaviy ishga tushirish).

## 1. Qamrov va auditoriya

Hujjatning maqsadi:

- Ustavlar, qo'shimchalar siyosati va nizolar bo'yicha ovoz beradigan Boshqaruv Kengashi a'zolari
  natijalar.
- Favqulodda vaziyatni muzlatish va bekor qilishni ko'rib chiqadigan vasiylik kengashi a'zolari.
- Ro'yxatga oluvchi navbatlarni boshqaradigan, auktsionlarni tasdiqlaydigan va boshqaruvchi qo'shimcha styuardlar
  daromad taqsimoti.
- SoraDNS tarqalishi, GAR yangilanishi uchun mas'ul bo'lgan resolver/shlyuz operatorlari,
  va telemetriya to'siqlari.
- Muvofiqlik, g'aznachilik va qo'llab-quvvatlash guruhlari har bir narsani ko'rsatishi kerak
  boshqaruv harakati tekshiriladigan Norito artefaktlarini qoldirdi.

U yopiq beta (N0), ommaviy ishga tushirish (N1) va kengaytirish (N2) bosqichlarini qamrab oladi.
Har bir ish jarayonini kerakli dalillar bilan bog'lash orqali `roadmap.md` da sanab o'tilgan,
asboblar paneli va eskalatsiya yo'llari.

## 2. Rollar va kontaktlar xaritasi

| Rol | Asosiy majburiyatlar | Birlamchi artefaktlar va telemetriya | Eskalatsiya |
|------|----------------------|----------------------------------------|------------|
| Boshqaruv Kengashi | Nizomlar, qoʻshimchalar siyosati, nizolar boʻyicha qarorlar va boshqaruvchi rotatsiyalarni ishlab chiqish va ratifikatsiya qilish. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, `sns governance charter submit` orqali saqlanadigan kengash byulletenlari. | Kengash raisi + boshqaruv hujjatini kuzatuvchisi. |
| Vasiylik kengashi | Yumshoq/qattiq muzlashlar, favqulodda vaziyatlar qoidalari va 72 soatlik sharhlar chiqaring. | `sns governance freeze` tomonidan chiqarilgan qo'riqchi chiptalari, `artifacts/sns/guardian/*` ostida qayd etilgan manifestlarni bekor qilish. | Qo'riqchining chaqiruv bo'yicha aylanishi (≤15min ACK). |
| Styuardlar qo'shimchasi | Ro'yxatga oluvchi navbatlarini, auktsionlarni, narxlash darajalarini va mijozlar bilan muloqot qilish; muvofiqligini tan olish. | `SuffixPolicyV1`-dagi boshqaruvchi siyosatlari, narxlar bo'yicha ma'lumotnomalar, boshqaruvchining tasdiqnomalari tartibga soluvchi eslatmalar yonida saqlanadi. | Styuard dasturi rahbari + qo'shimchasiga xos PagerDuty. |
| Registrator & Billing Operations | `/v1/sns/*` so'nggi nuqtalarini boshqaring, to'lovlarni yarashtiring, telemetriyani chiqaring va CLI oniy suratlarini saqlang. | Registrator API ([`registrar-api.md`](./registrar-api.md)), `sns_registrar_status_total` koʻrsatkichlari, `artifacts/sns/payments/*` ostida arxivlangan toʻlov dalillari. | Ro'yxatga oluvchi navbatchi menejeri va g'aznachilik aloqasi. |
| Resolver & Gateway operatorlari | SoraDNS, GAR va shlyuz holatini ro'yxatga oluvchi hodisalariga mos holda saqlang; oqim shaffofligi ko'rsatkichlari. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE on-call + Gateway ops ko'prigi. |
| G'aznachilik va moliya | 70/30 daromad bo'linishi, yo'llanmani ajratish, soliq/g'aznachilik arizalari va SLA sertifikatlarini qo'llang. | Daromadni hisoblash manifestlari, Stripe/xazina eksporti, `docs/source/sns/regulatory/` ostida choraklik KPI ilovalari. | Moliya nazoratchisi + muvofiqlik xodimi. |
| Muvofiqlik va tartibga solish bilan aloqa | Global majburiyatlarni kuzatib boring (Yevropa Ittifoqi DSA va boshqalar), KPI shartnomalarini yangilang va fayllarni oshkor qiling. | `docs/source/sns/regulatory/`dagi me'yoriy eslatmalar, ma'lumot to'plami, stol usti mashqlari uchun `ops/drill-log.md` yozuvlari. | Muvofiqlik dasturi rahbari. |
| Qo'llab-quvvatlash / SRE On-chaqiruv | Hodisalarni (to'qnashuvlar, hisob-kitoblar siljishi, hal qiluvchi uzilishlar) boshqaring, mijozlar xabarlarini muvofiqlashtiring va o'z ish kitoblariga ega bo'ling. | Hodisa shablonlari, `ops/drill-log.md`, bosqichma-bosqich laboratoriya dalillari, `incident/` ostida arxivlangan Slack/harbiy xona transkriptlari. | SNS qo'ng'iroq bo'yicha aylanish + SRE boshqaruvi. |

## 3. Kanonik artefaktlar va ma'lumotlar manbalari

| Artefakt | Manzil | Maqsad |
|----------|----------|---------|
| Nizom + KPI qo'shimchalari | `docs/source/sns/governance_addenda/` | Versiya tomonidan boshqariladigan imzolangan nizomlar, KPI shartnomalari va CLI ovozlari bilan havola qilingan boshqaruv qarorlari. |
| Ro'yxatga olish sxemasi | [`registry-schema.md`](./registry-schema.md) | Kanonik Norito tuzilmalari (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Registrator shartnomasi | [`registrar-api.md`](./registrar-api.md) | REST/gRPC yuklamalari, `sns_registrar_status_total` ko'rsatkichlari va boshqaruv hooki kutilmalari. |
| Manzil UX qo'llanma | [`address-display-guidelines.md`](./address-display-guidelines.md) | Hamyonlar/tadqiqotchilar tomonidan aks ettirilgan kanonik I105 (afzal) + siqilgan (`sora`, ikkinchi eng yaxshi) renderlar. |
| SoraDNS / GAR hujjatlari | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Deterministik xost hosilasi, shaffoflik tayler ish jarayoni va ogohlantirish qoidalari. |
| Normativ eslatmalar | `docs/source/sns/regulatory/` | Yurisdiksiyaga oid qabul eslatmalari (masalan, EI DSA), boshqaruvchining roziligi, shablon ilovalari. |
| Burg'ulash jurnali | `ops/drill-log.md` | Fazadan chiqishdan oldin zarur bo'lgan tartibsizlik va IR mashqlarini yozib olish. |
| Artefakt saqlash | `artifacts/sns/` | `sns governance ...` tomonidan ishlab chiqarilgan toʻlov hujjatlari, vasiylik chiptalari, hal qiluvchi farqlari, KPI eksporti va imzolangan CLI chiqishi. |

Barcha boshqaruv harakatlari yuqoridagi jadvalda kamida bitta artefaktga havola qilishi kerak
Shunday qilib, auditorlar 24 soat ichida qaror izlarini qayta qurishlari mumkin.

## 4. Hayotiy tsikl o'yin kitoblari

### 4.1 Nizom va boshqaruvchi harakatlari

| Qadam | Egasi | CLI / Dalil | Eslatmalar |
|------|-------|----------------|-------|
| Qo'shimcha loyihasi va KPI deltalari | Kengash ma'ruzachisi + boshqaruvchi yetakchi | Markdown shabloni `docs/source/sns/governance_addenda/YY/` | ostida saqlanadi KPI kelishuv identifikatorlari, telemetriya ilgaklari va faollashtirish shartlarini qo'shing. |
| Taklif yuborish | Kengash raisi | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` ishlab chiqaradi) | CLI `artifacts/sns/governance/<id>/charter_motion.json` ostida saqlangan Norito manifestini chiqaradi. |
| Ovoz berish va vasiyni tasdiqlash | Kengash + vasiylar | `sns governance ballot cast --proposal <id>` va `sns governance guardian-ack --proposal <id>` | Xeshlangan daqiqalar va kvorum dalillarini ilova qiling. |
| Styuardni qabul qilish | Styuard dasturi | `sns governance steward-ack --proposal <id> --signature <file>` | Qo'shimchalar siyosati o'zgarishidan oldin talab qilinadi; `artifacts/sns/governance/<id>/steward_ack.json` ostida rekord konvert. |
| Faollashtirish | Registrator operatsiyalari | `SuffixPolicyV1` ni yangilang, registrator keshlarini yangilang, `status.md` da eslatmani chop eting. | Faollashtirish vaqti `sns_governance_activation_total` tizimiga kirdi. |
| Audit jurnali | Muvofiqlik | Yozuvni `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` ga qo'shing va agar stol usti bajarilgan bo'lsa, matkap jurnali. | Telemetriya asboblar paneli va siyosat farqlariga havolalarni qo'shing. |

### 4.2 Ro'yxatdan o'tish, auktsion va narxlarni tasdiqlash

1. **Preflight:** Narxlar darajasini tasdiqlash uchun registrator `SuffixPolicyV1` so‘rovi,
   mavjud shartlar va imtiyoz/to'lov oynalari. Narxlar varaqlarini sinxronlashtiring
   3/4/5/6–9/10+ darajali jadval (asosiy daraja + qo'shimcha koeffitsientlar) hujjatlashtirilgan.
   yo'l xaritasi.
2. **Muhrlangan auktsionlar:** Premium hovuzlar uchun 72 soatlik majburiyatni bajaring / 24 soat oshkor qiling
   `sns governance auction commit` / `... reveal` orqali tsikl. Qarorni e'lon qiling
   ro'yxat (faqat xeshlar) `artifacts/sns/auctions/<name>/commit.json` ostida shunday
   Auditorlar tasodifiylikni tekshirishlari mumkin.
3. **Toʻlovni tekshirish:** Roʻyxatga oluvchilar `PaymentProofV1` ni tasdiqlaydilar.
   g'aznachilik bo'linishlari (70% xazina / ≤10% yo'llanma bilan 30% boshqaruvchi).
   Norito JSON-ni `artifacts/sns/payments/<tx>.json` ostida saqlang va uni ulang
   registratorning javobi (`RevenueAccrualEventV1`).
4. **Boshqaruv ilgagi:** Premium/qo‘riqlanadigan nomlar uchun `GovernanceHookV1` ilovasini biriktiring.
   kengash taklifi identifikatorlari va boshqaruvchi imzolariga havola. Kancalar etishmayotgan natija
   `sns_err_governance_missing` da.
5. **Faollashtirish + hal qiluvchi bilan sinxronlash:** Torii registr hodisasini chiqargandan so‘ng, ishga tushiriladi
   tarqatuvchi yangi GAR/zonasi holatini tasdiqlash uchun hal qiluvchi shaffoflik tailer
   (§4.5 ga qarang).
6. **Mijoz haqida ma’lumot:** Mijozlarga qarashli daftarni yangilash (hamyon/tadqiqotchi)
   [`address-display-guidelines.md`](./address-display-guidelines.md) da umumiy moslamalar orqali, I105 va
   siqilgan renderlar nusxa ko'chirish/QR ko'rsatmalariga mos keladi.

### 4.3 Yangilash, hisob-kitob va g'aznachilikni solishtirish- **Yangilash ish jarayoni:** Ro‘yxatga oluvchilar 30 kunlik imtiyoz + 60 kunlik to‘lovni amalga oshiradilar
  `SuffixPolicyV1` da ko'rsatilgan oynalar. 60 kundan so'ng Gollandiya ketma-ketligini qayta ochadi
  (7 kun, 10 × to'lov kuniga 15% pasaymoqda) `sns boshqaruvi orqali avtomatik ravishda ishga tushadi
  qayta oching`.
- **Daromadning bo'linishi:** Har bir yangilash yoki o'tkazish a hosil qiladi
  `RevenueAccrualEventV1`. G'aznachilik eksporti (CSV/Parquet) bilan mos kelishi kerak
  har kuni bu hodisalar; dalillarni `artifacts/sns/treasury/<date>.json` ga ilova qiling.
- **Yo'llanmalar soni:** Ixtiyoriy yo'naltirish foizlari har bir qo'shimcha bo'yicha kuzatiladi
  boshqaruvchi siyosatiga `referral_share` qo'shish orqali. Ro'yxatga oluvchilar finalni chiqaradilar
  to'lovni tasdiqlovchi hujjatning yonida yo'llanmani ajratish va saqlash.
- **Hisobot kadensi:** Moliya oylik KPI ilovalarini joylashtiradi (roʻyxatga olishlar,
  yangilanishlar, ARPU, nizo/obligatsiyalardan foydalanish) ostida
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Boshqaruv panellari tortib olinishi kerak
  bir xil eksport qilingan jadvallar, shuning uchun Grafana raqamlari daftar dalillariga mos keladi.
- **Oylik KPI tekshiruvi:** Birinchi seshanba kungi nazorat punkti moliya rahbarini birlashtiradi,
  navbatchi boshqaruvchi va dastur PM. [SNS KPI asboblar panelini] oching (./kpi-dashboard.md)
  (`sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json` portalining o'rnatilishi),
  ro'yxatga oluvchining o'tkazish qobiliyatini + daromad jadvallarini, ilovadagi log deltalarini eksport qilish,
  va artefaktlarni eslatmaga biriktiring. Tekshiruv aniqlansa, hodisani tetiklang
  SLA buzilishi (oynalarni muzlatish > 72 soat, registrator xatosi, ARPU drifti).

### 4.4 Muzlatishlar, nizolar va shikoyatlar

| Bosqich | Egasi | Harakat va dalillar | SLA |
|-------|-------|-------------------|-----|
| Yumshoq muzlatish so'rovi | Styuard / qo'llab-quvvatlash | `SNS-DF-<id>` chiptasini toʻlov dalillari, nizo boʻyicha maʼlumotnoma va taʼsirlangan selektor(lar). | Qabul qilingandan ≤4 soat. |
| Qo'riqchi chiptasi | Himoya kengashi | `sns governance freeze --selector <I105> --reason <text> --until <ts>` imzolangan `GuardianFreezeTicketV1` ishlab chiqaradi. JSON chiptasini `artifacts/sns/guardian/<id>.json` ostida saqlang. | ≤30min ACK, ≤2s bajarish. |
| Kengash ratifikatsiyasi | Boshqaruv kengashi | Muzlatishlarni ma'qullang yoki rad eting, vasiylik chiptasiga hujjat qarori havolasi va nizolar to'g'risidagi nizomlar dayjesti. | Kengashning navbatdagi sessiyasi yoki asenkron ovoz berish. |
| Arbitraj paneli | Muvofiqlik + boshqaruvchi | `sns governance dispute ballot` orqali yuborilgan xeshlangan byulletenlar bilan 7 nafar hakamlar hay'atini chaqiring (har bir yo'l xaritasi bo'yicha). Voqea paketiga anonim ovoz kvitansiyalarini biriktiring. | Hukm obligatsiya depozitidan keyin ≤7 kun. |
| Apellyatsiya | Qo'riqchi + kengash | Apellyatsiya rishtalarini ikki baravar oshiradi va hakamlar hay'ati jarayonini takrorlaydi; rekord Norito manifest `DisputeAppealV1` va havola asosiy chipta. | ≤10 kun. |
| Muzdan tushirish va tuzatish | Registrator + hal qiluvchi operatsiyalar | `sns governance unfreeze --selector <I105> --ticket <id>` ni ishga tushiring, registrator holatini yangilang va GAR/resolver farqlarini tarqating. | Hukm chiqarilgandan so'ng darhol. |

Favqulodda vaziyatlar qoidalari (qo‘riqchi tomonidan qo‘zg‘atilgan muzlashlar ≤72 soat) bir xil oqim bo‘ylab amal qiladi, lekin
retroaktiv kengash ko'rib chiqish va ostida shaffoflik eslatma talab
`docs/source/sns/regulatory/`.

### 4.5 Resolver va shlyuzning tarqalishi

1. **Voqealar kancasi:** Har bir reestr hodisasi rezolyutsiya hodisasi oqimiga chiqadi
   (`tools/soradns-resolver` SSE). Resolver ops obuna bo'ling va farqlarni yozib oling
   shaffoflik ishlab chiqaruvchisi (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **GAR shablonini yangilash:** Gatewaylar havola qilingan GAR shablonlarini yangilashi kerak
   `canonical_gateway_suffix()` va `host_pattern` ro'yxatini qayta imzolang. Do'kon farqlari
   `artifacts/sns/gar/<date>.patch` da.
3. **Zonefile nashri:** Bu yerda tasvirlangan zona fayl skeletidan foydalaning
   `roadmap.md` (ism, ttl, cid, isbot) va uni Torii/SoraFS ga suring. ni arxivlash
   `artifacts/sns/zonefiles/<name>/<version>.json` ostida Norito JSON.
4. **Oshkoralikni tekshirish:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml` dasturini ishga tushiring
   ogohlantirishlar yashil qolishi uchun. Prometheus matn chiqishini ilovaga ulang
   haftalik shaffoflik hisoboti.
5. **Gateway auditi:** `Sora-*` sarlavha namunalarini yozib oling (kesh siyosati, CSP, GAR)
   digest) va ularni boshqaruv jurnaliga biriktiring, shunda operatorlar buni isbotlashlari mumkin
   shlyuz yangi nomga mo'ljallangan to'siqlar bilan xizmat qildi.

## 5. Telemetriya va hisobot

| Signal | Manba | Tavsif / Harakat |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Torii registrator ishlovchilar | Ro'yxatga olish, yangilash, muzlatish, o'tkazish uchun muvaffaqiyat/xato hisoblagichi; `result="error"` har bir qo'shimchaga ko'tarilganda ogohlantiradi. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii ko'rsatkichlari | API ishlov beruvchilari uchun kechikish SLO'lari; `torii_norito_rpc_observability.json` dan qurilgan ozuqa asboblar paneli. |
| `soradns_bundle_proof_age_seconds` & `soradns_bundle_cid_drift_total` | Resolver shaffofligi tailer | Eskirgan dalillarni yoki GAR driftini aniqlang; `dashboards/alerts/soradns_transparency_rules.yml` da belgilangan himoya panjaralari. |
| `sns_governance_activation_total` | Boshqaruv CLI | Nizom/qo'shimcha faollashtirilganda hisoblagich ko'paytiriladi; kengash qarorlari va e'lon qilingan qo'shimchalarni kelishish uchun ishlatiladi. |
| `guardian_freeze_active` o'lchagich | Guardian CLI | Har bir selektor uchun yumshoq/qattiq muzlatish oynalarini kuzatib boradi; Agar qiymat e'lon qilingan SLA chegarasidan tashqarida `1` qolsa, SRE sahifasi. |
| KPI ilovasining boshqaruv paneli | Moliya / Hujjatlar | Normativ eslatmalar bilan birga chop etiladigan oylik to'plamlar; portal ularni [SNS KPI asboblar paneli](./kpi-dashboard.md) orqali joylashtiradi, shuning uchun boshqaruvchilar va regulyatorlar bir xil Grafana ko'rinishiga kirishlari mumkin. |

## 6. Dalillar va auditga qo'yiladigan talablar

| Harakat | Arxiv uchun dalillar | Saqlash |
|--------|--------------------|---------|
| Nizom / siyosat o'zgarishi | Imzolangan Norito manifest, CLI transkripti, KPI farqi, boshqaruvchining tasdiqlanishi. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Ro'yxatdan o'tish / yangilash | `RegisterNameRequestV1` foydali yuk, `RevenueAccrualEventV1`, to'lov isboti. | `artifacts/sns/payments/<tx>.json`, registrator API jurnallari. |
| Auktsion | Manifestlarni ko'rsatish/oshkor qilish, tasodifiylik urug'i, g'olibni hisoblash jadvali. | `artifacts/sns/auctions/<name>/`. |
| Muzlatish / muzlatish | Qo'riqchi chiptasi, kengash ovozi xeshi, hodisalar jurnali URL manzili, mijoz xabarlari shablonlari. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Rezolverning tarqalishi | Zonefile/GAR diff, ishlab chiqaruvchi JSONL parchasi, Prometheus surati. | `artifacts/sns/resolver/<date>/` + shaffoflik hisobotlari. |
| Normativ qabul qilish | Qabul qilish eslatmasi, oxirgi muddat kuzatuvchisi, boshqaruvchining tasdiqlanishi, KPI o'zgarishi xulosasi. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Faza darvozasi nazorat ro'yxati

| Bosqich | Chiqish mezonlari | Dalillar to'plami |
|-------|---------------|-----------------|
| N0 — Yopiq beta | SN-1/SN-2 ro'yxatga olish sxemasi, qo'lda ro'yxatga oluvchi CLI, vasiylik mashqlari to'liq. | Charter harakati + boshqaruvchi ACK, ro'yxatga oluvchi quruq ish jurnallari, hal qiluvchi shaffoflik hisoboti, `ops/drill-log.md` da burg'ulash yozuvi. |
| N1 — Ommaviy ishga tushirish | Auktsionlar + qat'iy narxlari darajalari `.sora`/`.nexus`, o'z-o'ziga xizmat ko'rsatish registratori, hal qiluvchi avtomatik sinxronlash, hisob-kitoblar paneli uchun ishlaydi. | Narxlar varaqasi farqi, registrator CI natijalari, toʻlov/KPI ilovasi, oshkoralik ishlab chiqaruvchisi chiqishi, voqea-hodisalar boʻyicha takroriy eslatmalar. |
| N2 — Kengaytirish | `.dao`, sotuvchi API'lari, nizolar portali, boshqaruvchi ko'rsatkichlar kartalari, tahlil asboblar paneli. | Portal skrinshotlari, bahsli SLA koʻrsatkichlari, styuard koʻrsatkichlari kartasi eksporti, sotuvchi siyosatlariga havola qiluvchi yangilangan boshqaruv ustavi. |

Fazali chiqishlar qayd etilgan stol usti mashqlarini talab qiladi (ro'yxatdan o'tish baxtli yo'l, muzlatish,
rezolyutsiyaning uzilishi) `ops/drill-log.md` ga biriktirilgan artefaktlar bilan.

## 8. Hodisaga javob berish va kuchayishi

| Trigger | Jiddiylik | Darhol egasi | Majburiy harakatlar |
|---------|----------|-----------------|-------------------|
| Resolver/GAR drift yoki eskirgan dalillar | Sev1 | Resolver SRE + vasiylik kengashi | Qo'ng'iroq paytida sahifani hal qiluvchi, ishlab chiqaruvchining ma'lumotlarini yozib oling, ta'sirlangan nomlarni muzlatib qo'yishni hal qiling, post holatini har 30 daqiqada yangilang. |
| Registratorning uzilishi, hisob-kitob xatosi yoki keng tarqalgan API xatolari | Sev1 | Registrator navbatchi menejeri | Yangi auktsionlarni to'xtating, qo'lda CLI rejimiga o'ting, boshqaruvchilar/g'aznachilikni xabardor qiling, voqea hujjatiga Torii jurnallarini biriktiring. |
| Yagona nomdagi kelishmovchilik, toʻlovning mos kelmasligi yoki mijozning eskalatsiyasi | Sev2 | Styuard + qo'llab-quvvatlash etakchi | To'lov dalillarini to'plang, yumshoq muzlatish kerakligini aniqlang, SLA doirasida so'rovchiga javob bering, nizo kuzatuvchisida natijalarni qayd qiling. |
| Muvofiqlik auditini aniqlash | Sev2 | Muvofiqlik aloqasi | Ta'mirlash rejasi loyihasi, `docs/source/sns/regulatory/` ostida fayl eslatmasi, kuzatuv kengashi sessiyasini rejalashtirish. |
| Mashq yoki mashq | Sev3 | Dastur PM | `ops/drill-log.md` dan skriptli stsenariyni bajaring, artefaktlarni arxivlang, yoʻl xaritasi vazifalari sifatida boʻshliqlarni belgilang. |

Barcha hodisalar egalik bilan `incident/YYYY-MM-DD-sns-<slug>.md` yaratishi kerak
jadvallar, buyruq jurnallari va bu davomida ishlab chiqarilgan dalillarga havolalar
o'yin kitobi.

## 9. Adabiyotlar

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR bo'limlari)

Charter matni, CLI sirtlari yoki telemetriya bo'lganda ushbu o'yin kitobini yangilab turing
shartnomalar o'zgaradi; `docs/source/sns/governance_playbook.md` havolasi bo'yicha yo'l xaritasi yozuvlari
har doim oxirgi tahrirga mos kelishi kerak.