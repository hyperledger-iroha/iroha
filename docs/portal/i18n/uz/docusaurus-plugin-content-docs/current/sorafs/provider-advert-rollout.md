---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) dan moslashtirilgan.

# SoraFS Provayder reklamasini tarqatish rejasi

Ushbu reja ruxsat beruvchi provayder reklamalaridan kesishni muvofiqlashtiradi
ko'p manbali bo'lak uchun zarur bo'lgan to'liq boshqariladigan `ProviderAdvertV1` yuzasi
olish. U uchta etkazib berishga qaratilgan:

- **Operator qo'llanmasi.** Saqlash provayderlari bosqichma-bosqich amallarni bajarishi kerak
  har bir darvoza burilishidan oldin.
- **Telemetriya qamrovi.** Observability va Ops foydalanadigan asboblar paneli va ogohlantirishlar
  tasdiqlash uchun tarmoq faqat mos reklamalarni qabul qiladi.
Chiqarish [SoraFS migratsiyasida SF-2b/2c bosqichlari bilan mos keladi.
yo'l xaritasi](./migration-roadmap) va qabul siyosatini qabul qiladi
[provayderni qabul qilish siyosati](./provider-admission-policy) allaqachon kiritilgan
ta'sir.

## Joriy talablar

SoraFS faqat boshqaruv bilan qoplangan `ProviderAdvertV1` foydali yuklarni qabul qiladi. The
qabul qilishda quyidagi talablar qo‘yiladi:

- `profile_id=sorafs.sf1@1.0.0` kanonik `profile_aliases` mavjud.
- `chunk_range_fetch` qobiliyatining foydali yuklari ko'p manba uchun kiritilishi kerak
  olish.
- `signature_strict=true` kengash imzolari bilan reklamaga ilova qilingan
  konvert.
- `allow_unknown_capabilities` faqat aniq GREASE mashqlari paytida ruxsat etiladi
  va jurnalga kiritilishi kerak.

## Operator nazorat ro'yxati

1. **Inventar reklamalari.** Har bir eʼlon qilingan eʼlonni roʻyxatlang va yozib oling:
   - Boshqaruvchi konvert yo'li (`defaults/nexus/sorafs_admission/...` yoki ishlab chiqarish ekvivalenti).
   - Reklama `profile_id` va `profile_aliases`.
   - Imkoniyatlar ro'yxati (kamida `torii_gateway` va `chunk_range_fetch` ni kuting).
   - `allow_unknown_capabilities` bayrog'i (sotuvchi tomonidan ajratilgan TLVlar mavjud bo'lganda talab qilinadi).
2. **Provayder vositalari yordamida qayta tiklang.**
   - Provayderingizning reklama nashriyotchisi bilan foydali yukni qayta yarating, bu esa:
     - `profile_id=sorafs.sf1@1.0.0`
     - Belgilangan `max_span` bilan `capability=chunk_range_fetch`
     - GREASE TLV mavjud bo'lganda `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` va `sorafs_fetch` orqali tasdiqlash; noma'lum haqida ogohlantirishlar
     qobiliyatlarni triyajlash kerak.
3. **Ko‘p manbali tayyorlikni tasdiqlang.**
   - `sorafs_fetch` ni `--provider-advert=<path>` bilan bajaring; CLI endi ishlamayapti
     `chunk_range_fetch` yo'q bo'lganda va e'tibor berilmagan noma'lum uchun ogohlantirishlarni chop etadi
     qobiliyatlar. JSON hisobotini yozib oling va uni operatsiyalar jurnallari bilan arxivlang.
4. **Bosqichni yangilash.**
   - `ProviderAdmissionRenewalV1` konvertlarini kamida 30 kun oldin yuboring
     amal qilish muddati. Yangilashda kanonik tutqich va imkoniyatlar to'plami saqlanib qolishi kerak;
     faqat ulush, so'nggi nuqtalar yoki metama'lumotlar o'zgarishi kerak.
5. **O'ziga qaram bo'lgan jamoalar bilan muloqot qiling.**
   - SDK egalari operatorlarga ogohlantiruvchi versiyalarni chiqarishlari kerak
     reklamalar rad etiladi.
   - DevRel har bir fazaga o'tishni e'lon qiladi; asboblar paneli havolalari va
     pastki mantiq chegarasi.
6. **Boshqaruv paneli va ogohlantirishlarni o‘rnating.**
   - Grafana eksportini import qiling va uni **SoraFS / Provayder ostida joylashtiring
     Rollout** asboblar paneli UID `sorafs-provider-admission` bilan.
   - Ogohlantirish qoidalari umumiy `sorafs-advert-rollout` ga ishora qilishiga ishonch hosil qiling
     sahnalashtirish va ishlab chiqarishda bildirishnoma kanali.

## Telemetriya va asboblar paneli

Quyidagi ko'rsatkichlar allaqachon `iroha_telemetry` orqali ochilgan:

- `torii_sorafs_admission_total{result,reason}` - qabul qilingan, rad etilgan hisoblar,
  va ogohlantirish natijalari. Sabablari orasida `missing_envelope`, `unknown_capability`,
  `stale` va `policy_violation`.

Grafana eksporti: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Faylni umumiy boshqaruv paneli omboriga import qiling (`observability/dashboards`)
va nashr qilishdan oldin faqat ma'lumotlar manbai UIDni yangilang.

Kengash Grafana jildida **SoraFS / Provider Rollout** ostida nashr etadi.
barqaror UID `sorafs-provider-admission`. Ogohlantirish qoidalari
`sorafs-admission-warn` (ogohlantirish) va `sorafs-admission-reject` (tanqidiy)
`sorafs-advert-rollout` bildirishnoma siyosatidan foydalanish uchun oldindan tuzilgan; sozlash
Agar maqsad ro'yxati tahrirlash o'rniga o'zgarsa, o'sha aloqa nuqtasi
asboblar paneli JSON.

Tavsiya etilgan Grafana panellari:

| Panel | So'rov | Eslatmalar |
|-------|-------|-------|
| **Qabul natijalari darajasi** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Qabul qilish va ogohlantirish va rad etishni tasavvur qilish uchun stek diagrammasi. Ogohlantirish > 0,05 * jami (ogohlantirish) yoki rad etish > 0 (tanqidiy) bo'lganda ogohlantirish. |
| **Ogohlantirish nisbati** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Peyjer chegarasini ta'minlaydigan bir qatorli vaqt seriyasi (5% ogohlantirish tezligi 15 daqiqa). |
| **Rad etish sabablari** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Drives runbook triage; yumshatish bosqichlariga havolalar qo'shing. |
| **Qarzni yangilash** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Provayderlarni yangilash muddatini o'tkazib yuborganligini ko'rsatadi; kashfiyot kesh jurnallari bilan o'zaro havola. |

Qo'lda boshqaruv paneli uchun CLI artefaktlari:

- `sorafs_fetch --provider-metrics-out` `failures`, `successes` va
  Har bir provayder uchun `disabled` hisoblagichlari. Kuzatuv uchun maxsus boshqaruv panellariga import qiling
  ishlab chiqarish provayderlarini almashtirishdan oldin orkestr quruq ishlaydi.
- JSON hisobotining `chunk_retry_rate` va `provider_failure_rate` maydonlari
  ko'pincha qabul qilishdan oldin bo'shatuvchi yoki eskirgan yuk belgilarini ajratib ko'rsatish
  rad etishlar.

### Grafana asboblar paneli tartibi

Observability maxsus kengashni nashr etadi - **SoraFS Provayderga kirish
Rollout** (`sorafs-provider-admission`) — **SoraFS ostida / Provayder Rollout**
quyidagi kanonik panel identifikatorlari bilan:

- Panel 1 — *Qabul natijalari darajasi* (to‘plangan maydon, “ops/min” birligi).
- Panel 2 — *Ogohlantirish nisbati* (bitta seriya), ifodani chiqaradi
  `sum(stavka(torii_sorafs_admission_jami{natija="ogohlantirish"}[5m])) /
   summa(stavka(torii_sorafs_qabul_umumiy[5m]))`.
- Panel 3 — *Rad etish sabablari* (vaqt seriyasi `reason` tomonidan guruhlangan), tartiblangan
  `rate(...[5m])`.
- Panel 4 — *Qarzni yangilash* (stat), yuqoridagi jadvaldagi so'rovni aks ettiruvchi va
  migratsiya kitobidan olingan reklamani yangilash muddatlari bilan izohlanadi.

JSON skeletini infratuzilma asboblar panelidagi repo-dan nusxa ko'chiring (yoki yarating).
`observability/dashboards/sorafs_provider_admission.json`, keyin faqat yangilang
ma'lumotlar manbai UID; panel identifikatorlari va ogohlantirish qoidalariga runbooks havola qilinadi
quyida, shuning uchun ushbu hujjatlarni qayta ko'rib chiqmasdan ularni qayta raqamlashdan saqlaning.

Qulaylik uchun ombor endi ma'lumot paneli ta'rifini jo'natadi
`docs/source/grafana_sorafs_admission.json`; agar bo'lsa, uni Grafana jildingizga nusxalash
mahalliy test uchun boshlang'ich nuqtasi kerak.

### Prometheus ogohlantirish qoidalari

Quyidagi qoidalar guruhini `observability/prometheus/sorafs_admission.rules.yml` ga qo'shing
(agar bu birinchi SoraFS qoida guruhi bo'lsa, faylni yarating) va uni qo'shing
sizning Prometheus konfiguratsiyasi. `<pagerduty>` ni haqiqiy marshrutlash bilan almashtiring
qo'ng'iroq bo'yicha aylanishingiz uchun yorliq.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml` ni ishga tushiring
sintaksisi `promtool check rules` o'tishini ta'minlash uchun o'zgarishlarni surishdan oldin.

## Qabul natijalari

- `chunk_range_fetch` qobiliyati etishmayapti → `reason="missing_capability"` bilan rad etish.
- `allow_unknown_capabilities=true` holda noma'lum qobiliyatli TLVlar → bilan rad etish
  `reason="unknown_capability"`.
- `signature_strict=false` → rad etish (izolyatsiya qilingan diagnostika uchun ajratilgan).
- Muddati tugagan `refresh_deadline` → rad etish.

## Aloqa va hodisalarni boshqarish

- **Haftalik status xabari.** DevRel qabul haqida qisqacha ma'lumotni tarqatadi
  ko'rsatkichlar, muhim ogohlantirishlar va yaqinlashib kelayotgan muddatlar.
- **Hodisaga javob.** Agar `reject` yong'in haqida ogohlantirsa, chaqiruv bo'yicha muhandislar:
  1. Torii kashfiyoti (`/v2/sorafs/providers`) orqali haqoratomuz reklamani oling.
  2. Provayder kanalida reklama tekshiruvini qayta ishga tushiring va shu bilan solishtiring
     Xatoni takrorlash uchun `/v2/sorafs/providers`.
  3. Keyingi yangilashdan oldin reklamani aylantirish uchun provayder bilan kelishib oling
     muddat.
- **O'zgarish muzlaydi.** R1/R2 davomida hech qanday qobiliyat sxemasi erni o'zgartirmaydi
  ishlab chiqarish komissiyasi imzo chekadi; GREASE sinovlari davomida rejalashtirilgan bo'lishi kerak
  haftalik parvarishlash oynasi va migratsiya kitobiga kirgan.

## Ma'lumotnomalar

- [SoraFS tugun/mijoz protokoli](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provayderga kirish siyosati](./provider-admission-policy)
- [Migratsiya yoʻl xaritasi](./migration-roadmap)
- [Provayder reklamasi uchun koʻp manbali kengaytmalar](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)