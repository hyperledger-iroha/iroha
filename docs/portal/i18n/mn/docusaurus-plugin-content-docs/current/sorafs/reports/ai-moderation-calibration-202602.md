---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI Зохицуулах шалгалт тохируулгын тайлан - 2026 оны 2-р сар

Энэхүү тайланд **MINFO-1**-ийн нээлтийн шалгалт тохируулгын олдворуудыг багцалсан болно. The
өгөгдлийн багц, манифест болон онооны самбарыг 2026-02-05-нд гаргасан бөгөөд
Яамны зөвлөл 2026-02-10-нд, засаглалын DAG-д өндөрт бэхлэгдсэн.
`912044`.

## Өгөгдлийн багцын манифест

- **Өгөгдлийн багцын лавлагаа:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Слаг:** `ai-moderation-calibration-202602`
- **Оролт:** манифест 480, хэсэг 12,800, мета өгөгдөл 920, аудио 160
- **Шошгоны холимог:** аюулгүй 68%, сэжигтэй 19%, өсөлт 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- ** Түгээлт:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Бүрэн манифест нь `docs/examples/ai_moderation_calibration_manifest_202602.json` дээр амьдардаг
ба хувилбарын үед авсан засаглалын гарын үсэг болон гүйгч хэшийг агуулсан
цаг.

## Онооны самбарын хураангуй

Шалгалт тохируулга нь opset 17 болон детерминист үрийн дамжуулах хоолойгоор хийгдсэн. The
иж бүрэн онооны самбар JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
хэш болон телеметрийн мэдээг бүртгэдэг; Доорх хүснэгтэд хамгийн их онцолсон болно
чухал хэмжүүрүүд.

| Загвар өмсөгч (гэр бүл) | Бриер | ECE | AUROC | Нарийвчлал @ Хорио цээр | Recall@Escalate |
| -------------- | ----- | --- | ----- | ------------------- | --------------- |
| ViT-H/14 Аюулгүй байдал (алсын хараа) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Аюулгүй байдал (олон төрлийн) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Мэдрэхүйн чуулга (перцепт) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

Хосолсон хэмжигдэхүүн: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Шийдвэр
шалгалт тохируулгын цонхоор тархалт 91.2%, хорио цээрийн дэглэм 6.8%,
манифестэд бичигдсэн бодлогын хүлээлттэй таарч, 2.0%-иар өссөн байна
хураангуй. Хуурамч эерэг хоцрогдол нь тэг хэвээр үлдсэн бөгөөд зөрөх оноо (7.1%)
дохиоллын 20%-ийн босгоос нэлээд доогуур унав.

## Босго & Гарах

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Засаглалын хөдөлгөөн: `MINFO-2026-02-07`
- `ministry-council-seat-03` `2026-02-10T11:33:12Z` хаягаар гарын үсэг зурсан

CI гарын үсэг зурсан багцыг `artifacts/ministry/ai_moderation/2026-02/`-д хадгалсан
moderation runner хоёртын файлуудын хажууд. Манифест тойм ба онооны самбар
аудит болон давж заалдах үед дээрх хэшүүдийг иш татсан байх ёстой.

## Хяналтын самбар ба сэрэмжлүүлэг

Зохицуулах SRE нь Grafana хяналтын самбарыг дараах хаягаар импортлох ёстой
`dashboards/grafana/ministry_moderation_overview.json` ба тохирох
`dashboards/alerts/ministry_moderation_rules.yml` дээрх Prometheus дохиоллын дүрэм
(туршилтын хамрах хүрээ нь `dashboards/alerts/tests/ministry_moderation_rules.test.yml` дагуу ажилладаг).
Эдгээр олдворууд нь залгих лангуу, шилжилт хөдөлгөөн болон хорио цээрийн талаар сэрэмжлүүлэг гаргадаг.
дарааллын өсөлт, хяналтын шаардлагыг хангасан
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md).