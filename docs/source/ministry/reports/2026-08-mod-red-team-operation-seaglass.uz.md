---
lang: uz
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# Red-Team Drill — Operation SeaGlass

- **Matkap ID:** `20260818-operation-seaglass`
- **Sana va oyna:** `2026-08-18 09:00Z – 11:00Z`
- **Ssenariy sinfi:** `smuggling`
- **Operatorlar:** `Miyu Sato, Liam O'Connor`
- **Boshqaruv panellari topshirilgandan muzlatilgan:** `364f9573b`
- **Dalillar to'plami:** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (ixtiyoriy):** `not pinned (local bundle only)`
- **Tegishli yoʻl xaritasi elementlari:** `MINFO-9`, shuningdek bogʻlangan kuzatuvlar `MINFO-RT-17` / `MINFO-RT-18`.

## 1. Maqsadlar va kirish shartlari

- **Asosiy maqsadlar**
  - Kontrabandaga urinish paytida yuk to'g'risida ogohlantirishlar paytida rad etilgan TTL ijrosini va shlyuz karantinini tasdiqlang.
  - Boshqaruvni takrorlashni aniqlashni tasdiqlang va moderatsiya ish kitobida ogoh bo'ling.
- **Tasdiqlangan shartlar**
  - `emergency_canon_policy.md` versiyasi `v2026-08-seaglass`.
  - `dashboards/grafana/ministry_moderation_overview.json` digest `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`.
  - Qo'ng'iroq paytida vakolatni bekor qilish: `Kenji Ito (GovOps pager)`.

## 2. Bajarish xronologiyasi

| Vaqt tamg'asi (UTC) | Aktyor | Harakat / Buyruq | Natija / Eslatmalar |
|----------------|-------|------------------|----------------|
| 09:00:12 | Miyu Sato | `364f9573b` da `scripts/ministry/export_red_team_evidence.py --freeze-only` orqali muzlatilgan asboblar paneli/ogohlantirishlar | Asosiy chiziq `dashboards/` | ostida saqlanadi va saqlanadi
| 09:07:44 | Liam O'Konnor | Eʼlon qilingan rad etish roʻyxatining surati + `sorafs_cli ... gateway update-denylist --policy-tier emergency` bilan sahnalashtirish uchun GAR bekor qilish | Surat qabul qilindi; Alertmanager | da qayd qilingan oynani bekor qilish
| 09:17:03 | Miyu Sato | AOK qilingan kontrabanda yuki + `moderation_payload_tool.py --scenario seaglass` yordamida boshqaruvni takrorlash | Ogohlantirish 3m12 soniyadan so'ng paydo bo'ldi; boshqaruv takrori belgilandi |
| 09:31:47 | Liam O'Konnor | Dalillarni eksport qilish va muhrlangan manifest `seaglass_evidence_manifest.json` | Dalillar toʻplami va `manifests/` | ostida saqlangan xeshlar

## 3. Kuzatishlar va ko'rsatkichlar

| Metrik | Maqsad | Kuzatilgan | O'tdi/qobiliyatsiz | Eslatmalar |
|--------|--------|----------|-----------|-------|
| Ogohlantirish javobining kechikishi | = 0,98 | 0,992 | ✅ | Kontrabanda va takroriy yuklar aniqlandi |
| Gateway anomaliyasini aniqlash | Ogohlantirish ishga tushirildi | Ogohlantirish o'chirildi + avtomatik karantin | ✅ | Budjet tugashidan oldin karantin qo‘llaniladi |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. Topilmalar va tuzatish

| Jiddiylik | Topish | Egasi | Maqsadli sana | Holat / Havola |
|----------|---------|-------|-------------|---------------|
| Yuqori | Boshqaruv qayta o'ynash ogohlantirishi ishga tushirildi, lekin kutish ro'yxatini o'zgartirish ishga tushirilganda SoraFS muhri 2 m ga kechiktirildi | Governance Ops (Liam O'Connor) | 2026-09-05 | `MINFO-RT-17` ochiq — uzilish yoʻliga replay muhri avtomatizatsiyasini qoʻshing |
| O'rta | Boshqaruv panelini muzlatish SoraFS ga mahkamlanmagan; operatorlar mahalliy to'plamga tayangan | Kuzatish qobiliyati (Miyu Sato) | 2026-08-25 | `MINFO-RT-18` ochiq — keyingi mashqdan oldin imzolangan CID bilan `dashboards/*` dan SoraFS gacha pin |
| Past | CLI jurnali birinchi o'tishda Norito manifest xeshini o'tkazib yubordi | Vazirlik operatsiyalari (Kenji Ito) | 2026-08-22 | Matkap paytida o'rnatiladi; jurnal jurnalida yangilangan andoza |Kalibrlash qanday namoyon boʻlishini hujjatlashtiring, rad etish qoidalari yoki SDK/asboblar oʻzgarishi kerak. GitHub/Jira muammolariga havola va bloklangan/bloklanmagan holatlarni qayd qiling.

## 5. Boshqaruv va tasdiqlash

- **Hodisa komandirining imzosi:** `Miyu Sato @ 2026-08-18T11:22Z`
- **Boshqaruv kengashining ko'rib chiqish sanasi:** `GovOps-2026-08-22`
- **Kuzatuv roʻyxati:** `[x] status.md updated`, `[x] roadmap row updated`, `[x] transparency packet annotated`

## 6. Qo'shimchalar

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

Dalillar toʻplamiga va SoraFS suratiga yuklangandan soʻng har bir biriktirmani `[x]` bilan belgilang.

---

_Oxirgi yangilangan: 2026-08-18_