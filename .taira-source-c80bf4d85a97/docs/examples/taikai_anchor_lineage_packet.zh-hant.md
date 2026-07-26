---
lang: zh-hant
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage 數據包模板 (SN13-C)

路線圖項目 **SN13-C — 清單和 SoraNS 錨點** 需要每個別名
輪換以發送確定性證據包。將此模板複製到您的
推出工件目錄（例如
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) 並替換
將數據包提交給治理之前的佔位符。

## 1. 元數據

|領域|價值|
|--------|--------|
|事件 ID | `<taikai.event.launch-2026-07-10>` |
|流媒體/演繹| `<main-stage>` |
|別名命名空間/名稱 | `<sora / docs>` |
|證據目錄| `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
|運營商聯繫方式 | `<name + email>` |
| GAR / RPT 機票 | `<governance ticket or GAR digest>` |

## 捆綁助手（可選）

複製 spool 工件並在之前發出 JSON（可選簽名）摘要
填寫剩餘部分：

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

助手拉`taikai-anchor-request-*`，`taikai-trm-state-*`，
`taikai-lineage-*`、Taikai 假脫機目錄中的信封和哨兵
（`config.da_ingest.manifest_store_dir/taikai`）所以證據文件夾已經
包含下面引用的確切文件。

## 2. 血統賬本和提示

附上磁盤上的沿襲分類帳和為此編寫的提示 JSON Torii
窗口。這些直接來自
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` 和
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`。

|文物|文件| SHA-256 |筆記|
|----------|------|---------|--------|
|血統分類賬 | `taikai-trm-state-docs.json` | `<sha256>` |證明先前的清單摘要/窗口。 |
|血統提示| `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` |在上傳到 SoraNS 錨點之前捕獲。 |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. 錨點有效負載捕獲

記錄 Torii 傳遞給錨點服務的 POST 負載。有效載荷
包括 `envelope_base64`、`ssm_base64`、`trm_base64` 和內聯
`lineage_hint` 對象；審計依靠此捕獲來證明所暗示的內容
發送至 SoraNS。 Torii 現在自動將此 JSON 寫入為
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
在 Taikai spool 目錄（`config.da_ingest.manifest_store_dir/taikai/`）內，所以
操作員可以直接複製它，而不是抓取 HTTP 日誌。

|文物|文件| SHA-256 |筆記|
|----------|------|---------|--------|
|錨定帖子 | `requests/2026-07-10T18-00Z.json` | `<sha256>` |從 `taikai-anchor-request-*.json`（Taikai 線軸）複製的原始請求。 |

## 4. 清單摘要確認

|領域|價值|
|--------|--------|
|新清單摘要 | `<hex digest>` |
|先前的清單摘要（來自提示）| `<hex digest>` |
|窗口開始/結束 | `<start seq> / <end seq>` |
|接受時間戳| `<ISO8601>` |

參考上面記錄的賬本/提示哈希值，以便審核者可以驗證
被取代的窗口。

## 5. 指標 / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` 快照：`<Prometheus query + export path>`
- `/status taikai_alias_rotations` 轉儲（每個別名）：`<file path + hash>`

提供顯示計數器的 Prometheus/Grafana 導出或 `curl` 輸出
增量和此別名的 `/status` 數組。

## 6. 證據目錄清單

生成證據目錄的確定性清單（假脫機文件，
有效負載捕獲、指標快照），因此治理可以驗證每個哈希，而無需
解壓存檔。

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

|文物|文件| SHA-256 |筆記|
|----------|------|---------|--------|
|證據清單 | `manifest.json` | `<sha256>` |將此附加到治理數據包/GAR。 |

## 7. 清單

- [ ] 譜系分類帳複製 + 散列。
- [ ] 沿襲提示已復制+散列。
- [ ] 捕獲並散列錨點 POST 有效負載。
- [ ] 已填寫清單摘要表。
- [ ] 導出的指標快照（`taikai_trm_alias_rotations_total`、`/status`）。
- [ ] 使用 `scripts/repo_evidence_manifest.py` 生成的清單。
- [ ] 數據包上傳至治理，包含哈希值 + 聯繫信息。

為每個別名輪換維護此模板可以保持 SoraNS 治理
捆綁可重複性並將譜系提示直接與 GAR/RPT 證據聯繫起來。