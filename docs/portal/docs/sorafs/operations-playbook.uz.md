---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 31b279a990f47774972731f7f6b5181ed3b4625a7cd9d3e015b24c180e129c7b
source_last_modified: "2026-01-22T14:35:36.755283+00:00"
translation_last_reviewed: 2026-02-07
id: operations-playbook
title: SoraFS Operations Playbook
sidebar_label: Operations Playbook
description: Incident response guides and chaos drill procedures for SoraFS operators.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/sorafs_ops_playbook.md` ostida saqlanadigan runbookni aks ettiradi. Sfenks hujjatlari to'plami to'liq ko'chirilgunga qadar ikkala nusxani ham sinxron holda saqlang.
:::

## Asosiy havolalar

- Kuzatish mumkin bo'lgan aktivlar: `dashboards/alerts/` da `dashboards/grafana/` va Prometheus ogohlantirish qoidalari ostidagi Grafana asboblar paneliga qarang.
- Metrik katalog: `docs/source/sorafs_observability_plan.md`.
- Orkestr telemetriya sirtlari: `docs/source/sorafs_orchestrator_plan.md`.

## Eskalatsiya matritsasi

| Ustuvorlik | Trigger misollari | Birlamchi chaqiruv | Zaxira | Eslatmalar |
|----------|------------------|-----------------|--------|-------|
| P1 | Global shlyuzning uzilishi, PoR ishlamay qolish darajasi > 5% (15 daqiqa), replikatsiya kechikishi har 10 daqiqada ikki barobar ortadi | Saqlash SRE | Kuzatish qobiliyati TL | Ta'sir 30 daqiqadan oshsa, boshqaruv kengashini jalb qiling. |
| P2 | Mintaqaviy shlyuzning kechikishi SLO buzilishi, SLA ta'sirisiz orkestratorning qayta urinish ko'tarilishi | Kuzatish qobiliyati TL | Saqlash SRE | Chiqarishni davom ettiring, lekin yangi manifestlarni oching. |
| P3 | Kritik bo'lmagan ogohlantirishlar (aniq eskirganlik, sig'imi 80–90%) | Qabul qilish triaji | Operatsiya gildiyasi | Keyingi ish kuni ichida manzil. |

## Gateway uzilishi / Mavjudligi yomonlashgan

**Aniqlash**

- Ogohlantirishlar: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Boshqaruv paneli: `dashboards/grafana/sorafs_gateway_overview.json`.

**Tezkor harakatlar**

1. So'rov tezligi paneli orqali qo'llanilish doirasini tasdiqlang (yagona provayder va flot).
2. Ops konfiguratsiyasida (`docs/source/sorafs_gateway_self_cert.md`) `sorafs_gateway_route_weights` ni almashtirish orqali Torii marshrutini sog'lom provayderlarga (agar ko'p provayder bo'lsa) o'tkazing.
3. Agar barcha provayderlar ta'sir qilgan bo'lsa, CLI/SDK mijozlari uchun "to'g'ridan-to'g'ri olish" ni yoqing (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- `sorafs_gateway_stream_token_limit` ga qarshi oqim tokenidan foydalanishni tekshiring.
- TLS yoki qabul qilish xatolari uchun shlyuz jurnallarini tekshiring.
- Eksport qilingan shlyuz sxemasi kutilgan versiyaga mos kelishiga ishonch hosil qilish uchun `scripts/telemetry/run_schema_diff.sh` ni ishga tushiring.

**Tuzatish imkoniyatlari**

- Faqat ta'sirlangan shlyuz jarayonini qayta ishga tushiring; Agar bir nechta provayderlar muvaffaqiyatsiz bo'lmasa, butun klasterni qayta ishlashdan qoching.
- Agar toʻyinganlik tasdiqlansa, oqim tokeni chegarasini vaqtincha 10–15% ga oshiring.
- Stabilizatsiyadan so'ng o'z-o'zini sertifikatlashni qayta ishga tushiring (`scripts/sorafs_gateway_self_cert.sh`).

**Hodisadan keyingi**

- `docs/source/sorafs/postmortem_template.md` yordamida P1 postmortem fayli.
- Agar tuzatish qo'lda aralashuvga tayangan bo'lsa, keyingi tartibsizlik mashqlarini rejalashtiring.

## Ishonchsizlikni isbotlovchi keskin (PoR / PoTR)

**Aniqlash**

- Ogohlantirishlar: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Boshqaruv paneli: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetriya: `torii_sorafs_proof_stream_events_total` va `sorafs.fetch.error` hodisalari `provider_reason=corrupt_proof` bilan.

**Tezkor harakatlar**

1. Manifest registrini (`docs/source/sorafs/manifest_pipeline.md`) belgilab, yangi manifest qabullarini muzlatib qo'ying.
2. Ta'sir ko'rsatuvchi provayderlarni rag'batlantirishni to'xtatib turish to'g'risida hukumatga xabar bering.

**Triage**

- `sorafs_node_replication_backlog_total` va PoR sinov navbatining chuqurligini tekshiring.
- Oxirgi tarqatishlar uchun tasdiqlovchi tekshirish quvur liniyasini (`crates/sorafs_node/src/potr.rs`) tasdiqlang.
- Provayder proshivka versiyalarini operator registrlari bilan solishtiring.

**Tuzatish imkoniyatlari**

- Eng so'nggi manifest bilan `sorafs_cli proof stream` yordamida PoR takrorlarini ishga tushiring.
- Agar dalillar doimiy ravishda muvaffaqiyatsiz bo'lsa, boshqaruv registrini yangilash va orkestr ko'rsatkichlari jadvalini yangilashga majburlash orqali provayderni faol to'plamdan olib tashlang.

**Hodisadan keyingi**

- Keyingi ishlab chiqarishni joylashtirishdan oldin PoR xaos matkap stsenariysini ishga tushiring.
- O'limdan keyingi shablonda darslarni yozib oling va provayderning malakasini tekshirish ro'yxatini yangilang.

## Replikatsiya kechikishi / Orqaga o'sish

**Aniqlash**

- Ogohlantirishlar: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Import
  `dashboards/alerts/sorafs_capacity_rules.yml` va ishga tushiring
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  rag'batlantirishdan oldin, shuning uchun Alertmanager hujjatlashtirilgan chegaralarni aks ettiradi.
- Boshqaruv paneli: `dashboards/grafana/sorafs_capacity_health.json`.
- Ko'rsatkichlar: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Tezkor harakatlar**

1. Kechikishlar hajmini (yagona provayder yoki flot) tekshiring va muhim bo'lmagan replikatsiya vazifalarini to'xtatib turing.
2. Agar kechikish izolyatsiya qilingan bo'lsa, replikatsiya rejalashtiruvchisi orqali muqobil provayderlarga yangi buyurtmalarni vaqtincha qayta tayinlang.

**Triage**

- Orkestrator telemetriyasini qayta urinib ko'ring, ular orqada qolishi mumkin bo'lgan portlashlarni tekshiring.
- Saqlash maqsadlarida etarli joy mavjudligini tasdiqlang (`sorafs_node_capacity_utilisation_percent`).
- So'nggi konfiguratsiya o'zgarishlarini ko'rib chiqing (parchalangan profil yangilanishlari, isbot kadanslari).

**Tuzatish imkoniyatlari**

- Kontentni qayta tarqatish uchun `sorafs_cli` ni `--rebalance` opsiyasi bilan ishga tushiring.
- Ta'sir qilingan provayder uchun replikatsiya ishchilarini gorizontal ravishda masshtablash.
- TTL oynalarini qayta tekislash uchun manifest yangilanishini ishga tushiring.

**Hodisadan keyingi**

- Provayderning to'yinganligi nosozligiga e'tibor qaratgan holda sig'imli matkapni rejalashtiring.
- `docs/source/sorafs_node_client_protocol.md` da replikatsiya SLA hujjatlarini yangilang.

## Orqaga tushgan va SLA buzilishlarini tuzatish

**Aniqlash**

- Ogohlantirishlar:
  - `SoraFSRepairBacklogHigh` (navbat chuqurligi > 50 yoki eng eski navbat yoshi > 10 m uchun 4 soat).
  - `SoraFSRepairEscalations` (> 3 eskalatsiya/soat).
  - `SoraFSRepairLeaseExpirySpike` (> 5 ijara muddati/soat).
  - `SoraFSRetentionBlockedEvictions` (so'nggi 15 m ichida faol ta'mirlash bilan bloklangan saqlash).
- Boshqaruv paneli: `dashboards/grafana/sorafs_capacity_health.json`.

**Tezkor harakatlar**

1. Ta'sir ko'rsatuvchi provayderlarni aniqlang (navbat chuqurligining keskin o'zgarishi) va ular uchun yangi pinlar/ko'paytirish buyurtmalarini to'xtatib turing.
2. Ta'mirlash ishchilarining hayotiyligini tekshiring va agar xavfsiz bo'lsa, ishchilarning bir vaqtda ishlashini oshiring.

**Triage**

- `torii_sorafs_repair_backlog_oldest_age_seconds` ni 4 soatlik SLA oynasi bilan solishtiring.
- `torii_sorafs_repair_lease_expired_total{outcome=...}`-ni yiqilish/soat qiyshayish naqshlari uchun tekshiring.
- Takroriy manifest/provayder juftligi uchun oshirilgan chiptalarni ko'rib chiqing va dalillar to'plamini tekshiring.

**Tuzatish imkoniyatlari**

- to'xtab qolgan ta'mirlash ishchilarini qayta tayinlash yoki qayta ishga tushirish; oddiy da'vo oqimi orqali toza yetim ijara.
- Qo'shimcha SLA bosimiga yo'l qo'ymaslik uchun ta'mirlash paytida drenajni yangi pinlarni gaz bilan o'tkazing.
- Agar kuchayish davom etsa, boshqaruvga o'ting va ta'mirlash auditi artefaktlarini ilova qiling.

## Saqlash / GC tekshiruvi (faqat o'qish uchun)

**Aniqlash**

- Ogohlantirishlar: `SoraFSCapacityPressure` yoki barqaror `torii_sorafs_storage_bytes_used` > 90%.
- Boshqaruv paneli: `dashboards/grafana/sorafs_capacity_health.json`.

**Tezkor harakatlar**

1. Mahalliy saqlash snapshotini ishga tushiring:
   ```bash
   iroha app sorafs gc inspect --data-dir /var/lib/sorafs
   ```
2. Triaj uchun faqat muddati o‘tgan ko‘rinishni suratga oling:
   ```bash
   iroha app sorafs gc dry-run --data-dir /var/lib/sorafs
   ```
3. Tekshirish imkoniyati uchun JSON chiqishlarini voqea chiptasiga biriktiring.

**Triage**

- Qaysi manifest hisoboti `retention_epoch=0` (muddati tugamagan) va muddati o'tgan hisobotlarni tasdiqlang.
- Qaysi cheklov samarali ekanligini bilish uchun GC JSON chiqishida `retention_sources` dan foydalaning.
  ushlab turish (`deal_end`, `governance_cap`, `pin_policy` yoki `unbounded`). Bitim va boshqaruv chegaralari
  `sorafs.retention.deal_end_epoch` manifest metama'lumotlar kalitlari orqali ta'minlanadi va
  `sorafs.retention.governance_cap_epoch`.
- Agar `dry-run` hisobot muddati oʻtgan boʻlsa, lekin sigʻim mahkamlangan boʻlsa, yoʻqligini tekshiring.
  faol ta'mirlash yoki saqlash siyosati blokdan chiqarishni bekor qiladi.
  Imkoniyatlar bilan ishga tushirilgan tozalashlar muddati o'tgan manifestlarni eng kam ishlatilgan buyurtma bo'yicha chiqarib tashlaydi.
  `manifest_id` bog'lovchilar.

**Tuzatish imkoniyatlari**

- GC CLI faqat o'qish uchun mo'ljallangan. Ishlab chiqarishda manifestlar yoki bo'laklarni qo'lda o'chirmang.
- Saqlash siyosatiga tuzatishlar kiritish yoki imkoniyatlarni kengaytirish uchun boshqaruvga o'tish
  muddati o'tgan ma'lumotlar avtomatik ravishda ko'chirishsiz to'planganda.

## Chaos Drill Cadence

- **Har chorakda**: shlyuzning birlashtirilgan uzilishi + orkestrning qayta urinish bo'ron simulyatsiyasi.
- **Yillik ikki marta**: tiklangan ikkita provayderda PoR/PoTR nosozliklari.
- **Oylik spot-check**: sahnalashtirish manifestlari yordamida replikatsiya kechikish stsenariysi.
- Umumiy runbook jurnalida (`ops/drill-log.md`) mashqlarni kuzatib boring:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Jurnalni amalga oshirishdan oldin tasdiqlang:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Kelgusi mashqlar uchun `--status scheduled`, tugallangan mashqlar uchun `pass`/`fail` va amallar ochiq qolganda `follow-up` dan foydalaning.
- Quruq yugurish yoki avtomatlashtirilgan tekshirish uchun `--log` bilan belgilangan manzilni bekor qiling; busiz skript `ops/drill-log.md` yangilanishini davom ettiradi.

## O'limdan keyingi shablon

Har bir P1/P2 hodisasi va xaos matkap retrospektivlari uchun `docs/source/sorafs/postmortem_template.md` dan foydalaning. Shablon vaqt jadvalini, ta'sir miqdorini aniqlash, hissa qo'shadigan omillar, tuzatuvchi harakatlar va keyingi tekshirish vazifalarini o'z ichiga oladi.