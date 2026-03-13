---
lang: uz
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1be9268784bf75c4c5d1bf854e72c817475a079c0d2bf06ce120ccd325ad6083
source_last_modified: "2026-01-22T14:45:01.248924+00:00"
translation_last_reviewed: 2026-02-07
id: payment-settlement-plan
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
---

> Kanonik manba: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

Yo'l xaritasi vazifasi **SN-5 — To'lov va hisob-kitob xizmati** deterministikni taqdim etadi
Sora Name Service uchun to'lov qatlami. Har bir ro'yxatdan o'tish, yangilash yoki pulni qaytarish
tuzilgan Norito foydali yukni chiqarishi kerak, shuning uchun xazina, boshqaruvchilar va boshqaruv
moliyaviy oqimlarni jadvallarsiz takrorlang. Ushbu sahifa spetsifikatsiyani distillaydi
portal auditoriyasi uchun.

## Daromad modeli

- Asosiy to'lov (`gross_fee`) ro'yxatga oluvchining narxlash matritsasidan olinadi.  
- G'aznachilik `gross_fee × 0.70` oladi, styuardlar qolgan minusni oladi
  yo'llanma bonuslari (cheklangan 10%).  
- Ixtiyoriy cheklashlar boshqaruvga nizolar paytida boshqaruvchi to'lovlarni to'xtatib turish imkonini beradi.  
- O'rnatish to'plamlari beton bilan `ledger_projection` blokini ochib beradi
  `Transfer` ISI, shuning uchun avtomatlashtirish XOR harakatlarini to'g'ridan-to'g'ri Torii ga joylashtirishi mumkin.

## Xizmatlar va avtomatlashtirish

| Komponent | Maqsad | Dalil |
|----------|---------|----------|
| `sns_settlementd` | Siyosatni qo'llaydi, to'plamlar, yuzalar `/v2/sns/settlements` belgilari. | JSON to'plami + xesh. |
| Hisob-kitob navbati & yozuvchi | Idempotent navbat + `iroha_cli app sns settlement ledger` tomonidan boshqariladigan daftar topshiruvchisi. | To‘plam xesh ↔ tx xesh manifesti. |
| Yarashtirish ishi | `docs/source/sns/reports/` ostida kunlik farq + oylik hisobot. | Markdown + JSON dayjesti. |
| To'lovni qaytarish stoli | `/settlements/{id}/refund` orqali boshqaruv tomonidan tasdiqlangan toʻlovlar. | `RefundRecordV1` + chipta. |

CI yordamchilari ushbu oqimlarni aks ettiradi:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Kuzatish va hisobot berish

- Boshqaruv panellari: `dashboards/grafana/sns_payment_settlement.json` xazina va boshqalar uchun
  styuard jami, yo'llanma to'lovlari, navbat chuqurligi va to'lovni qaytarish kechikishi.
- Ogohlantirishlar: `dashboards/alerts/sns_payment_settlement_rules.yml` monitorlari kutilmoqda
  yoshi, yarashuvdagi muvaffaqiyatsizliklar va daftarning o'zgarishi.
- Bayonotlar: kunlik dayjestlar (`settlement_YYYYMMDD.{json,md}`) har oyga aylanadi
  hisobotlar (`settlement_YYYYMM.md`) ham Git, ham
  boshqaruv ob'ektlari do'koni (`s3://sora-governance/sns/settlements/<period>/`).
- Boshqaruv paketlari boshqaruv paneli, CLI jurnallari va kengashda tasdiqlashlar to'plami
  ro'yxatdan o'tish.

## Chiqarish nazorat ro'yxati

1. Iqtibos prototipi + daftar yordamchilari va staging to'plamini suratga oling.
2. `sns_settlementd`ni navbat + yozuvchi, sim asboblar paneli va mashqlar bilan ishga tushiring
   ogohlantirish testlari (`promtool test rules ...`).
3. To'lovni qaytarish yordamchisini va oylik hisobot shablonini yetkazib berish; aks ettirilgan artefaktlar
   `docs/portal/docs/sns/reports/`.
4. Hamkor mashqini o'tkazing (to'liq oylik hisob-kitoblar) va qo'lga oling
   boshqaruv ovozi SN-5ni tugallangan deb belgilash.

Aniq sxema ta'riflari uchun manba hujjatiga qayting, oching
savollar va kelajakdagi tuzatishlar.