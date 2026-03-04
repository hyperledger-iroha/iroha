---
id: nexus-settlement-faq
lang: uz
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Bu sahifa ichki hisob-kitoblarni aks ettiradi (`docs/source/nexus_settlement_faq.md`)
shuning uchun portal o'quvchilari bir xil yo'riqnomani qazib olmagan holda ko'rib chiqishlari mumkin
mono-repo. Bu Settlement Router to'lovlarni qanday amalga oshiradi, qanday ko'rsatkichlarni tushuntiradi
monitoring qilish va SDKlar Norito foydali yuklarini qanday birlashtirishi kerak.

## E'tiborga molik

1. **Line xaritalash** — har bir maʼlumot maydoni `settlement_handle` ni eʼlon qiladi
   (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` yoki
   `xor_dual_fund`). Eng so'nggi yo'lak katalogiga qarang
   `docs/source/project_tracker/nexus_config_deltas/`.
2. **Deterministik konversiya** — router orqali barcha hisob-kitoblarni XOR ga o'zgartiradi
   boshqaruv tomonidan tasdiqlangan likvidlik manbalari. Xususiy yo'llar XOR buferlarini oldindan moliyalashtiradi;
   Sartaroshlar faqat buferlar siyosatdan tashqariga chiqqanda qo'llaniladi.
3. **Telemetriya** — soat `nexus_settlement_latency_seconds`, konversiya hisoblagichlari,
   va soch o'lchagichlari. Boshqaruv panellari `dashboards/grafana/nexus_settlement.json` da yashaydi
   va `dashboards/alerts/nexus_audit_rules.yml` da ogohlantirishlar.
4. **Dalillar** — arxiv konfiguratsiyasi, router jurnallari, telemetriya eksporti va
   audit uchun solishtirish hisobotlari.
5. **SDK mas'uliyati** — har bir SDK hisob-kitob yordamchilarini, qator identifikatorlarini,
   va marshrutizator bilan tenglikni saqlash uchun Norito foydali yuk enkoderlari.

## Oqimlarga misol

| Yo'lak turi | Qo'lga olish uchun dalillar | Bu nimani isbotlaydi |
|----------|--------------------|----------------|
| Shaxsiy `xor_hosted_custody` | Router jurnali + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC buferlari debet deterministik XOR va soch turmagi siyosat doirasida qoladi. |
| Ommaviy `xor_global` | Router jurnali + DEX/TWAP ma'lumotnomasi + kechikish/konversiya ko'rsatkichlari | Birgalikda likvidlik yo'li nashr etilgan TWAPda o'tkazma narxini nol sochlar bilan belgiladi. |
| Gibrid `xor_dual_fund` | Router jurnali umumiy va himoyalangan split + telemetriya hisoblagichlarini ko'rsatadi | Himoyalangan/ommaviy aralash boshqaruv nisbatlariga rioya qildi va har bir oyoqqa qo'llaniladigan soch turmagini qayd etdi. |

## Batafsil ma'lumot kerakmi?

- To'liq tez-tez so'raladigan savollar: `docs/source/nexus_settlement_faq.md`
- Hisob-kitob marshrutizatorining xususiyatlari: `docs/source/settlement_router.md`
- CBDC siyosati kitobi: `docs/source/cbdc_lane_playbook.md`
- Operatsiyalar kitobi: [Nexus operatsiyalar](./nexus-operations)