---
lang: zh-hans
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 分析 `iroha_data_model` 构建

要在 `iroha_data_model` 中找到缓慢的构建步骤，请运行帮助程序脚本：

```sh
./scripts/profile_build.sh
```

它运行 `cargo build -p iroha_data_model --timings` 并将时序报告写入 `target/cargo-timings/`。
在浏览器中打开 `cargo-timing.html` 并按持续时间对任务进行排序，以查看哪些包或构建步骤花费最多时间。

使用计时将优化工作集中在最慢的任务上。