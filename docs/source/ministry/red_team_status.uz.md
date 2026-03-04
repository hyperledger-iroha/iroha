---
lang: uz
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# Vazirlikning Qizil jamoasi maqomi

Bu sahifa [Moderation Red-Team Plan](moderation_red_team_plan.md)ni toʻldiradi
yaqin muddatli mashq taqvimini, dalillar to'plamlarini va tuzatishni kuzatish orqali
holati. Qo'lga olingan artefaktlar bilan birga har bir yugurishdan keyin uni yangilang
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## Kelgusi mashqlar

| Sana (UTC) | Ssenariy | Ega(lar)i | Dalillarni tayyorlash | Eslatmalar |
|------------|---------|----------|---------------|-------|
| 2026-11-12 | **Koʻrni bogʻlash operatsiyasi** — Taikai aralash rejimli kontrabanda repetisiyasi, shlyuzni pasaytirish urinishlari | Xavfsizlik muhandisligi (Miyu Sato), Vazirlik operatsiyalari (Liam O'Konnor) | `scripts/ministry/scaffold_red_team_drill.py` to'plami `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + sahnalashtirish katalogi `artifacts/ministry/red-team/2026-11/operation-blindfold/` | Mashqlar GAR/Taikai bir-biriga o'xshash va DNS o'chirilishi; ishga tushirishdan oldin rad etilgan Merkle snapshotini va asboblar paneli yozib olingandan keyin `export_red_team_evidence.py` ishga tushirishni talab qiladi. |

## Oxirgi mashq surati

| Sana (UTC) | Ssenariy | Dalillar to'plami | Tuzatish va kuzatish |
|------------|---------|-----------------|--------------------------|
| 2026-08-18 | **Operation SeaGlass** — Gateway kontrabandasi, boshqaruvni takrorlash va ogohlantirish mashxurligi | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana eksporti, Alertmanager jurnallari, `seaglass_evidence_manifest.json`) | **Ochiq:** muhrni takrorlashni avtomatlashtirish (`MINFO-RT-17`, egasi: Governance Ops, muddati 2026-09-05); PIN asboblar paneli SoraFS (`MINFO-RT-18`, Kuzatish mumkinligi, 2026-08-25 tugaydi) ga qotib qoladi. **Yopiq:** jurnal shabloni Norito manifest xeshlarini olib yurish uchun yangilandi. |

## Kuzatuv va asboblar

- In'ektsion qadoqlash uchun `scripts/ministry/moderation_payload_tool.py` dan foydalaning
  har bir stsenariy bo'yicha foydali yuklar va rad etish yamoqlari.
- `scripts/ministry/export_red_team_evidence.py` orqali asboblar paneli/jurnalga yozib olish
  Har bir mashqdan so'ng darhol dalil manifestida imzolangan xeshlar mavjud.
- CI qo'riqchisi `ci/check_ministry_red_team.sh` matkap hisobotlarini bajarishga majbur qiladi
  to'ldiruvchi matnni o'z ichiga olmaydi va havola qilingan artefaktlar ilgari mavjud
  birlashish.

Havola qilingan jonli xulosa uchun `status.md` (§ *Vazirlikning qizil jamoasi holati*) ga qarang.
haftalik muvofiqlashtirish chaqiruvlarida.