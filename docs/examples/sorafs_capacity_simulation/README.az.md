---
lang: az
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Tutum Simulyasiyası Alət dəsti

Bu kataloq SF-2c tutum bazarı üçün təkrarlana bilən artefaktları göndərir
simulyasiya. Alətlər dəsti kvota danışıqlarını, uğursuzluqla işləməyi və kəsişməni həyata keçirir
istehsal CLI köməkçiləri və yüngül analiz skriptindən istifadə edərək remediasiya.

## İlkin şərtlər

- İş sahəsi üzvləri üçün `cargo run`-i işə sala bilən Rust alətlər silsiləsi.
- Python 3.10+ (yalnız standart kitabxana).

## Sürətli başlanğıc

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` skripti qurmaq üçün `sorafs_manifest_stub capacity`-i çağırır:

- Kvota danışıqları qurğusu üçün deterministik provayder bəyannamələri.
- Danışıqlar ssenarisinə uyğun təkrarlama sifarişi.
- İstifadə etmə pəncərəsi üçün telemetriya snapshotları.
- Kəsmə sorğusunu tutan mübahisə yükü.

Skript Norito bayt (`*.to`), base64 faydalı yükləri (`*.b64`), Torii sorğunu yazır.
gövdələr və seçilmiş artefakt altında insan tərəfindən oxuna bilən xülasələr (`*_summary.json`)
kataloq.

`analyze.py` yaradılan xülasələri istehlak edir, ümumiləşdirilmiş hesabat hazırlayır
(`capacity_simulation_report.json`) və Prometheus mətn faylı yayır
(`capacity_simulation.prom`) daşıyan:

- `sorafs_simulation_quota_*` danışıq qabiliyyətini və bölüşdürməni təsvir edən ölçülər
  provayder başına pay.
- `sorafs_simulation_failover_*` iş vaxtı deltalarını və seçilmişləri vurğulayan ölçü cihazları
  əvəz provayderi.
- `sorafs_simulation_slash_requested` çıxarılan remediasiya faizini qeyd edir
  mübahisənin faydalı yükündən.

Grafana paketini `dashboards/grafana/sorafs_capacity_simulation.json`-də idxal edin
və onu yaradılan mətn faylını qıran Prometheus məlumat mənbəyinə yönəldin (üçün
misal node-exporter mətn faylı kollektoru vasitəsilə). Runbook ünvanında
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` tam şəkildə gəzir
Prometheus konfiqurasiya məsləhətləri daxil olmaqla iş axını.

## Qurğular

- `scenarios/quota_negotiation/` — Provayder bəyannaməsinin xüsusiyyətləri və replikasiya qaydası.
- `scenarios/failover/` — Əsas kəsinti və uğursuzluq qaldırma üçün telemetriya pəncərələri.
- `scenarios/slashing/` — Eyni replikasiya sırasına istinad edən mübahisə spesifikasiyası.

Bu qurğular `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`-də təsdiq edilmişdir
onların CLI sxemi ilə sinxron qalmasını təmin etmək üçün.