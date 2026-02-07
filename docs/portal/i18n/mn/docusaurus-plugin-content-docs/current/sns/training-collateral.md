---
id: training-collateral
lang: mn
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> `docs/source/sns/training_collateral.md` толь. Мэдээлэл өгөхдөө энэ хуудсыг ашиглана уу
> бүртгэгч, DNS, асран хамгаалагч, санхүүгийн багууд дагавар бүрийн өмнө.

## 1. Сургалтын хөтөлбөрийн агшин зураг

| Зам | Зорилго | Урьдчилан унших |
|-------|------------|-----------|
| Бүртгэлийн үйл ажиллагаа | Манифест илгээх, KPI хяналтын самбарыг хянах, алдааг нэмэгдүүлэх. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS & гарц | Шийдвэрлэгч араг ясыг түрхэж, хөлдөлт/буцах дасгалыг давт. | `sorafs/gateway-dns-runbook`, шууд горимын бодлогын дээж. |
| Асран хамгаалагч, зөвлөл | Маргааныг шийдвэрлэх, засаглалын нэмэлт, бүртгэлийн хавсралтыг шинэчлэх. | `sns/governance-playbook`, няравын онооны карт. |
| Санхүү ба аналитик | ARPU/бөөнөөр хэмжигдэхүүнийг авах, хавсралтын багцуудыг нийтлэх. | `finance/settlement-iso-mapping`, KPI хяналтын самбар JSON. |

### Модулийн урсгал

1. **M1 — KPI чиг баримжаа (30мин):** Алхах дагавар шүүлтүүр, экспорт болон оргон зайлсан
   тоолуурыг хөлдөөх. Хүргэгдэх боломжтой: SHA-256 тоймтой PDF/CSV агшин.
2. **M2 — Манифестын амьдралын мөчлөг (45мин):** Бүртгүүлэгчийн манифестуудыг бүтээх, баталгаажуулах,
   `scripts/sns_zonefile_skeleton.py`-ээр дамжуулан шийдэгч араг яс үүсгэх. Хүргэлт:
   араг яс + GAR нотлох баримтыг харуулсан git diff.
3. **M3 — Маргаантай дасгалууд (40мин):** Асран хамгаалагчийг хөлдөөх + давж заалдах, барьж авах
   `artifacts/sns/training/<suffix>/<cycle>/logs/` доор асран хамгаалагч CLI бүртгэлүүд.
4. **M4 — Хавсралтын зураг авах (25мин):** JSON хяналтын самбарыг экспортод ажиллуулна:

   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Хүргэгдэх боломжтой: шинэчилсэн хавсралт Markdown + зохицуулалтын + портал тэмдэглэлийн блокууд.

## 2. Локалчлалын ажлын урсгал

- Хэл: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, I18NI00000000.
- Орчуулга бүр эх файлын хажууд байдаг
  (`docs/source/sns/training_collateral.<lang>.md`). `status` + шинэчлэх
  Сэргээх дараа `translation_last_reviewed`.
- Хэл тус бүрд хамаарах хөрөнгө
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (слайд/, ажлын ном/,
  бичлэг/, бүртгэл/).
- Англи хэлийг засварласны дараа `python3 scripts/sync_docs_i18n.py --lang <code>` ажиллуулна уу
  эх сурвалж, ингэснээр орчуулагчид шинэ хэшийг харна.

### Хүргэлтийн хяналтын хуудас

1. Орчуулгын stub-г (`status: complete`) нутагшуулсаны дараа шинэчилнэ үү.
2. Слайдуудыг PDF формат руу экспортлох ба хэл бүрийн `slides/` лавлах руу байршуулах.
3. ≤10мин KPI-ийн мэдээллийг бүртгэх; хэлний бүдүүвчээс холбоос.
4. Slide/workbook агуулсан `sns-training` шошготой файлын удирдлагын тасалбар
   тойм, бичлэгийн холбоос, хавсралт нотлох баримт.

## 3. Сургалтын хөрөнгө

- Слайдын тойм: `docs/examples/sns_training_template.md`.
- Ажлын дэвтрийн загвар: `docs/examples/sns_training_workbook.md` (оролцогч бүрт нэг).
- Урилга + сануулагч: `docs/examples/sns_training_invite_email.md`.
- Үнэлгээний маягт: `docs/examples/sns_training_eval_template.md` (хариулт
  `artifacts/sns/training/<suffix>/<cycle>/feedback/` дор архивлагдсан).

## 4. Хуваарь, хэмжүүр

| Цикл | Цонх | хэмжүүр | Тэмдэглэл |
|-------|--------|---------|-------|
| 2026-03 | KPI тоймыг нийтлэх | Ирц %, хавсралтын тоймыг бүртгэсэн | `.sora` + `.nexus` когорт |
| 2026-06 | Өмнө нь `.dao` GA | Санхүүгийн бэлэн байдал ≥90% | Бодлогын шинэчлэлийг оруулах |
| 2026-09 | Өргөтгөл | Маргааны өрөм <20мин, хавсралт SLA ≤2days | SN-7 урамшуулалд нийцүүлэх |

Нэргүй санал хүсэлтийг `docs/source/sns/reports/sns_training_feedback.md` дээр бичээрэй
Тиймээс дараагийн бүлэг нь нутагшуулах болон лабораторийг сайжруулах боломжтой.