---
lang: uz
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-12-29T18:16:35.087502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iqtisodiy tahlil - 2025-10 -> 2025-11 Shadow Run

Manba artefakt: `docs/examples/soranet_incentive_shadow_run.json` (imzo +
bir xil katalogdagi ochiq kalit). Simulyatsiya har bir estafeta uchun 60 ta davrni takrorladi
`RewardConfig` ga biriktirilgan mukofot dvigateli bilan
`reward_config.json`.

## Tarqatish xulosasi

- **Jami toʻlovlar:** 5160 XOR 360 dan ortiq mukofotlangan davrlar.
- **Adolat konverti:** Jini koeffitsienti 0,121; yuqori o'rni ulushi 23,26%
  (30% boshqaruv panjarasidan ancha past).
- **Mavjudligi:** flot o'rtacha 96,97%, barcha o'rni 94% dan yuqori bo'lib qoldi.
- **Transport kengligi:** flot o'rtacha 91,20%, eng past ko'rsatkich 87,23%
  rejalashtirilgan ta'mirlash vaqtida; jarimalar avtomatik tarzda qo'llanildi.
- **Muvofiqlik shovqini:** 9 ta ogohlantirish davri va 3 ta suspenziya kuzatildi va
  to'lovlarni kamaytirishga tarjima qilingan; hech qanday rele 12-ogohlantirish qopqog'idan oshmadi.
- **Ish gigienasi:** hech qanday ko‘rsatkichlar snapshotlari etishmayotganligi sababli o‘tkazib yuborilmadi
  konfiguratsiya, obligatsiyalar yoki dublikatlar; hech qanday kalkulyator xatosi yo'q edi.

## Kuzatishlar

- Süspansiyonlar o'rni parvarishlash rejimiga kirgan davrlarga to'g'ri keladi. The
  to'lov mexanizmi saqlab qolgan holda o'sha davrlar uchun nol to'lovlarni chiqardi
  soyada boshqariladigan JSONdagi audit izi.
- Ogohlantirish jarimalari zararlangan to'lovlardan 2% chegirildi; natijada
  tarqatish hali ham ish vaqti/tarmoqli kengligi og'irliklari (650/350
  promille).
- Tarmoqli kengligi farqi anonim himoyalangan issiqlik xaritasini kuzatib boradi. Eng past ko'rsatkich
  (`6666...6666`) deraza bo'ylab, 0,6x qavatdan yuqorida 620 XORni saqlab qoldi.
- Kechikishga sezgir ogohlantirishlar (`SoranetRelayLatencySpike`) ogohlantirish ostida qoldi
  deraza bo'ylab eshiklar; o'zaro bog'liq asboblar paneli ostida olingan
  `dashboards/grafana/soranet_incentives.json`.

## GA dan oldin tavsiya etilgan harakatlar

1. Oylik soyalarni takrorlashni davom ettiring va artefakt to'plamini yangilang va bu
   flot tarkibi o'zgargan taqdirda tahlil qilish.
2. Yo‘l xaritasida ko‘rsatilgan Grafana ogohlantirish to‘plamida avtomatik to‘lovlarni amalga oshiring
   (`dashboards/alerts/soranet_incentives_rules.yml`); skrinshotlarni nusxalash
   yangilashni talab qilganda boshqaruv protokollari.
3. Iqtisodiy stress testini qayta o'tkazing, agar asosiy mukofot, ish vaqti / tarmoqli kengligi og'irliklari yoki
   muvofiqlik jazosi >=10% ga o'zgaradi.