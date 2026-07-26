---
lang: az
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Profilin yaradılması `iroha_data_model` Quruluş

`iroha_data_model`-də yavaş qurma addımlarını tapmaq üçün köməkçi skripti işə salın:

```sh
./scripts/profile_build.sh
```

Bu, `cargo build -p iroha_data_model --timings` ilə işləyir və vaxt hesabatlarını `target/cargo-timings/`-ə yazır.
Brauzerdə `cargo-timing.html`-i açın və hansı qutuların və ya quraşdırma addımlarının daha çox vaxt apardığını görmək üçün tapşırıqları müddətə görə çeşidləyin.

Optimallaşdırma səylərini ən yavaş tapşırıqlara yönəltmək üçün vaxtlardan istifadə edin.