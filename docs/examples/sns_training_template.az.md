---
lang: az
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS Təlim Slayd Şablonu

Bu Markdown konturu fasilitatorların uyğunlaşmalı olduğu slaydları əks etdirir
onların dil kohortları. Bu bölmələri Keynote/PowerPoint/Google-a köçürün
Lazım gələrsə, işarə nöqtələrini, skrinşotları və diaqramları sürüşdürün və lokallaşdırın.

## Başlıq slaydı
- Proqram: "Sora Name Service onboarding"
- Altyazı: şəkilçi + dövrü göstərin (məsələn, `.sora — 2026‑03`)
- Təqdimatçılar + mənsubiyyətlər

## KPI oriyentasiyası
- `docs/portal/docs/sns/kpi-dashboard.md` ekran görüntüsü və ya yerləşdirilməsi
- Suffiks filtrlərini izah edən güllə siyahısı, ARPU cədvəli, dondurma izləyicisi
- PDF/CSV ixracı üçün qeydlər

## Manifest həyat dövrü
- Diaqram: qeydiyyatçı → Torii → idarəetmə → DNS/şlüz
- `docs/source/sns/registry_schema.md`-ə istinad edən addımlar
- Annotasiya ilə nümunə manifest çıxarışı

## Məşqləri mübahisələndirin və dondurun
- Qəyyumun müdaxiləsi üçün axın diaqramı
- `docs/source/sns/governance_playbook.md`-ə istinad edən yoxlama siyahısı
- Məsələn, bilet qrafikinin dondurulması

## Əlavə ələ keçirmə
- `cargo xtask sns-annex ... --portal-entry ...`-i göstərən komanda parçası
- `artifacts/sns/regulatory/<suffix>/<cycle>/` altında Grafana JSON arxivi üçün xatırlatma
- `docs/source/sns/reports/.<suffix>/<cycle>.md` ilə əlaqə

## Növbəti addımlar
- Təlim üzrə rəy linki (bax: `docs/examples/sns_training_eval_template.md`)
- Slack/Matrix kanal tutacaqları
- Qarşıdan gələn mərhələ tarixləri