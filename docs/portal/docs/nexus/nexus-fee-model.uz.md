---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e45957522f3ac3ab0d003af79dc75bee1a2bf3c16d3aa8b6926f4c2b50a524a1
source_last_modified: "2025-12-29T18:16:35.137714+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-fee-model
title: Nexus fee model updates
description: Mirror of `docs/source/nexus_fee_model.md`, documenting the lane settlement receipts and reconciliation surfaces.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/nexus_fee_model.md`ni aks ettiradi. Yapon, ibroniy, ispan, portugal, frantsuz, rus, arab va urdu tillariga tarjimalar koʻchib oʻtayotganda ikkala nusxani ham tekislang.
:::

# Nexus to'lov modeli yangilanishi

Yagona hisob-kitob marshrutizatori endi har bir tarmoqli deterministik tushumlarni oladi
operatorlar gaz debetlarini Nexus to'lov modeliga solishtirishlari mumkin.

- Routerning to'liq arxitekturasi, bufer siyosati, telemetriya matritsasi va ishlab chiqarish uchun
  ketma-ketlik `docs/settlement-router.md` ga qarang. Ushbu qo'llanma qanday ekanligini tushuntiradi
  Bu yerda hujjatlashtirilgan parametrlar NX-3 yoʻl xaritasi yetkazib berilishi va qanday SRE bilan bogʻliq
  ishlab chiqarishda marshrutizatorni kuzatishi kerak.
- Gaz aktivlari konfiguratsiyasi (`pipeline.gas.units_per_gas`) a kiradi
  `twap_local_per_xor` kasrli, `liquidity_profile` (`tier1`, `tier2`,
  yoki `tier3`) va `volatility_class` (`stable`, `elevated`, `dislocated`).
  Ushbu bayroqlar hisob-kitob routerini oziqlantiradi, natijada XOR hosil bo'ladi
  Iqtibos kanonik TWAP va chiziq uchun soch turmagi darajasiga mos keladi.
- Gaz to'laydigan har bir tranzaksiyada `LaneSettlementReceipt` qayd etiladi.  Har bir
  kvitansiya qo'ng'iroq qiluvchi tomonidan taqdim etilgan manba identifikatorini, mahalliy mikro-summani,
  XOR darhol to'lanadi, XOR soch kesilganidan keyin kutilgan, amalga oshirilgan
  dispersiya (`xor_variance_micro`) va blok vaqt tamg'asi millisekundlarda.
- Blok ijrosi har bir qator/ma'lumotlar maydoni bo'yicha tushumlarni jamlaydi va ularni nashr etadi
  `lane_settlement_commitments` orqali `/v1/sumeragi/status` da.  Jami
  `total_local_micro`, `total_xor_due_micro` va
  `total_xor_after_haircut_micro` tungi blokda yakunlandi
  muvofiqlashtirish eksporti.
- Yangi `total_xor_variance_micro` hisoblagichi xavfsizlik chegarasi qancha ekanligini kuzatib boradi
  iste'mol qilingan (kerakli XOR va soch kesishdan keyingi kutish o'rtasidagi farq),
  va `swap_metadata` deterministik konvertatsiya parametrlarini hujjatlashtiradi
  (TWAP, epsilon, likvidlik profili va volatilite_sinfi).
  ish vaqti konfiguratsiyasidan qat'i nazar, taklif kiritishlarini tekshiring.

Iste'molchilar `lane_settlement_commitments` ni mavjud bo'lak bilan birga tomosha qilishlari mumkin
toʻlov buferlari, soch turmagi darajalari,
va almashtirishni amalga oshirish sozlangan Nexus to'lov modeliga mos keladi.