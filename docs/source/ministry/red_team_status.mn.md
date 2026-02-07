---
lang: mn
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

# Яамны Улаан-Багийн статус

Энэ хуудас нь [Moderation Red-Team Plan](moderation_red_team_plan.md)-ийг нөхөж байна.
ойрын хугацааны өрөмдлөгийн хуанли, нотлох баримтын багц, засч залруулах ажлыг хянах замаар
статус. Доор авсан олдворуудын хажуугаар гүйлтийн дараа үүнийг шинэчил
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## Удахгүй болох дасгалууд

| Огноо (UTC) | Хувилбар | Эзэмшигч(үүд) | Нотлох баримт бэлтгэх | Тэмдэглэл |
|------------|---------|----------|---------------|-------|
| 2026-11-12 | **Хараа боох ажиллагаа** — Тайкайгийн холимог горимын хууль бус наймааны сургуулилт, гарцыг бууруулах оролдлого | Хамгаалалтын инженерчлэл (Мию Сато), яамны үйл ажиллагаа (Лиам О’Коннор) | `scripts/ministry/scaffold_red_team_drill.py` багц `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + үе шатны лавлах `artifacts/ministry/red-team/2026-11/operation-blindfold/` | Дасгалууд GAR/Taikai давхцал дээр нэмээд DNS-ийн шилжүүлэлт; эхлүүлэхийн өмнө denylist Merkle хормын хувилбарыг шаардаж, `export_red_team_evidence.py` хяналтын самбарыг авсны дараа ажиллуулна. |

## Сүүлийн өрмийн агшин зураг

| Огноо (UTC) | Хувилбар | Нотлох баримтын багц | Засвар, хяналт |
|------------|---------|-----------------|--------------------------|
| 2026-08-18 | **Operation SeaGlass** — Gateway хууль бусаар хил давуулах, засаглалын давталт, сэрэмжлүүлэх сургуулилт | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana экспорт, Alertmanager бүртгэл, `seaglass_evidence_manifest.json`) | **Нээлттэй:** лацыг дахин тоглуулах автоматжуулалт (`MINFO-RT-17`, эзэмшигч: Засаглалын үйл ажиллагаа, 2026-09-05-нд дуусах); хяналтын самбар SoraFS (`MINFO-RT-18`, Ажиглах боломжтой, 2026-08-25-ны өдөр) хүртэл хөлддөг. **Хаалттай:** бүртгэлийн дэвтрийн загварыг Norito манифест хэш агуулсан байхаар шинэчилсэн. |

## Хяналт ба багаж хэрэгсэл

- Тарилгын савлахдаа `scripts/ministry/moderation_payload_tool.py` хэрэглэнэ
  нэг хувилбарт даацын ачаалал болон үгүйсгэх засварууд.
- `scripts/ministry/export_red_team_evidence.py`-ээр хяналтын самбар/логийн бичлэг хийх
  дасгал бүрийн дараа шууд нотлох баримт бичигт гарын үсэг зурсан хэш агуулагдах болно.
- CI хамгаалагч `ci/check_ministry_red_team.sh` дасгалын тайлангуудыг мөрддөг.
  орлуулагч текст агуулаагүй бөгөөд лавласан олдворууд өмнө нь байдаг
  нэгтгэх.

Лавлагаатай шууд тоймыг `status.md` (§ *Яамны улаан багийн статус*) үзнэ үү.
долоо хоног бүрийн зохицуулалтын дуудлагад.