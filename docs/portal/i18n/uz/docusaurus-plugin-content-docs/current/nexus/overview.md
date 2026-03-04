---
id: nexus-overview
lang: uz
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus overview
description: High-level summary of the Iroha 3 (Sora Nexus) architecture with pointers to the canonical mono-repo docs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Nexus (Iroha 3) Iroha 2 ni koʻp qatorli, boshqaruv doirasi bilan kengaytiradi
ma'lumotlar bo'shliqlari va har bir SDK bo'ylab umumiy vositalar. Bu sahifa yangilikni aks ettiradi
Mono-repoda `docs/source/nexus_overview.md` qisqacha ma'lumot, shuning uchun portal o'quvchilari
arxitektura qismlari bir-biriga qanday mos kelishini tezda tushuning.

## Chiqarish qatorlari

- **Iroha 2** – konsortsium yoki xususiy tarmoqlar uchun oʻz-oʻzidan joylashtirish.
- **Iroha 3 / Sora Nexus** – operatorlar ishlaydigan ko'p tarmoqli umumiy tarmoq.
  ma'lumotlar bo'shliqlarini (DS) ro'yxatdan o'tkazish va umumiy boshqaruvni, hisob-kitoblarni va meros qilib olish
  kuzatuvchanlik vositalari.
- Ikkala qator ham bir xil ish maydonidan kompilyatsiya qilinadi (IVM + Kotodama asboblar zanjiri), shuning uchun SDK
  tuzatishlar, ABI yangilanishlari va Norito moslamalari portativ bo'lib qoladi. Operatorlar yuklab olish
  Nexusga qo'shilish uchun `iroha3-<version>-<os>.tar.zst` to'plami; murojaat qiling
  Toʻliq ekran nazorat roʻyxati uchun `docs/source/sora_nexus_operator_onboarding.md`.

## Qurilish bloklari

| Komponent | Xulosa | Portal kancalari |
|----------|---------|--------------|
| Ma'lumotlar maydoni (DS) | Boshqaruv tomonidan belgilangan ijro/saqlovchi domen bir yoki bir nechta qatorga ega, validatorlar toʻplami, maxfiylik klassi, toʻlov + DA siyosatini eʼlon qiladi. | Manifest sxemasi uchun [Nexus spec](./nexus-spec) ga qarang. |
| Lane | Deterministik bajarilish parchasi; global NPoS halqasi buyurtma qilgan majburiyatlarni chiqaradi. Lane sinflariga `default_public`, `public_custom`, `private_permissioned` va `hybrid_confidential` kiradi. | [Line modeli](./nexus-lane-model) geometriya, saqlash prefikslari va ushlab turishni oladi. |
| O'tish rejasi | To'ldiruvchi identifikatorlari, marshrutlash bosqichlari va ikki profilli qadoqlash bir qatorli joylashtirishlar Nexus ga qanday o'tishini kuzatib boradi. | [Oʻtish qaydlari](./nexus-transition-notes) har bir migratsiya bosqichini hujjatlashtiradi. |
| Kosmik katalog | DS manifestlari + versiyalarini saqlaydigan registr shartnomasi. Operatorlar qo'shilishdan oldin katalogdagi yozuvlarni ushbu katalogga moslashtiradi. | Manifest diff tracker `docs/source/project_tracker/nexus_config_deltas/` ostida ishlaydi. |
| Lane katalogi | `[nexus]` konfiguratsiya boʻlimi, tarmoq identifikatorlarini taxalluslar, marshrutlash siyosatlari va DA chegaralari bilan taqqoslaydi. `irohad --sora --config … --trace-config` audit uchun hal qilingan katalogni chop etadi. | CLI orqali o'tish uchun `docs/source/sora_nexus_operator_onboarding.md` dan foydalaning. |
| Hisob-kitob routeri | XOR transfer orkestratori xususiy CBDC yo'llarini jamoat likvidligi yo'llari bilan bog'laydi. | `docs/source/cbdc_lane_playbook.md` siyosat tugmalari va telemetriya eshiklarini talaffuz qiladi. |
| Telemetriya/SLOs | `dashboards/grafana/nexus_*.json` ostida asboblar paneli + ogohlantirishlar chiziq balandligi, DA kechikishi, hisob-kitob kechikishi va boshqaruv navbati chuqurligini oladi. | [Telemetriyani tuzatish rejasi](./nexus-telemetry-remediation) asboblar paneli, ogohlantirishlar va audit dalillarini ifodalaydi. |

## Rollout snapshoti

| Bosqich | Fokus | Chiqish mezonlari |
|-------|-------|---------------|
| N0 – Yopiq beta | Kengash tomonidan boshqariladigan registrator (`.sora`), operatorni qo'lda ishga tushirish, statik chiziqli katalog. | Imzolangan DS manifestlari + boshqaruvning takroriy topshiriqlari. |
| N1 – Ommaviy ishga tushirish | `.nexus` qo'shimchalarini, auktsionlarni, o'z-o'ziga xizmat ko'rsatish registratorini, XOR hisob-kitob simlarini qo'shadi. | Resolver/shlyuz sinxronlash testlari, hisob-kitoblarni yarashtirish asboblar paneli, bahsli stol usti mashqlari. |
| N2 – Kengaytirish | `.dao`, sotuvchi API'lari, tahlillar, nizolar portali, boshqaruvchi ko'rsatkich kartalari bilan tanishtiradi. | Muvofiqlik artefaktlari versiyasi, siyosat-hakamlar hay'ati asboblar to'plami onlayn, xazina shaffofligi hisobotlari. |
| NX-12/13/14 eshigi | Muvofiqlik mexanizmi, telemetriya asboblar paneli va hujjatlar hamkor uchuvchilardan oldin birga yuborilishi kerak. | [Nexus umumiy koʻrinishi](./nexus-overview) + [Nexus operatsiyalari](./nexus-operations) chop etildi, asboblar paneli simli, siyosat mexanizmi birlashtirildi. |

## Operatorning majburiyatlari

1. **Konfiguratsiya gigienasi** – `config/config.toml` ni eʼlon qilingan chiziq bilan sinxronlash &
   ma'lumotlar maydoni katalogi; arxiv `--trace-config` chiqishi har bir reliz chiptasi bilan.
2. **Manifest kuzatuvi** – katalogdagi yozuvlarni oxirgi Space bilan muvofiqlashtirish
   Tugunlarga qo'shilish yoki yangilashdan oldin katalog to'plami.
3. **Telemetriya qamrovi** – `nexus_lanes.json`, `nexus_settlement.json`,
   va tegishli SDK boshqaruv panellari; PagerDuty-ga simli ogohlantirishlar yuboradi va telemetriyani tuzatish rejasiga muvofiq har chorakda ko'rib chiqing.
4. **Hodisalar haqida xabar berish** – jiddiylik matritsasiga amal qiling
   [Nexus operatsiyalari](./nexus-operations) va RCAlarni besh ish kuni ichida topshiring.
5. **Boshqaruv tayyorligi** – Nexus kengash ovozlarida qatnashing
   Har chorakda orqaga qaytarish ko'rsatmalarini takrorlang (kuzatuv orqali
   `docs/source/project_tracker/nexus_config_deltas/`).

## Shuningdek qarang

- Kanonik sharh: `docs/source/nexus_overview.md`
- Batafsil spetsifikatsiya: [./nexus-spec](./nexus-spec)
- Bo'lak geometriyasi: [./nexus-lane-model](./nexus-lane-model)
- O'tish rejasi: [./nexus-transition-notes](./nexus-transition-notes)
- Telemetriyani tuzatish rejasi: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Operations runbook: [./nexus-operations](./nexus-operations)
- Operatorni ishga tushirish bo'yicha qo'llanma: `docs/source/sora_nexus_operator_onboarding.md`