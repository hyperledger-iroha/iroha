---
lang: az
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet Relay Təşviq Parlament Paketi

Bu paket Sora Parlamentinin təsdiq etməsi üçün tələb olunan artefaktları özündə cəmləşdirir
avtomatik relay ödənişləri (SNNet-7):

- `reward_config.json` - Norito-seriallaşdırıla bilən mükafat mühərriki konfiqurasiyası, hazır
  `iroha app sorafs incentives service init` tərəfindən qəbul edilməlidir. The
  `budget_approval_id` idarəetmə protokollarında sadalanan hashlara uyğun gəlir.
- `shadow_daemon.json` - benefisiar və replay tərəfindən istehlak edilən istiqraz xəritəsi
  qoşqu (`shadow-run`) və istehsal demonu.
- `economic_analysis.md` - 2025-10 -> 2025-11 üçün ədalətlilik xülasəsi
  kölgə simulyasiyası.
- `rollback_plan.md` - avtomatik ödənişləri söndürmək üçün əməliyyat kitabı.
- Dəstəkləyici artefaktlar: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Dürüstlük Yoxlamaları

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

Həzmləri Parlamentin protokollarında qeyd olunan dəyərlərlə müqayisə edin. Doğrulayın
-də təsvir olunduğu kimi kölgə imzası
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Paketin yenilənməsi

1. Mükafat çəkiləri, əsas ödəniş və ya istənilən vaxt `reward_config.json`-i yeniləyin
   təsdiq hash dəyişikliyi.
2. 60 günlük kölgə simulyasiyasını yenidən işə salın, `economic_analysis.md` ilə yeniləyin
   yeni tapıntılar edin və JSON + ayrılmış imza cütünü təhvil verin.
3. Yenilənmiş paketi Rəsədxananın idarə paneli ilə birlikdə Parlamentə təqdim edin
   yenilənmiş təsdiq istəyərkən ixrac.