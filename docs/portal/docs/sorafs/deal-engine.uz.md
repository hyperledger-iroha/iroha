---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6404e09aa8f3520328249a1d5c41309b291087908a2a8f5abae3e2fe12de44fb
source_last_modified: "2026-01-05T09:28:11.861409+00:00"
translation_last_reviewed: 2026-02-07
id: deal-engine
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: Overview of the SF-8 deal engine, Torii integration, and telemetry surfaces.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

# SoraFS Deal Engine

SF-8 yo'l xaritasi treki SoraFS kelishuv dvigatelini taqdim etadi,
o'rtasidagi saqlash va olish shartnomalari uchun deterministik hisobga olish
mijozlar va provayderlar. Shartnomalar Norito foydali yuklari bilan tavsiflanadi
`crates/sorafs_manifest/src/deal.rs` da belgilangan, bitim shartlarini, obligatsiyani o'z ichiga oladi
qulflash, ehtimollik mikroto'lovlari va hisob-kitob yozuvlari.

O'rnatilgan SoraFS ishchisi (`sorafs_node::NodeHandle`) endi
Har bir tugun jarayoni uchun `DealEngine` misoli. Dvigatel:

- `DealTermsV1` yordamida bitimlarni tasdiqlaydi va qayd etadi;
- replikatsiyadan foydalanish haqida xabar berilganda XOR bilan belgilangan to'lovlarni hisoblab chiqadi;
- deterministik yordamida ehtimollik mikroto'lov oynalarini baholaydi
  Bleyk3 asosidagi namuna olish; va
- boshqaruv uchun mos bo'lgan buxgalteriya resurslari va hisob-kitob yuklarini ishlab chiqaradi
  nashr qilish.

Birlik testlari tekshirish, mikroto'lovni tanlash va hisob-kitob oqimlarini qamrab oladi
operatorlar API-lardan ishonch bilan foydalanishlari mumkin. Hisob-kitoblar endi chiqariladi
`DealSettlementV1` boshqaruv yuklari, simlarni bevosita SF-12 ga ulash
nashr qilish quvuri va `sorafs.node.deal_*` OpenTelemetry seriyasini yangilang
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) Torii asboblar paneli va SLO uchun
ijro etish. Kuzatuv elementlari auditor tashabbusi bilan slash avtomatlashtirish va
boshqaruv siyosati bilan bekor qilish semantikasini muvofiqlashtirish.

Foydalanish telemetriyasi endi `sorafs.node.micropayment_*` ko'rsatkichlari to'plamini ham ta'minlaydi:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano` va chipta hisoblagichlari
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Bu jamilar ehtimollikni ochib beradi
Lotereya oqimi, shuning uchun operatorlar mikroto'lov yutuqlari va kredit o'tkazishni o'zaro bog'lashlari mumkin
hisob-kitob natijalari bilan.

## Torii integratsiyasi

Torii provayderlar foydalanish haqida xabar berishlari va boshqaruvni boshqarishlari uchun maxsus so'nggi nuqtalarni ochib beradi.
buyurtma qilingan simlarsiz ishlash muddati:

- `POST /v2/sorafs/deal/usage` `DealUsageReport` telemetriyasini qabul qiladi va qaytaradi
  deterministik buxgalteriya natijalari (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` oqimli joriy oynani yakunlaydi
  natijada `DealSettlementRecord` base64 kodli `DealSettlementV1` bilan birga
  boshqaruv DAG nashri uchun tayyor.
- Torii ning `/v2/events/sse` tasmasi endi `SorafsGatewayEvent::DealUsage`ni efirga uzatadi
  har bir foydalanish topshirig'ini jamlovchi yozuvlar (davr, o'lchangan GiB-soatlar, chipta
  hisoblagichlar, deterministik to'lovlar), `SorafsGatewayEvent::DealSettlement`
  kanonik hisob-kitob kitobi snapshotini o'z ichiga olgan yozuvlar va
  Diskdagi boshqaruv artefaktining BLAKE3 dayjesti/hajmi/bazasi64 va
  `SorafsGatewayEvent::ProofHealth` har doim PDP/PoTR chegaralari bo'lganda ogohlantiradi
  oshib ketgan (provayder, oyna, ish tashlash/sovutish holati, jarima miqdori). Iste'molchilar mumkin
  yangi telemetriya, aholi punktlari yoki sog'liqni tekshirish ogohlantirishlariga so'rovsiz javob berish uchun provayder tomonidan filtrlang.

Ikkala so'nggi nuqta yangi orqali SoraFS kvotasi doirasida ishtirok etadi.
Operatorlarga sozlash imkonini beruvchi `torii.sorafs.quota.deal_telemetry` oynasi
har bir joylashtirish uchun ruxsat etilgan yuborish tezligi.