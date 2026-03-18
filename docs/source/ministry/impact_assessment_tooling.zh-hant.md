---
lang: zh-hant
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 影響評估工具 (MINFO-4b)

路線圖參考：**MINFO-4b — 影響評估工具。 **  
所有者：治理委員會/Analytics

本註釋記錄了現在的 `cargo xtask ministry-agenda impact` 命令
生成公投數據包所需的自動哈希族差異。的
工具使用經過驗證的議程理事會提案、重複註冊表以及
可選的拒絕名單/策略快照，以便審核者可以準確地看到哪些
指紋是新的，與現有策略相衝突，以及有多少條目
每個哈希家族都有貢獻。

## 輸入

1. **議程提案。 ** 隨後的一個或多個文件
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md)。
   使用 `--proposal <path>` 顯式傳遞它們或將命令指向
   通過 `--proposal-dir <dir>` 的目錄以及該路徑下的每個 `*.json` 文件
   包括在內。
2. **重複註冊表（可選）。 ** 匹配的 JSON 文件
   `docs/examples/ministry/agenda_duplicate_registry.json`。衝突是
   報告為 `source = "duplicate_registry"`。
3. **策略快照（可選）。 ** 列出每個策略的輕量級清單
   GAR/部委政策已強制實施指紋。裝載機期望
   架構如下所示（參見
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   完整的樣本）：

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

任何 `hash_family:hash_hex` 指紋與提議目標匹配的條目都是
在 `source = "policy_snapshot"` 下報告，並引用 `policy_id`。

## 用法

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

可以通過重複的 `--proposal` 標誌或通過
提供包含整個公投批次的目錄：

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

當省略 `--out` 時，該命令將生成的 JSON 打印到標準輸出。

## 輸出

該報告是一份已簽署的人工製品（將其記錄在公投數據包的下方）
`artifacts/ministry/impact/`目錄），結構如下：

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

將此 JSON 與中立摘要一起附加到每個公投檔案中，以便
小組成員、陪審員和治理觀察員可以看到確切的爆炸半徑
每個提案。輸出是確定性的（按哈希族排序）並且可以安全地
包含在 CI/運行手冊中；如果重複的註冊表或策略快照發生更改，
在投票開始之前重新運行命令並附加刷新的工件。

> **下一步：** 將生成的影響報告輸入
> [`cargo xtask ministry-panel packet`](referendum_packet.md) 所以
> `ReferendumPacketV1` 檔案包含哈希家族細分和
> 正在審查的提案的詳細衝突列表。