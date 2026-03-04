---
lang: uz
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-12-29T18:16:35.174056+00:00"
translation_last_reviewed: 2026-02-07
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
---

# SNS ko'rsatkichlari va ishga tushirish to'plami

Yo‘l xaritasi **SN-8** bandi ikkita va’dani birlashtiradi:

1. Roʻyxatga olishlar, yangilanishlar, ARPU, nizolarni va
   `.sora`, `.nexus` va `.dao` uchun oynalarni muzlatish.
2. Roʻyxatga oluvchilar va styuardlar DNS, narxlar va maʼlumotlarni ulashlari uchun bortga joylashtirish toʻplamini yuboring.
   APIlar har qanday qo'shimcha jonli efirga o'tishdan oldin.

Bu sahifa manba versiyasini aks ettiradi
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
shuning uchun tashqi sharhlovchilar xuddi shu tartibni bajarishlari mumkin.

## 1. Metrik to'plam

### Grafana asboblar paneli va portalni o'rnatish

- `dashboards/grafana/sns_suffix_analytics.json`ni Grafana (yoki boshqa) ga import qiling
  analytics host) standart API orqali:

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Xuddi shu JSON ushbu portal sahifasining iframe-ga quvvat beradi (qarang: **SNS KPI asboblar paneli**).
  Har safar asboblar paneliga urilganda yuguring
  `npm run build && npm run serve-verified-preview` `docs/portal` ichida
  ikkala Grafana va o'rnatish sinxronlanganligini tasdiqlang.

### Panellar va dalillar

| Panel | Ko'rsatkichlar | Boshqaruv dalillari |
|-------|---------|---------------------|
| Ro'yxatga olish va yangilash | `sns_registrar_status_total` (muvaffaqiyat + yangilash hal qiluvchi yorliqlari) | Har bir qo'shimchaning o'tkazish qobiliyati + SLA kuzatuvi. |
| ARPU / aniq birliklar | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Moliya ro'yxatga oluvchi manifestlari bilan daromadga mos kelishi mumkin. |
| Munozaralar va muzlatishlar | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Faol muzlatishlar, arbitraj ritmi va vasiyning ish yukini ko'rsatadi. |
| SLA/xato stavkalari | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Mijozlarga ta'sir qilishdan oldin API regressiyalarini ta'kidlaydi. |
| Ommaviy manifest kuzatuvchisi | `sns_bulk_release_manifest_total`, `manifest_id` yorliqlari bilan toʻlov koʻrsatkichlari | CSV tomchilarini hisob-kitob chiptalariga ulaydi. |

Oylik KPI davomida Grafana (yoki o'rnatilgan iframe) dan PDF/CSVni eksport qiling
ko'rib chiqing va tegishli ilova yozuviga ilova qiling
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Styuardlar SHA-256 ni ham qo'lga olishadi
`docs/source/sns/reports/` ostida eksport qilingan to'plamning (masalan,
`steward_scorecard_2026q1.md`) shuning uchun auditlar dalillar yo'lini takrorlashi mumkin.

### Ilovani avtomatlashtirish

Qo'shimcha fayllarni to'g'ridan-to'g'ri boshqaruv paneli eksportidan yarating, shunda sharhlovchilar bir
izchil hazm qilish:

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- Yordamchi eksportni xeshlaydi, UID/teglar/panellar sonini oladi va yozadi
  `docs/source/sns/reports/.<suffix>/<cycle>.md` ostida Markdown ilovasi (qarang
  Ushbu hujjat bilan birga tuzilgan `.sora/2026-03` namunasi).
- `--dashboard-artifact` eksportni ko'chiradi
  `artifacts/sns/regulatory/<suffix>/<cycle>/` shuning uchun ilovada havola qilingan
  kanonik dalillar yo'li; faqat ishora qilish kerak bo'lganda `--dashboard-label` dan foydalaning
  tarmoqdan tashqari arxivda.
- `--regulatory-entry` boshqaruv eslatmasiga ishora qiladi. Yordamchi qo'shimchalar (yoki
  o'rnini bosadi) ilova yo'lini, asboblar panelini qayd qiluvchi `KPI Dashboard Annex` bloki
  artefakt, dayjest va vaqt tamg'asi, shuning uchun dalillar qayta ishlagandan keyin sinxron bo'lib qoladi.
- `--portal-entry` Docusaurus nusxasini saqlaydi (`docs/portal/docs/sns/regulatory/*.md`)
  ko'rib chiquvchilar alohida ilova xulosalarini qo'lda farqlashlari shart bo'lmasligi uchun tekislangan.
- Agar siz `--regulatory-entry`/`--portal-entry` ni o'tkazib yuborsangiz, yaratilgan faylni ilovaga biriktiring.
  eslatmalarni qo'lda va hali ham Grafana dan olingan PDF/CSV snapshotlarini yuklang.
- Takroriy eksport uchun qo'shimchalar/sikl juftlarini ro'yxatlang
  `docs/source/sns/regulatory/annex_jobs.json` va ishga tushiring
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Yordamchi har bir kirishda yuradi,
  asboblar paneli eksportidan nusxa ko'chiradi (standart `dashboards/grafana/sns_suffix_analytics.json`
  aniqlanmagan bo'lsa) va har bir me'yoriy hujjat ichidagi ilova blokini yangilaydi (va,
  mavjud bo'lganda, portal) bir o'tishda eslatma.
- Ish ro'yxati tartiblangan/deduced bo'lib qolishi, har bir eslatma mos keladigan `sns-annex` belgisi va ilova stubkasi mavjudligini isbotlash uchun `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (yoki `make check-sns-annex`) ni ishga tushiring. Yordamchi boshqaruv paketlarida ishlatiladigan mahalliy til/xesh xulosalari yoniga `artifacts/sns/annex_schedule_summary.json` ni yozadi.
Bu qo'lda nusxa ko'chirish/joylashtirish bosqichlarini olib tashlaydi va SN-8 ilova dalillarini doimiy ravishda saqlaydi
CIda qo'riqlash jadvali, marker va lokalizatsiya drifti.

## 2. O'rnatish to'plami komponentlari

### Suffiks simlari

- Ro'yxatga olish kitobi sxemasi + selektor qoidalari:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  va [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS skeleti yordamchisi:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  da olingan repetisiya oqimi bilan
  [shlyuz/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Har bir registrator ishga tushirilishi uchun ostida qisqa eslatma yozing
  `docs/source/sns/reports/` selektor namunalarini, GAR dalillarini va DNS xeshlarini umumlashtiradi.

### Narxlar jadvali

| Yorliq uzunligi | Asosiy to'lov (AQSh dollari ekviv) |
|-------------|--------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

Suffiks koeffitsientlari: `.sora` = 1,0×, `.nexus` = 0,8×, `.dao` = 1,3×.  
Muddat ko'paytiruvchilar: 2‑yil −5%, 5‑yil −12%; inoyat oynasi = 30 kun, sotib olish
= 60 kun (20% to'lov, min $5, maksimal $200). Muzokara qilingan og'ishlarni qayd qiling
registrator chiptasi.

### Premium auktsionlar va yangilanishlar

1. **Premium pul ** — muhrlangan taklifni topshirish/oshkor qilish (SN-3). Takliflarni kuzatib boring
   `sns_premium_commit_total` va manifestni ostida nashr eting
   `docs/source/sns/reports/`.
2. **Gollandiya qayta ochiladi** — imtiyoz + sotib olish muddati tugagandan so‘ng, 7 kunlik Gollandiya savdosini boshlang
   10 × da, bu kuniga 15% ni yemiradi. Yorliq `manifest_id` bilan namoyon bo'ladi, shuning uchun
   asboblar paneli yuzaki siljishi mumkin.
3. **Yangilanishlar** — monitor `sns_registrar_status_total{resolver="renewal"}` va
   avtoyangilash nazorat ro'yxatini yozib oling (bildirishnomalar, SLA, zaxira to'lov relslari)
   registrator chiptasi ichida.

### Ishlab chiquvchilar uchun API va avtomatlashtirish

- API shartnomalari: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Ommaviy yordamchi va CSV sxemasi:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Misol buyruq:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

Manifest identifikatorini (`--submission-log` chiqishi) KPI asboblar paneli filtriga kiriting
shuning uchun moliya har bir reliz uchun daromad panellarini moslashtirishi mumkin.

### Dalillar to'plami

1. Kontaktlar, qo'shimchalar doirasi va to'lov relslari bilan registrator chiptasi.
2. DNS/resolver dalillari (zonaviy fayl skeletlari + GAR dalillari).
3. Narxlar ish varag'i + boshqaruv tomonidan tasdiqlangan har qanday bekor qilish.
4. API/CLI tutun sinovi artefaktlari (`curl` namunalari, CLI transkriptlari).
5. KPI asboblar paneli skrinshoti + CSV eksporti, oylik ilovaga biriktirilgan.

## 3. Tekshirish ro'yxatini ishga tushirish

| Qadam | Egasi | Artefakt |
|------|-------|----------|
| Boshqaruv paneli import qilindi | Mahsulot tahlili | Grafana API javob + asboblar paneli UID |
| Portal kiritilishi tasdiqlandi | Docs/DevRel | `npm run build` jurnallari + oldindan ko'rish skrinshoti |
| DNS repetisiyasi tugallandi | Tarmoq/Ops | `sns_zonefile_skeleton.py` chiqishlari + runbook jurnali |
| Registrator avtomatlashtirish quruq ish | Registrator Eng | `sns_bulk_onboard.py` yuborishlar jurnali |
| Boshqaruv dalillari topshirildi | Boshqaruv Kengashi | Eksport qilingan asboblar panelining ilova havolasi + SHA-256 |

Ro'yxatga oluvchi yoki qo'shimchani faollashtirishdan oldin nazorat ro'yxatini to'ldiring. Imzolangan
bundle SN-8 yo'l xaritasi eshigini tozalaydi va auditorlarga qachon bitta ma'lumot beradi
bozorni ishga tushirishni ko'rib chiqish.