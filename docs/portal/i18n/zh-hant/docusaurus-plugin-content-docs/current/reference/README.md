---
slug: /reference
lang: zh-hant
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Reference Index
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

本節匯總了 Iroha 的“將其作為規範閱讀”材料。這些頁面保持穩定，即使
指南和教程不斷發展。

## 今天可用

- **Norito編解碼器概述** – `reference/norito-codec.md`直接鏈接到權威
  填充入口表時的 `norito.md` 規範。
- **Torii OpenAPI** – `/reference/torii-openapi` 使用以下方式呈現最新的 Torii REST 規範
  重新記錄。使用 `npm run sync-openapi -- --version=current --latest` 重新生成規範（添加
  `--mirror=<label>` 將快照複製到其他歷史版本中）。
- **配置表** – 完整的參數目錄保存在
  `docs/source/references/configuration.md`。在門戶發布自動導入之前，請參考
  用於精確默認值和環境覆蓋的 Markdown 文件。
- **文檔版本控制** – 導航欄版本下拉列表公開了使用創建的凍結快照
  `npm run docs:version -- <label>`，可以輕鬆比較不同版本的指南。

## 即將推出

- **Torii REST 參考** – OpenAPI 定義將通過同步到此部分
  啟用管道後，`docs/portal/scripts/sync-openapi.mjs`。
- **CLI 命令索引** – 生成的命令矩陣（鏡像 `crates/iroha_cli/src/commands`）
  將與規範示例一起登陸此處。
- **IVM ABI 表** – 指針類型和系統調用矩陣（在 `crates/ivm/docs` 下維護）
  一旦文檔生成作業連接完畢，將呈現到門戶中。

## 保持該索引最新

添加新的參考資料時——生成的 API 文檔、編解碼器規範、配置矩陣——放置
`docs/portal/docs/reference/` 下的頁面並在上面鏈接。如果頁面是自動生成的，請注意
同步腳本，以便貢獻者知道如何刷新它。這使參考樹保持有用，直到
完全自動生成的導航土地。