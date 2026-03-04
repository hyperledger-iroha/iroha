---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cac03d504c6a7dcfacaa4b298e14f0a71ccbcb5ec58f1977b5bf124300c8ec61
source_last_modified: "2026-01-05T09:28:11.865094+00:00"
translation_last_reviewed: 2026-02-07
id: developer-deployment
title: SoraFS Deployment Notes
sidebar_label: Deployment Notes
description: Checklist for promoting the SoraFS pipeline from CI to production.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

# Joylashtirish bo'yicha eslatmalar

SoraFS qadoqlash ish jarayoni determinizmni qattiqlashtiradi, shuning uchun CI dan o'tish
ishlab chiqarish, asosan, operatsion to'siqlarni talab qiladi. Bu nazorat roʻyxatidan qachon foydalaning
asboblarni haqiqiy shlyuzlar va saqlash provayderlariga tarqatish.

## Parvoz oldidan

- **Ro'yxatga olish kitobini tekislash** — chunker profillari va manifestlarga havolani tasdiqlang
  bir xil `namespace.name@semver` korteji (`docs/source/sorafs/chunker_registry.md`).
- **Qabul siyosati** — imzolangan provayder eʼlonlari va taxallus dalillarini koʻrib chiqing
  `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) uchun kerak.
- **Pin registrini runbook** — `docs/source/sorafs/runbooks/pin_registry_ops.md` saqlang
  tiklash stsenariylari uchun qulay (taxallusni aylantirish, replikatsiya xatosi).

## Atrof-muhit konfiguratsiyasi

- Shlyuzlar isbot oqimining oxirgi nuqtasini yoqishi kerak (`POST /v1/sorafs/proof/stream`)
  shuning uchun CLI telemetriya xulosalarini chiqarishi mumkin.
- Standart sozlamalardan foydalanib, `sorafs_alias_cache` siyosatini sozlang
  `iroha_config` yoki CLI yordamchisi (`sorafs_cli manifest submit --alias-*`).
- Xavfsiz maxfiy menejer orqali oqim tokenlarini (yoki Torii hisobga olish ma'lumotlarini) taqdim eting.
- Telemetriya eksportyorlarini yoqish (`torii_sorafs_proof_stream_*`,
  `torii_sorafs_chunk_range_*`) va ularni Prometheus/OTel stekingizga yuboring.

## Chiqarish strategiyasi

1. **Ko‘k/yashil ko‘rinishlar**
   - Har bir tarqatish uchun javoblarni arxivlash uchun `manifest submit --summary-out` dan foydalaning.
   - Imkoniyatni aniqlash uchun `torii_sorafs_gateway_refusals_total` ni kuzatib boring
     erta mos kelmaslik.
2. **Isbotni tekshirish**
   - `sorafs_cli proof stream` dagi nosozliklarni joylashtirish blokerlari sifatida davolash; kechikish
     tikanlar ko'pincha provayderning qisqarishi yoki noto'g'ri sozlangan darajalarni ko'rsatadi.
   - `proof verify` CARni ta'minlash uchun tutundan keyingi sinovning bir qismi bo'lishi kerak
     provayderlar tomonidan joylashtirilgan hali ham manifest dayjestiga mos keladi.
3. **Telemetriya asboblar paneli**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` ni Grafana ga import qiling.
   - PIN ro'yxatga olish kitobi salomatligi uchun qo'shimcha panellarni joylashtiring
     (`docs/source/sorafs/runbooks/pin_registry_ops.md`) va parcha diapazoni statistikasi.
4. **Ko‘p manbalarni yoqish**
   - Bosqichli chiqarish bosqichlarini bajaring
     yoqilganda `docs/source/sorafs/runbooks/multi_source_rollout.md`
     orkestrator va auditlar uchun skorbord/temetriya artefaktlarini arxivlang.

## Hodisani boshqarish

- `docs/source/sorafs/runbooks/` da eskalatsiya yo'llariga rioya qiling:
  - `sorafs_gateway_operator_playbook.md` shlyuz uzilishlari va oqim tokeni uchun
    charchoq.
  - `dispute_revocation_runbook.md` replikatsiya nizolari yuzaga kelganda.
  - tugun darajasida texnik xizmat ko'rsatish uchun `sorafs_node_ops.md`.
  - `multi_source_rollout.md` orkestrni bekor qilish, tengdoshlarning qora ro'yxati va
    bosqichli chiqishlar.
- Mavjud bo'lganlar orqali GovernanceLog-da xatoliklarni va kechikish anomaliyalarini yozib oling
  PoR kuzatuvchisi API-lari, shuning uchun boshqaruv provayderning ish faoliyatini baholashi mumkin.

## Keyingi qadamlar

- Orkestrni avtomatlashtirishni (`sorafs_car::multi_fetch`) bir marta integratsiyalash
  ko'p manbali olib kelish orkestri (SF-6b) erlari.
- SF-13/SF-14 ostida PDP/PoTR yangilanishlarini kuzatish; CLI va hujjatlar rivojlanadi
  bu dalillar barqarorlashgandan so'ng, sirt muddati va daraja tanlash.

Ushbu joylashtirish eslatmalarini tezkor boshlash va CI retseptlari bilan birlashtirib, jamoalar
mahalliy tajribalardan ishlab chiqarish darajasidagi SoraFS quvurlariga o'tishi mumkin.
takrorlanadigan, kuzatilishi mumkin bo'lgan jarayon.