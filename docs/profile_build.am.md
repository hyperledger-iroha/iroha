---
lang: am
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
---

# Profiling `iroha_data_model` Build

To locate slow build steps in `iroha_data_model`, run the helper script:

```sh
./scripts/profile_build.sh
```

This runs `cargo build -p iroha_data_model --timings` and writes timing reports to `target/cargo-timings/`.
Open `cargo-timing.html` in a browser and sort tasks by duration to see which crates or build steps take the most time.

Use the timings to focus optimization efforts on the slowest tasks.
