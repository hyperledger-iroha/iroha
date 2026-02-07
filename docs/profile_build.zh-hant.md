---
lang: zh-hant
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 分析 `iroha_data_model` 構建

要在 `iroha_data_model` 中找到緩慢的構建步驟，請運行幫助程序腳本：

```sh
./scripts/profile_build.sh
```

它運行 `cargo build -p iroha_data_model --timings` 並將時序報告寫入 `target/cargo-timings/`。
在瀏覽器中打開 `cargo-timing.html` 並按持續時間對任務進行排序，以查看哪些包或構建步驟花費最多時間。

使用計時將優化工作集中在最慢的任務上。