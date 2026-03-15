---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cac03d504c6a7dcfacaa4b298e14f0a71ccbcb5ec58f1977b5bf124300c8ec61
source_last_modified: "2026-01-05T09:28:11.865094+00:00"
translation_last_reviewed: 2026-02-07
id: developer-deployment
title: SoraFS Deployment Notes
sidebar_label: Deployment Notes
description: Checklist for promoting the SoraFS pipeline from CI to production.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

# Yerləşdirmə Qeydləri

SoraFS qablaşdırma iş axını determinizmi sərtləşdirir, beləliklə CI-dən keçid
istehsal əsasən əməliyyat qoruyucuları tələb edir. Zaman bu yoxlama siyahısından istifadə edin
alətləri real şlüzlərə və saxlama təminatçılarına yaymaq.

## Uçuşdan əvvəl

- **Registr uyğunlaşdırılması** — chunker profillərini təsdiqləyin və manifestlərə istinad edin
  eyni `namespace.name@semver` tuple (`docs/source/sorafs/chunker_registry.md`).
- **Qəbul siyasəti** — imzalanmış provayder reklamlarını və ləqəb sübutlarını nəzərdən keçirin
  `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) üçün lazımdır.
- **Pin reyestrinin runbook** — `docs/source/sorafs/runbooks/pin_registry_ops.md` saxlayın
  bərpa ssenariləri (ləqəb fırlanma, təkrarlama uğursuzluqları) üçün əlverişlidir.

## Ətraf mühitin konfiqurasiyası

- Şlüzlər sübut axınının son nöqtəsini aktivləşdirməlidir (`POST /v1/sorafs/proof/stream`)
  beləliklə, CLI telemetriya xülasələrini yaya bilər.
- Defolt parametrlərdən istifadə edərək `sorafs_alias_cache` siyasətini konfiqurasiya edin
  `iroha_config` və ya CLI köməkçisi (`sorafs_cli manifest submit --alias-*`).
- Təhlükəsiz məxfi menecer vasitəsilə axın tokenlərini (və ya Torii etimadnaməsini) təmin edin.
- Telemetriya ixracatçılarını aktivləşdirin (`torii_sorafs_proof_stream_*`,
  `torii_sorafs_chunk_range_*`) və onları Prometheus/OTel yığınınıza göndərin.

## Yayımlama strategiyası

1. **Mavi/yaşıl təzahürlər**
   - Hər buraxılış üçün cavabları arxivləşdirmək üçün `manifest submit --summary-out` istifadə edin.
   - Bacarıqları tutmaq üçün `torii_sorafs_gateway_refusals_total`-ə diqqət yetirin
     erkən uyğunsuzluqlar.
2. **Sübutun təsdiqi**
   - `sorafs_cli proof stream`-dəki uğursuzluqları yerləşdirmə blokerləri kimi müalicə edin; gecikmə
     tırmanışlar tez-tez provayderin azaldılmasını və ya səhv konfiqurasiya edilmiş səviyyələri göstərir.
   - `proof verify` CAR-ı təmin etmək üçün post-pin tüstü testinin bir hissəsi olmalıdır
     provayderlər tərəfindən barındırılan hələ də manifest həzminə uyğun gəlir.
3. **Telemetriya panelləri**
   - `docs/examples/sorafs_proof_streaming_dashboard.json`-i Grafana-ə idxal edin.
   - PIN reyestrinin sağlamlığı üçün əlavə panellər yerləşdirin
     (`docs/source/sorafs/runbooks/pin_registry_ops.md`) və yığın diapazonu statistikası.
4. **Çox mənbəli aktivləşdirmə**
   - Mərhələli yayım addımlarını izləyin
     yandırarkən `docs/source/sorafs/runbooks/multi_source_rollout.md`
     orkestrator və auditlər üçün tablo/temetriya artefaktlarını arxivləşdirin.

## Hadisənin idarə edilməsi

- `docs/source/sorafs/runbooks/`-də eskalasiya yollarını izləyin:
  - Gateway kəsintiləri və axın nişanı üçün `sorafs_gateway_operator_playbook.md`
    tükənmə.
  - Replikasiya mübahisələri baş verdikdə `dispute_revocation_runbook.md`.
  - Node səviyyəsində texniki xidmət üçün `sorafs_node_ops.md`.
  - `multi_source_rollout.md` orkestratorun ləğvi, həmyaşıdların qara siyahısı və
    mərhələli buraxılışlar.
- Mövcud vasitəsilə GovernanceLog-da sübut uğursuzluqları və gecikmə anomaliyalarını qeyd edin
  PoR izləyicisi API-ləri beləliklə idarəetmə provayderin fəaliyyətini qiymətləndirə bilər.

## Növbəti addımlar

- Bir dəfə orkestratorun avtomatlaşdırılmasını (`sorafs_car::multi_fetch`) inteqrasiya edin
  çox mənbəli gətirmə orkestratoru (SF-6b) torpaqları.
- SF-13/SF-14 altında PDP/PoTR təkmilləşdirmələrini izləyin; CLI və sənədlər inkişaf edəcək
  bu sübutlar sabitləşdikdən sonra səthin son tarixləri və səviyyə seçimi.

Bu yerləşdirmə qeydlərini sürətli başlanğıc və CI reseptləri ilə birləşdirərək, komandalar
yerli təcrübələrdən istehsal dərəcəli SoraFS boru kəmərlərinə keçə bilər.
təkrarlana bilən, müşahidə edilə bilən proses.