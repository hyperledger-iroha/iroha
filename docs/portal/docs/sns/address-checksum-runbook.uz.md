---
lang: uz
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fcd909a7013c5147e4f0c89c67de856ff56797b99281b954c7708ad83ab5cdc8
source_last_modified: "2026-01-28T17:11:30.699790+00:00"
translation_last_reviewed: 2026-02-07
id: address-checksum-runbook
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/sns/address_checksum_failure_runbook.md`ni aks ettiradi. Yangilash
avval manba faylini, keyin esa ushbu nusxani sinxronlashtiring.
:::

Tekshirish summasi xatosi `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) ko'rinishida yuzaga keladi.
Torii, SDK'lar va hamyon/ekspert mijozlari. Endi ADDR-6/ADDR-7 yo‘l xaritasi elementlari
operatorlardan nazorat summasi ogohlantirishlari yoki qo'llab-quvvatlanganda ushbu ish kitobiga amal qilishni talab qiladi
chiptalar yong'in.

## O'yinni qachon boshlash kerak

- **Ogohlantirishlar:** `AddressInvalidRatioSlo` (belgilangan
  `dashboards/alerts/address_ingest_rules.yml`) safarlari va izohlar roʻyxati
  `reason="ERR_CHECKSUM_MISMATCH"`.
- **Fiksture drift:** `account_address_fixture_status` Prometheus matn fayli yoki
  Grafana asboblar paneli har qanday SDK nusxasi uchun nazorat summasi mos kelmasligi haqida xabar beradi.
- **Yordamning kuchayishi:** Wallet/explorer/SDK guruhlari nazorat summasi, IME xatolarini keltirib chiqaradi
  buzilish yoki endi dekodlanmaydigan clipboard skanerlari.
- **Qo'lda kuzatish:** Torii jurnallarida takroriy `address_parse_error=checksum_mismatch` ko'rsatiladi
  ishlab chiqarishning so'nggi nuqtalari uchun.

Agar hodisa Lokal-8/Mahalliy-12 to'qnashuvi haqida bo'lsa, quyidagiga amal qiling
Buning o'rniga `AddressLocal8Resurgence` yoki `AddressLocal12Collision` o'yin kitoblari.

## Dalillarni tekshirish ro'yxati

| Dalil | Buyruq / Manzil | Eslatmalar |
|----------|-------------------|-------|
| Grafana surati | `dashboards/grafana/address_ingest.json` | Yaroqsiz sabablarni va ta'sirlangan so'nggi nuqtalarni yozib oling. |
| Ogohlantirish yuki | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Kontekst yorliqlari va vaqt belgilarini qo'shing. |
| Armatura sog'ligi | `artifacts/account_fixture/address_fixture.prom` + Grafana | SDK nusxalari `fixtures/account/address_vectors.json` dan o'zgarganligini isbotlaydi. |
| PromQL so'rovi | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Hodisa hujjati uchun CSV eksport qiling. |
| Jurnallar | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (yoki jurnallarni yig'ish) | Ulashishdan oldin PIIni tozalang. |
| Fiksturani tekshirish | `cargo xtask address-vectors --verify` | Kanonik generatorni tasdiqlaydi va belgilangan JSON rozi. |
| SDK paritetini tekshirish | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Ogohlantirishlar/chiptalarda bildirilgan har bir SDK uchun ishga tushiring. |
| Bufer/IME aqli | `iroha tools address inspect <literal>` | Yashirin belgilarni yoki IMEni qayta yozishni aniqlaydi; `address_display_guidelines.md` keltiring. |

## Darhol javob

1. Ogohlantirishni tan oling, voqeada Grafana suratlarini + PromQL chiqishini bog'lang
   mavzu va eslatma Torii kontekstlariga ta'sir qildi.
2. Manifest reklama aktsiyalarini / SDK relizlarini manzil tahliliga tegib muzlatish.
3. Boshqaruv paneli snapshotlari va yaratilgan Prometheus matn fayli artefaktlarini quyidagi manzilda saqlang
   voqea papkasi (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. `checksum_mismatch` foydali yuklarni ko'rsatadigan jurnal namunalarini oling.
5. SDK egalarini (`#sdk-parity`) namunaviy yuklamalar bilan xabardor qiling, shunda ular triajlashlari mumkin.

## Ildiz sabab izolyatsiyasi

### Fikstur yoki generator drifti

- `cargo xtask address-vectors --verify`-ni qayta ishga tushiring; muvaffaqiyatsiz bo'lsa, qayta tiklang.
- `ci/account_fixture_metrics.sh` (yoki individual
  Paketni tasdiqlash uchun har bir SDK uchun `scripts/account_fixture_helper.py check`).
  armatura kanonik JSONga mos keladi.

### Mijoz kodlovchilari / IME regressiyalari

- Nol kenglikni topish uchun `iroha tools address inspect` orqali foydalanuvchi tomonidan taqdim etilgan harflarni tekshiring
  qo'shilishlar, kana konvertatsiyalari yoki kesilgan foydali yuklar.
- Cross-check hamyon/Explorer bilan oqadi
  `docs/source/sns/address_display_guidelines.md` (ikki nusxadagi maqsadlar, ogohlantirishlar,
  QR yordamchilari) tasdiqlangan UXga rioya qilishlariga ishonch hosil qiling.

### Manifest yoki ro'yxatga olish bilan bog'liq muammolar

- Oxirgi manifest to'plamini qayta tasdiqlash uchun `address_manifest_ops.md` ga rioya qiling va
  Hech qanday Local-8 selektori qayta tiklanmaganligiga ishonch hosil qiling.
  foydali yuklarda paydo bo'ladi.

### Zararli yoki noto'g'ri tuzilgan trafik

- Torii jurnallari va `torii_http_requests_total` orqali haqoratli IP/ilova identifikatorlarini ajrating.
- Xavfsizlik/boshqaruvni kuzatish uchun kamida 24 soatlik jurnallarni saqlang.

## Yumshatish va tiklash

| Ssenariy | Amallar |
|----------|---------|
| Fiksturning drifti | `fixtures/account/address_vectors.json`-ni qayta tiklang, `cargo xtask address-vectors --verify`-ni qayta ishga tushiring, SDK to'plamlarini yangilang va chiptaga `address_fixture.prom` suratlarini biriktiring. |
| SDK/mijoz regressiyasi | Kanonik moslama + `iroha tools address inspect` chiqishi va SDK pariteti CI (masalan, `ci/check_address_normalize.sh`) ortidagi eshik relizlari bilan bog'liq fayl muammolari. |
| Zararli xabarlar | Tariflarni cheklang yoki qoidabuzarlik qiluvchi direktorlarni bloklang, agar qabr toshini tanlash kerak bo'lsa, Boshqaruvga etkazing. |

Yumshatish choralari qo'yilgach, tasdiqlash uchun yuqoridagi PromQL so'rovini qayta ishga tushiring
`ERR_CHECKSUM_MISMATCH` kamida nolda qoladi (`/tests/*` dan tashqari)
Hodisani pasaytirishdan 30 daqiqa oldin.

## Yopish

1. Arxiv Grafana suratlari, PromQL CSV, jurnaldan parchalar va `address_fixture.prom`.
2. `status.md` (ADDR bo'limi) va agar asboblar/hujjatlar mavjud bo'lsa, yo'l xaritasi qatorini yangilang
   o'zgardi.
3. Yangi darslar boshlanganda `docs/source/sns/incidents/` ostida voqeadan keyingi qaydlarni fayllang
   paydo bo'ladi.
4. SDK nashri eslatmalarida, agar kerak bo'lsa, nazorat yig'indisi tuzatishlari ko'rsatilganligiga ishonch hosil qiling.
5. Ogohlantirish 24 soat davomida yashil bo'lib turishini va armatura tekshiruvlari oldin yashil bo'lib qolishini tasdiqlang
   hal qilish.