---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SF-6 Security Review
summary: Findings and follow-up items from the independent assessment of keyless signing, proof streaming, and manifest submission pipelines.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-6 安全審查

**評估窗口：** 2026-02-10 → 2026-02-18  
**審核線索：** 安全工程協會 (`@sec-eng`)、工具工作組 (`@tooling-wg`)  
**範圍：** SoraFS CLI/SDK（`sorafs_cli`、`sorafs_car`、`sorafs_manifest`）、證明流 API、Torii 清單處理、Sigstore/OIDC 集成、CI 發布鉤子。  
**文物：**  
- CLI 源和測試 (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii 艙單/證明處理程序 (`crates/iroha_torii/src/sorafs/api.rs`)  
- 發布自動化（`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`）  
- 確定性奇偶校驗工具（`crates/sorafs_car/tests/sorafs_cli.rs`、[SoraFS Orchestrator GA 奇偶校驗報告](./orchestrator-ga-parity.md)）

## 方法論

1. **威脅建模研討會**映射了開發人員工作站、CI 系統和 Torii 節點的攻擊者能力。  
2. **代碼審查**重點關注憑證表面（OIDC 令牌交換、無密鑰簽名）、Norito 清單驗證和證明流背壓。  
3. **動態測試** 使用奇偶校驗工具和定制模糊驅動器重放夾具清單和模擬故障模式（令牌重放、清單篡改、截斷的證明流）。  
4. **配置檢查**驗證了 `iroha_config` 默認值、CLI 標誌處理和發布腳本，以確保確定性、可審核的運行。  
5. **流程訪談** 與工具工作組發布所有者確認了修復流程、升級路徑和審計證據捕獲。

## 調查結果摘要

|身份證 |嚴重性 |面積 |尋找|分辨率|
|----|----------|------|---------|------------|
| SF6-SR-01 |高|無鑰匙簽名 | OIDC 令牌受眾默認值隱含在 CI 模板中，存在跨租戶重放的風險。 |在發布掛鉤和 CI 模板中添加了顯式 `--identity-token-audience` 強制（[發布過程](../developer-releases.md)、`docs/examples/sorafs_ci.md`）。現在，當觀眾被忽略時，CI 就會失敗。 |
| SF6-SR-02 |中等|證明流 |背壓路徑接受無限的訂閱者緩衝區，從而導致內存耗盡。 | `sorafs_cli proof stream` 通過確定性截斷強制限制通道大小，記錄 Norito 摘要併中止流； Torii 鏡像已更新以綁定響應塊 (`crates/iroha_torii/src/sorafs/api.rs`)。 |
| SF6-SR-03 |中等|清單提交 |當 `--plan` 不存在時，CLI 接受清單而不驗證嵌入的塊計劃。 |除非提供 `--expect-plan-digest`，否則 `sorafs_cli manifest submit` 現在會重新計算和比較 CAR 摘要，拒絕不匹配並顯示修復提示。測試涵蓋成功/失敗案例 (`crates/sorafs_car/tests/sorafs_cli.rs`)。 |
| SF6-SR-04 |低|審計追踪|發布清單缺少用於安全審查的簽名批准日誌。 |添加了 [發布流程](../developer-releases.md) 部分，要求在 GA 之前附加審核備忘錄哈希值和簽核票 URL。 |

所有高/中發現結果均在審查窗口期間修復，並通過現有的奇偶校驗工具進行驗證。不存在任何潛在的關鍵問題。

## 控制驗證

- **憑證範圍：** 默認 CI 模板現在強制要求明確的受眾和頒發者斷言； CLI 和發布助手都會快速失敗，除非 `--identity-token-audience` 伴隨 `--identity-token-provider`。  
- **確定性重播：**更新的測試涵蓋正/負清單提交流程，確保不匹配的摘要保持非確定性故障，並在接觸網絡之前浮出水面。  
- **證明流式傳輸背壓：** Torii 現在通過有界通道流式傳輸 PoR/PoTR 項目，並且 CLI 僅保留截斷的延遲樣本 + 五個故障示例，防止無限制的訂閱者增長，同時保持確定性摘要。  
- **可觀察性：** 證明流計數器 (`torii_sorafs_proof_stream_*`) 和 CLI 摘要捕獲中止原因，為操作員提供審計麵包屑。  
- **文檔：** 開發人員指南（[開發人員索引](../developer-index.md)、[CLI 參考](../developer-cli.md)）調用了安全敏感標誌和升級工作流程。

## 發布清單添加內容

發布經理在提升 GA 候選人時**必須**附上以下證據：

1. 最新安全審查備忘錄（本文檔）的哈希值。  
2. 鏈接到所跟踪的修復票證（例如，`governance/tickets/SF6-SR-2026.md`）。  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` 的輸出顯示明確的受眾/發行者參數。  
4. 從奇偶校驗工具 (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`) 捕獲日誌。  
5. 確認 Torii 發行說明包含有界證明流遙測計數器。

未能收集上述工件會阻止 GA 簽署。

**參考人工製品哈希值（2026 年 2 月 20 日簽署）：**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## 出色的後續行動

- **威脅模型刷新：** 每季度或在添加主要 CLI 標誌之前重複此審核。  
- **模糊測試覆蓋範圍：** 證明流傳輸編碼通過 `fuzz/proof_stream_transport` 進行模糊測試，涵蓋身份、gzip、deflate 和 zstd 有效負載。  
- **事件演練：** 安排操作員練習模擬令牌洩露和清單回滾，確保文檔反映實踐的程序。

## 批准

- 安全工程協會代表：@sec-eng (2026-02-20)  
- 工具工作組代表：@tooling-wg (2026-02-20)

將簽署的批准與發布工件包一起存儲。