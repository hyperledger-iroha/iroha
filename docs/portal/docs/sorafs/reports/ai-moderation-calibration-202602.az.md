---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6066e49abf73a788490f9d6a738d90376b13dde4150509e81431508ffaaaf916
source_last_modified: "2025-12-29T18:16:35.199423+00:00"
translation_last_reviewed: 2026-02-07
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
---

# AI Moderasiya Kalibrləmə Hesabatı - Fevral 2026

Bu hesabat **MINFO-1** üçün ilk kalibrləmə artefaktlarını paketləyir. The
məlumat dəsti, manifest və skorbord 2026-02-05 tarixində hazırlanmışdır,
Nazirlik şurası 2026-02-10 və yüksəklikdə idarəetmə DAG-da lövbər saldı
`912044`.

## Dataset Manifest

- **Dataset arayışı:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **İllik:** `ai-moderation-calibration-202602`
- **Girişlər:** manifest 480, yığın 12,800, metadata 920, audio 160
- **Etiket qarışığı:** təhlükəsiz 68%, şübhəli 19%, eskalasiya 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Paylanma:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Tam manifest `docs/examples/ai_moderation_calibration_manifest_202602.json`-də yaşayır
və buraxılış zamanı əldə edilmiş idarəetmə imzası və qaçışçı hashını ehtiva edir
vaxt.

## Hesab lövhəsinin xülasəsi

Kalibrləmələr opset 17 və deterministik toxum kəməri ilə aparıldı. The
tam hesab lövhəsi JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
hash və telemetriya həzmlərini qeyd edir; aşağıdakı cədvəl ən çox vurğulanır
mühüm ölçülər.

| Model (ailə) | Brier | ECE | AUROC | Precision@Karantin | Recall@Escalate |
| -------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Təhlükəsizlik (görmə) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Təhlükəsizlik (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Qavrama ansamblı (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Birləşdirilmiş ölçülər: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Hökm
kalibrləmə pəncərəsi üzrə paylama 91,2%, karantin 6,8% keçdi,
manifestdə qeyd olunan siyasət gözləntilərinə uyğun olaraq 2.0% yüksəldi
xülasə. Yanlış-müsbət geriləmə sıfırda qaldı və sürüşmə xalı (7,1%)
20% xəbərdarlıq həddindən xeyli aşağı düşdü.

## Eşiklər və Qeydiyyatdan Çıxış

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- İdarəetmə hərəkəti: `MINFO-2026-02-07`
- `2026-02-10T11:33:12Z` ünvanında `ministry-council-seat-03` tərəfindən imzalanmışdır

CI imzalanmış paketi `artifacts/ministry/ai_moderation/2026-02/`-də saxladı
moderasiya runner binaries yanaşı. Manifest həzm və skorbord
yoxlamalar və müraciətlər zamanı yuxarıdakı hashlərə istinad edilməlidir.

## İdarə Panelləri və Xəbərdarlıqlar

Moderasiya SRE-ləri Grafana idarə panelini idxal etməlidir
`dashboards/grafana/ministry_moderation_overview.json` və uyğunluq
`dashboards/alerts/ministry_moderation_rules.yml`-də Prometheus xəbərdarlıq qaydaları
(test əhatə dairəsi `dashboards/alerts/tests/ministry_moderation_rules.test.yml` altında yaşayır).
Bu artefaktlar tövlələr, drift sıçrayışları və karantin üçün xəbərdarlıqlar verir
növbə artımı, çağırılan monitorinq tələblərinə cavab verir
[AI Moderasiya Runner Spesifikasiyası](../../ministry/ai-moderation-runner.md).