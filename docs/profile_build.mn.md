---
lang: mn
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

`iroha_data_model` дээр удаан бүтээх алхмуудыг олохын тулд туслах скриптийг ажиллуулна уу:

```sh
./scripts/profile_build.sh
```

Энэ нь `cargo build -p iroha_data_model --timings`-ийг ажиллуулж, `target/cargo-timings/` руу цагийн тайланг бичдэг.
`cargo-timing.html`-г хөтчөөр нээгээд аль хайрцаг эсвэл бүтээх алхмууд хамгийн их цаг зарцуулж байгааг харахын тулд даалгавруудыг хугацаанд нь ангилна.

Оновчлолын хүчин чармайлтыг хамгийн удаан даалгаврууд дээр төвлөрүүлэхийн тулд цагийг ашигла.