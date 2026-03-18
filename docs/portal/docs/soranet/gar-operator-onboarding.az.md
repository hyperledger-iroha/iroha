---
lang: az
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

SNNet-9 uyğunluq konfiqurasiyasını təkrarlana bilən,
audit üçün əlverişli prosesdir. Hər bir operator üçün onu yurisdiksiyalı araşdırma ilə birləşdirin
eyni həzmlərdən və sübut tərtibindən istifadə edir.

## Addımlar

1. **Konfiquranı yığın**
   - `governance/compliance/soranet_opt_outs.json` idxal edin.
   - `operator_jurisdictions`-i dərc edilmiş attestasiya həzmləri ilə birləşdirin
     [yurisdiksiya araşdırmasında](gar-jurisdictional-review).
2. **Təsdiq edin**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - İsteğe bağlı: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Dəlilləri ələ keçirin**
   - `artifacts/soranet/compliance/<YYYYMMDD>/` altında mağaza:
     - `config.json` (son uyğunluq bloku)
     - `attestations.json` (URI + həzmlər)
     - doğrulama qeydləri
     - imzalanmış PDF-lərə/Norito zərflərinə istinadlar
4. **Aktivləşdirin**
   - Təqdimatı etiketləyin (`gar-opt-out-<date>`), orkestrator/SDK konfiqurasiyalarını yenidən yerləşdirin,
     və gözlənilən yerlərdə qeydlərdə `compliance_*` hadisələrinin yayılmasını təsdiqləyin.
5. **Bağlayın**
   - Sübut paketini İdarəetmə Şurasına təqdim edin.
   - Aktivləşdirmə pəncərəsini + təsdiqləyiciləri GAR jurnalına daxil edin.
   - Yurisdiksiya üzrə yoxlama cədvəlindən növbəti baxış tarixlərini planlaşdırın.

## Tez yoxlama siyahısı

- [ ] `jurisdiction_opt_outs` kanonik kataloqa uyğun gəlir.
- [ ] Attestasiya həzmləri tam olaraq kopyalanır.
- [ ] Doğrulama əmrləri işə salınır və arxivləşdirilir.
- [ ] Sübut dəsti `artifacts/soranet/compliance/<date>/`-də saxlanılır.
- [ ] Təqdimat etiketi + GAR jurnalı yeniləndi.
- [ ] Növbəti baxış xatırlatmaları təyin edildi.

## Həmçinin bax

- [GAR yurisdiksiyasının nəzərdən keçirilməsi](gar-jurisdictional-review)
- [GAR Uyğunluğu üzrə Təlim Kitabı (mənbə)](../../../source/soranet/gar_compliance_playbook.md)