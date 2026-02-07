---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2025-12-29T18:16:35.926037+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 歐盟法律簽署備忘錄模板

本備忘錄記錄了路線圖項目 **AND6** 之前要求的法律審查
歐盟 (ETSI/GDPR) 工件數據包已提交給監管機構。律師應該克隆
每個版本的此模板，填充下面的字段，並存儲簽名的副本
以及備忘錄中提到的不可改變的文物。

## 總結

- **發布/訓練：** `<e.g., 2026.1 GA>`
- **審核日期：** `<YYYY-MM-DD>`
- **顧問/審閱者：** `<name + organisation>`
- **範圍：** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **相關門票：** `<governance or legal issue IDs>`

## 工件清單

|文物 | SHA-256 |位置/鏈接|筆記|
|----------|---------|-----------------|--------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + 治理檔案 |確認發布標識符和威脅模型調整。 |
| `gdpr_dpia_summary.md` | `<hash>` |相同目錄/本地化鏡像 |確保編輯策略引用與 `sdk/android/telemetry_redaction.md` 匹配。 |
| `sbom_attestation.md` | `<hash>` |證據桶中的同一目錄+共同簽名捆綁包|驗證 CycloneDX + 來源簽名。 |
|證據日誌行| `<hash>` | `docs/source/compliance/android/evidence_log.csv` |行號 `<n>` |
|設備實驗室應急包| `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` |確認與此版本相關的故障轉移演練。 |

> 如果數據包包含更多文件（例如隱私文件），請附加附加行
> 附錄或 DPIA 翻譯）。每個人工製品都必須引用其不可變的
> 上傳目標和生成它的 Buildkite 作業。

## 調查結果和例外情況

- `None.` *（替換為涵蓋剩餘風險的項目符號列表，補償
  控制措施或所需的後續行動。）*

## 批准

- **決定：** `<Approved / Approved with conditions / Blocked>`
- **簽名/時間戳：** `<digital signature or email reference>`
- **後續所有者：** `<team + due date for any conditions>`

將最終備忘錄上傳到治理證據桶，將 SHA-256 複製到
`docs/source/compliance/android/evidence_log.csv`，並鏈接上傳路徑
`status.md`。如果決策為“阻止”，則升級至 AND6 轉向
委員會並記錄路線圖熱門列表和
設備實驗室應急日誌。