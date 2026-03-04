---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Taikai Monitoring Dashboards
description: Portal summary of the viewer/cache Grafana boards that back SN13-C evidence
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Taikai marshrutlash-manifesti (TRM) tayyorligi ikkita Grafana platalariga va ularning
yordamchi ogohlantirishlar. Ushbu sahifada eng muhim voqealar aks ettirilgan
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`,
va `dashboards/alerts/taikai_viewer_rules.yml`, shuning uchun sharhlovchilar kuzatib borishlari mumkin
omborni klonlashsiz.

## Ko'rish paneli (`taikai_viewer.json`)

- **Jonli chekka va kechikish:** Panellar p95/p99 kechikish gistogrammalarini ingl.
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) boshiga
  klaster/oqim. p99 >900ms yoki drift >1,5s uchun tomosha qiling (
  `TaikaiLiveEdgeDrift` ogohlantirish).
- **Segment xatolari:** `taikai_ingest_segment_errors_total{reason}` ni buzadi
  dekodlashda xatoliklarni, naslni takrorlash urinishlarini yoki aniq nomuvofiqliklarni ko'rsatish.
  Bu panel yuqoriga ko'tarilganda, SN13-C hodisalariga skrinshotlarni biriktiring
  "ogohlantirish" bandi.
- **Tomoshabin va CEK salomatligi:** Panellar `taikai_viewer_*` metrika trekidan olingan
  CEK aylanish yoshi, PQ qo'riqchisi aralashmasi, rebufer soni va ogohlantirish roliklari. CEK
  panel boshqaruv yangisini tasdiqlashdan oldin ko'rib chiqadigan aylanish SLAni amalga oshiradi
  taxalluslar.
- **Taxallus telemetriya surati:** `/status → telemetry.taikai_alias_rotations`
  Jadval to'g'ridan-to'g'ri doskada joylashgan bo'lib, operatorlar manifest dayjestlarini tasdiqlashlari mumkin
  boshqaruv dalillarini ilova qilishdan oldin.

## Kesh asboblar paneli (`taikai_cache.json`)

- **Daraj bosimi:** Panellar diagrammasi `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  va `sorafs_taikai_cache_promotions_total`. TRM mavjudligini bilish uchun ulardan foydalaning
  aylanish muayyan darajalarni haddan tashqari yuklaydi.
- **QoS rad etishlari:** kesh bosimida `sorafs_taikai_qos_denied_total` yuzalar
  kuchlarni bostirish; stavka noldan chiqqanda matkap jurnaliga izoh bering.
- **Egressdan foydalanish:** SoraFS chiqishlari Taikai bilan mos kelishini tasdiqlashga yordam beradi
  CMAF oynalari aylanganda tomoshabinlar.

## Ogohlantirishlar va dalillarni olish

- Peyjing qoidalari `dashboards/alerts/taikai_viewer_rules.yml` da ishlaydi va birinchi xaritada
  yuqoridagi panelli biriga (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`,
  `TaikaiCekRotationLag`, sog'liqni saqlash bo'yicha ogohlantirishlar). Har bir ishlab chiqarishni ta'minlash
  klaster ularni Alertmanagerga ulaydi.
- Mashqlar paytida olingan suratlar/skrinshotlar saqlanishi kerak
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` spool fayllari bilan birga va
  `/status` JSON. `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` dan foydalaning
  ijroni umumiy matkap jurnaliga qo'shish uchun.
- Boshqaruv paneli o'zgarganda, JSON faylining SHA-256 dayjestini ichiga kiriting
  auditorlar boshqariladigan Grafana jildiga mos kelishi uchun portalning PR tavsifi
  repo versiyasi.

## Dalillar to'plamining nazorat ro'yxati

SN13-C sharhlari har bir mashq yoki hodisa ro'yxatdagi bir xil artefaktlarni yuborishini kutadi
Taikai anchor runbookda. To'plam bo'lishi uchun ularni quyidagi tartibda oling
boshqaruvni tekshirishga tayyor:

1. Eng oxirgi `taikai-anchor-request-*.json` nusxasini oling,
   `taikai-trm-state-*.json` va `taikai-lineage-*.json` fayllari
   `config.da_ingest.manifest_store_dir/taikai/`. Bu g'altakning artefaktlari isbotlaydi
   qaysi marshrutlash manifest (TRM) va nasl oynasi faol edi. Yordamchi
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   spool fayllarni nusxalaydi, xeshlarni chiqaradi va ixtiyoriy ravishda xulosaga imzo chekadi.
2. Filtrlangan `/v1/status` chiqishini yozib oling
   `.telemetry.taikai_alias_rotations[]` va uni spool fayllari yonida saqlang.
   Sharhlovchilar xabar qilingan `manifest_digest_hex` va oyna chegaralarini solishtiradilar
   nusxalangan spool holati.
3. Yuqorida sanab o‘tilgan ko‘rsatkichlar uchun Prometheus snapshotlarini eksport qiling va skrinshot oling
   tegishli klaster/oqim filtrlari bilan tomoshabin/kesh boshqaruv paneli
   ko'rish. Xom JSON/CSV va skrinshotlarni artefakt jildiga tashlang.
4. Qoidalarga havola qiluvchi Alertmanager hodisa identifikatorlarini (agar mavjud bo'lsa) qo'shing
   `dashboards/alerts/taikai_viewer_rules.yml` va ular avtomatik yopilgan-yopilmaganligiga e'tibor bering
   vaziyat tozalangandan keyin.

`artifacts/sorafs_taikai/drills/<YYYYMMDD>/` ostida hamma narsani saqlang, shuning uchun matkap
audit va SN13-C boshqaruv sharhlari bitta arxivni olishi mumkin.

## Burg'ulash kadansi va ro'yxatga olish

- Har oyning birinchi seshanba kuni soat 15:00 UTC da Taikai ankraj matkapini bajaring.
  Jadval SN13 boshqaruv sinxronlashidan oldin dalillarni yangilab turadi.
- Yuqoridagi artefaktlarni qo'lga kiritgandan so'ng, ijroni umumiy daftarga qo'shing
  `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` bilan. The
  yordamchi `docs/source/sorafs/runbooks-index.md` tomonidan talab qilinadigan JSON yozuvini chiqaradi.
- Runbook indeksidagi arxivlangan artefaktlarni bog'lang va har qanday muvaffaqiyatsizlikni oshiring
  Media Platformasi WG/SRE orqali 48 soat ichida ogohlantirishlar yoki asboblar panelidagi regressiyalar
  kanal.
- Mashqlar sarhisobi skrinshotini saqlang (kechikish, drift, xatolar, CEK aylanishi,
  kesh bosimi) spool to'plami bilan birga, operatorlar qanday qilib aniq ko'rsatishi mumkin
  asboblar paneli mashq paytida o'zini tutdi.

Buning uchun [Taikai Anchor Runbook](./taikai-anchor-runbook.md) ga qayting.
To'liq Sev1 protsedurasi va dalillarni tekshirish ro'yxati. Bu sahifa faqat suratga oladi
SN13-C chiqishdan oldin talab qiladigan asboblar paneliga xos ko'rsatmalar 🈺.