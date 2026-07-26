---
lang: zh-hant
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-12-29T18:16:35.071058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 文檔預覽反饋表（W1 合作夥伴 Wave）

從 W1 審閱者收集反饋時使用此模板。複製它每
合作夥伴，填寫元數據，並將完成的副本存儲在
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。

## 審閱者元數據

- **合作夥伴 ID：** `partner-w1-XX`
- **請求票：** `DOCS-SORA-Preview-REQ-PXX`
- **邀請已發送（UTC）：** `YYYY-MM-DD hh:mm`
- **確認的校驗和（UTC）：** `YYYY-MM-DD hh:mm`
- **主要關注領域：**（例如_SoraFS Orchestrator 文檔_、_Torii ISO 流程_）

## 遙測和人工製品確認

|清單項目 |結果 |證據|
| ---| ---| ---|
|校驗和驗證 | ✅ / ⚠️ |日誌路徑（例如，`build/checksums.sha256`）|
|嘗試一下代理冒煙測試 | ✅ / ⚠️ | `npm run manage:tryit-proxy …` 轉錄片段 |
| Grafana 儀表板回顧 | ✅ / ⚠️ |屏幕截圖路徑 |
|門戶探查報告審查| ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

為審閱者檢查的任何其他 SLO 添加行。

## 反饋日誌

|面積 |嚴重性（信息/次要/主要/阻止）|描述 |建議的修復或問題 |追踪器問題 |
| ---| ---| ---| ---| ---|
| | | | | |

參考最後一欄中的 GitHub 問題或內部票證，以便預覽
跟踪器可以將補救項目與此表單聯繫起來。

## 調查總結

1. **您對校驗和指導和邀請流程有多大信心？ ** (1-5)
2. **哪些文檔最有/最沒有幫助？ **（簡短回答）
3. **是否有任何攔截器訪問 Try it 代理或遙測儀表板？ **
4. **是否需要額外的本地化或輔助內容？ **
5. **正式發布前還有其他意見嗎？ **

如果您使用外部表單，請捕獲簡短的答案並附加原始調查導出。

## 知識檢查

- 分數：`__/10`
- 錯誤問題（如有）：`[#1, #4, …]`
- 後續行動（如果分數 < 9/10）：是否安排了補救電話？是/否

## 簽核

- 審稿人姓名和時間戳：
- 文檔/DevRel 審閱者和時間戳：

將簽名副本與相關工件一起存儲，以便審核員可以重播
沒有額外上下文的揮手。