---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI Moderatsiyasi kalibrlash hisoboti - 2026 yil fevral

Ushbu hisobot **MINFO-1** uchun dastlabki kalibrlash artefaktlarini jamlaydi. The
ma'lumotlar to'plami, manifest va reyting jadvali 2026-02-05 da ishlab chiqarilgan,
2026-02-10 da vazirlik kengashi va balandlikda DAG boshqaruvida langar.
`912044`.

## Maʼlumotlar toʻplami manifest

- **Maʼlumotlar majmuasi maʼlumotnomasi:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Yozuvlar:** manifest 480, parcha 12800, metamaʼlumotlar 920, audio 160
- **Yorliq aralashmasi:** xavfsiz 68%, shubhali 19%, eskalatsiya 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Tarqatish:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

To'liq manifest `docs/examples/ai_moderation_calibration_manifest_202602.json` da yashaydi
va boshqaruv imzosi va chop etish vaqtida olingan yuguruvchi xeshini o'z ichiga oladi
vaqt.

## Hisoblar jadvali xulosasi

Kalibrlash opset 17 va deterministik urug'lik quvuri bilan amalga oshirildi. The
JSON to'liq skorbord (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
xeshlar va telemetriya dayjestlarini qayd qiladi; Quyidagi jadval eng ko'p ta'kidlangan
muhim ko'rsatkichlar.

| Model (oila) | Brier | ECE | AUROC | Precision@Karantin | Recall@Escalate |
| -------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Xavfsizlik (ko'rish) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Xavfsizlik (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Pertseptiv ansambli (pertseptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Birlashtirilgan ko'rsatkichlar: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Hukm
kalibrlash oynasi bo'ylab taqsimlash 91,2%, karantin 6,8%,
manifestda qayd etilgan siyosat kutilmalariga mos keladigan 2,0% ga oshadi
xulosa. Yolg'on-musbat kechikish nol darajasida qoldi va drift ball (7,1%)
20% ogohlantirish chegarasidan ancha pastga tushdi.

## Eshiklar va ro'yxatdan o'tish

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Boshqaruv harakati: `MINFO-2026-02-07`
- `ministry-council-seat-03` tomonidan imzolangan `2026-02-10T11:33:12Z`

CI imzolangan paketni `artifacts/ministry/ai_moderation/2026-02/` da saqladi
moderatsiya yuguruvchisi ikkiliklari bilan birga. Manifest dayjesti va reyting jadvali
Yuqoridagi xeshlarga auditlar va apellyatsiyalar paytida murojaat qilish kerak.

## Boshqaruv paneli va ogohlantirishlar

Moderatsiya SRElari Grafana asboblar panelini quyidagi manzilga import qilishlari kerak
`dashboards/grafana/ministry_moderation_overview.json` va moslik
`dashboards/alerts/ministry_moderation_rules.yml` da Prometheus ogohlantirish qoidalari
(sinov qamrovi `dashboards/alerts/tests/ministry_moderation_rules.test.yml` ostida ishlaydi).
Bu artefaktlar yutish joylari, drift spikleri va karantin haqida ogohlantirishlar chiqaradi
navbatdagi o'sish, chaqirilgan monitoring talablarini qondirish
[AI Moderation Runner Spetsifikatsiyasi](../../ministry/ai-moderation-runner.md).