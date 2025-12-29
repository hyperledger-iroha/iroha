---
lang: ru
direction: ltr
source: docs/profile_build.md
status: complete
translator: manual
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-11-02T04:40:28.811778+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/profile_build.md (Profiling iroha_data_model Build) -->

# Профилирование сборки `iroha_data_model`

Чтобы найти медленные этапы сборки `iroha_data_model`, запустите вспомогательный скрипт:

```sh
./scripts/profile_build.sh
```

Скрипт выполняет `cargo build -p iroha_data_model --timings` и записывает отчёты по
времени в `target/cargo-timings/`.
Откройте файл `cargo-timing.html` в браузере и отсортируйте задачи по длительности, чтобы
увидеть, какие crates или шаги сборки занимают больше всего времени.

Используйте эти данные, чтобы сосредоточить усилия по оптимизации на самых медленных
задачах.

