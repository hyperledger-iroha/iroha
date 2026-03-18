---
lang: uz
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Profiling `iroha_data_model` Build

`iroha_data_model` da sekin qurish bosqichlarini topish uchun yordamchi skriptni ishga tushiring:

```sh
./scripts/profile_build.sh
```

Bu `cargo build -p iroha_data_model --timings` ishlaydi va vaqt hisobotlarini `target/cargo-timings/` ga yozadi.
Brauzerda `cargo-timing.html` ni oching va qaysi qutilar yoki qurilish bosqichlari ko'p vaqt talab qilinishini ko'rish uchun vazifalarni davomiyligi bo'yicha tartiblang.

Optimallashtirish harakatlarini eng sekin vazifalarga qaratish uchun vaqtlardan foydalaning.