---
lang: zh-hant
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2025-12-29T18:16:35.977493+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 議程理事會提案架構 (MINFO-2a)

路線圖參考：**MINFO-2a — 提案格式驗證器。 **

議程委員會工作流程批量處理公民提交的黑名單和政策變更
治理小組審查提案之前。該文件定義了
規範的有效負載模式、證據要求和重複檢測規則
由新驗證器 (`cargo xtask ministry-agenda validate`) 消耗，因此
提案者可以在將 JSON 提交上傳到門戶之前在本地檢查它們。

## 負載概述

議程提案使用 `AgendaProposalV1` Norito 架構
（`iroha_data_model::ministry::AgendaProposalV1`）。當以下情況時，字段被編碼為 JSON：
通過 CLI/門戶界面提交。

|領域 |類型 |要求|
|--------|------|--------------|
| `version` | `1` (u16) |必須等於 `AGENDA_PROPOSAL_VERSION_V1`。 |
| `proposal_id` |字符串 (`AC-YYYY-###`) |穩定的標識符；在驗證期間強制執行。 |
| `submitted_at_unix_ms` | u64 | 64自 Unix 紀元以來的毫秒數。 |
| `language` |字符串| BCP-47 標籤（`"en"`、`"ja-JP"` 等）。 |
| `action` |枚舉（`add-to-denylist`、`remove-from-denylist`、`amend-policy`）|要求部採取行動。 |
| `summary.title` |字符串|建議≤256 個字符。 |
| `summary.motivation` |字符串|為什麼需要採取該行動。 |
| `summary.expected_impact` |字符串|行動被接受後的結果。 |
| `tags[]` |小寫字符串 |可選分類標籤。允許的值：`csam`、`malware`、`fraud`、`harassment`、`impersonation`、`policy-escalation`、`terrorism`、`spam`。 |
| `targets[]` |對象|一個或多個哈希族條目（見下文）。 |
| `evidence[]` |對象|一份或多份證據附件（見下文）。 |
| `submitter.name` |字符串|顯示名稱或組織。 |
| `submitter.contact` |字符串|電子郵件、Matrix 手柄或電話；從公共儀表板編輯。 |
| `submitter.organization` |字符串（可選）|在審閱者 UI 中可見。 |
| `submitter.pgp_fingerprint` |字符串（可選）| 40 進制大寫指紋。 |
| `duplicates[]` |字符串|對先前提交的提案 ID 的可選引用。 |

### 目標條目 (`targets[]`)

每個目標代表提案引用的哈希族摘要。

|領域 |描述 |驗證 |
|--------|-------------|------------|
| `label` |審閱者上下文的友好名稱。 |非空。 |
| `hash_family` |哈希標識符（`blake3-256`、`sha256` 等）。 | ASCII 字母/數字/`-_.`，≤48 個字符。 |
| `hash_hex` |以小寫十六進制編碼的摘要。 | ≥16 個字節（32 個十六進製字符）並且必須是有效的十六進制。 |
| `reason` |簡短描述為什麼應該對摘要採取行動。 |非空。 |

驗證器拒絕同一組中重複的 `hash_family:hash_hex` 對
當相同的指紋已存在於提案和報告中時，提案和報告會發生衝突
重複註冊表（見下文）。

### 證據附件 (`evidence[]`)

證據條目記錄了審閱者可以獲取支持上下文的位置。|領域 |類型 |筆記|
|--------|------|--------|
| `kind` |枚舉（`url`、`torii-case`、`sorafs-cid`、`attachment`）|確定消化要求。 |
| `uri` |字符串| HTTP(S) URL、Torii 案例 ID 或 SoraFS URI。 |
| `digest_blake3_hex` |字符串| `sorafs-cid` 和 `attachment` 類型需要；對於其他人來說是可選的。 |
| `description` |字符串|供審閱者選擇的自由格式文本。 |

### 重複註冊表

操作員可以維護現有指紋的註冊表以防止重複
案例。驗證器接受格式如下的 JSON 文件：

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

當提案目標與條目匹配時，驗證器將中止，除非
指定了 `--allow-registry-conflicts`（仍然發出警告）。
使用 [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) 來
生成交叉引用副本的公投就緒摘要
註冊表和策略快照。

## CLI 用法

Lint 單個提案並根據重複的註冊表檢查它：

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

通過 `--allow-registry-conflicts` 將重複命中降級為警告
執行歷史審計。

CLI 依賴於相同的 Norito 模式和驗證助手
`iroha_data_model`，因此 SDK/門戶可以重用 `AgendaProposalV1::validate`
一致行為的方法。

## 排序 CLI (MINFO-2b)

路線圖參考：**MINFO-2b — 多時隙抽籤和審核日誌。 **

議程委員會名冊現在通過確定性抽籤進行管理，因此公民
可以獨立審核每次抽獎。使用新命令：

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — 描述每個合格成員的 JSON 文件：

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  示例文件位於
  `docs/examples/ministry/agenda_council_roster.json`。可選字段（角色、
  組織、聯繫人、元數據）都在 Merkle 葉中捕獲，因此審計員
  可以證明抽籤的名單。

- `--slots` — 待填補的理事會席位數量。
- `--seed` — 32 字節 BLAKE3 種子（64 個小寫十六進製字符）記錄在
  抽籤的治理記錄。
- `--out` — 可選輸出路徑。省略時，JSON 摘要將打印到
  標準輸出。

### 輸出摘要

該命令發出 `SortitionSummary` JSON blob。樣本輸出存儲在
`docs/examples/ministry/agenda_sortition_summary_example.json`。關鍵領域：

|領域 |描述 |
|--------|-------------|
| `algorithm` |分類標籤 (`agenda-sortition-blake3-v1`)。 |
| `roster_digest` |名冊文件的 BLAKE3 + SHA-256 摘要（用於確認對同一成員列表進行審核）。 |
| `seed_hex` / `slots` |回顯 CLI 輸入，以便審核員可以重現抽籤結果。 |
| `merkle_root_hex` |名冊 Merkle 樹的根（`hash_node`/`hash_leaf` 助手位於 `xtask/src/ministry_agenda.rs` 中）。 |
| `selected[]` |每個槽位的條目，包括規范成員元數據、合格索引、原始花名冊索引、確定性繪製熵、葉哈希和 Merkle 證明同級。 |

### 驗證平局1. 獲取 `roster_path` 引用的名冊並驗證其 BLAKE3/SHA-256
   摘要與摘要相匹配。
2. 使用相同的種子/時段/名冊重新運行 CLI；生成的 `selected[].member_id`
   順序應與發布的摘要相符。
3. 對於特定成員，使用序列化成員 JSON 計算 Merkle 葉子
   (`norito::json::to_vec(&sortition_member)`) 並折疊每個證明哈希。決賽
   摘要必須等於 `merkle_root_hex`。示例摘要中的幫助程序顯示
   如何組合 `eligible_index`、`leaf_hash_hex` 和 `merkle_proof[]`。

這些人工製品滿足 MINFO-2b 對可驗證隨機性的要求，
k-of-m 選擇和僅附加審核日誌，直到連接鏈上 API。

## 驗證錯誤參考

`AgendaProposalV1::validate` 發出 `AgendaProposalValidationError` 變體
每當有效負載無法進行 linting 時。下表總結了最常見的
錯誤，以便門戶審核者可以將 CLI 輸出轉換為可操作的指導。|錯誤 |意義|修復|
|--------|---------|-------------|
| `UnsupportedVersion { expected, found }` |有效負載 `version` 與驗證器支持的架構不同。 |使用最新的架構包重新生成 JSON，以便版本與 `expected` 匹配。 |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` 為空或不是 `AC-YYYY-###` 形式。 |重新提交之前，請按照記錄的格式填充唯一標識符。 |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` 為零或缺失。 |記錄提交時間戳（以 Unix 毫秒為單位）。 |
| `InvalidLanguageTag { value }` | `language` 不是有效的 BCP-47 標籤。 |使用標準標籤，例如 `en`、`ja-JP` 或 BCP-47 識別的其他區域設置。 |
| `MissingSummaryField { field }` | `summary.title`、`.motivation` 或 `.expected_impact` 之一為空。 |為指定的摘要字段提供非空文本。 |
| `MissingSubmitterField { field }` | `submitter.name` 或 `submitter.contact` 缺失。 |提供缺少的提交者元數據，以便審核者可以聯繫提案者。 |
| `InvalidTag { value }` | `tags[]` 條目不在允許列表中。 |刪除標記或將其重命名為已記錄的值之一（`csam`、`malware` 等）。 |
| `MissingTargets` | `targets[]` 數組為空。 |提供至少一個目標哈希族條目。 |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` |目標條目缺少 `label` 或 `reason` 字段。 |在重新提交之前填寫索引條目的必填字段。 |
| `InvalidHashFamily { index, value }` |不支持的 `hash_family` 標籤。 |將哈希系列名稱限制為 ASCII 字母數字加 `-_`。 |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` |摘要不是有效的十六進製或短於 16 字節。 |為索引目標提供小寫十六進制摘要（≥32 個十六進製字符）。 |
| `DuplicateTarget { index, fingerprint }` |目標摘要復制較早的條目或註冊表指紋。 |刪除重複項或將支持證據合併為單個目標。 |
| `MissingEvidence` |沒有提供證據附件。 |附上至少一份鏈接到復製材料的證據記錄。 |
| `MissingEvidenceUri { index }` |證據條目缺少 `uri` 字段。 |提供索引證據條目的可獲取 URI 或案例標識符。 |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` |需要摘要的證據條目（SoraFS CID 或附件）丟失或具有無效的 `digest_blake3_hex`。 |為索引條目提供 64 個字符的小寫 BLAKE3 摘要。 |

## 示例

- `docs/examples/ministry/agenda_proposal_example.json` — 規範，
  帶有兩個證據附件的干淨提案有效負載。
- `docs/examples/ministry/agenda_duplicate_registry.json` — 啟動註冊表
  包含單個 BLAKE3 指紋和基本原理。

集成門戶工具或編寫 CI 時，將這些文件重複用作模板
檢查自動提交。