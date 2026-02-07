---
id: chunker-profile-authoring
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Profile Authoring Guide
sidebar_label: Chunker Authoring Guide
description: Checklist for proposing new SoraFS chunker profiles and fixtures.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

# SoraFS Chunker 配置文件創作指南

本指南介紹瞭如何為 SoraFS 提議和發布新的分塊配置文件。
它補充了架構 RFC (SF-1) 和註冊表參考 (SF-2a)
具有具體的創作要求、驗證步驟和提案模板。
有關規範示例，請參閱
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
以及隨附的試運行登錄
`docs/source/sorafs/reports/sf1_determinism.md`。

## 概述

進入註冊表的每個配置文件都必須：

- 公佈相同的確定性 CDC 參數和多重哈希設置
  架構；
- 發布可重玩的裝置（Rust/Go/TS JSON + 模糊語料庫 + PoR 見證）
  下游 SDK 無需定制工具即可驗證；
- 包括治理就緒元數據（命名空間、名稱、semver）以及遷移
- 在理事會審查之前通過確定性差異套件。

請按照下面的清單準備滿足這些規則的提案。

## 註冊章程快照

在起草提案之前，請確認其符合強制執行的註冊管理機構章程
通過 `sorafs_manifest::chunker_registry::ensure_charter_compliance()`：

- 配置文件 ID 是單調遞增且無間隙的正整數。
- 規範句柄 (`namespace.name@semver`) 必須出現在別名列表中
  並且**必須**是第一個條目。
- 別名不得與其他規範句柄衝突或出現多次。
- 別名必須非空並刪除空格。

方便的 CLI 助手：

```bash
# JSON listing of all registered descriptors (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emit metadata for a candidate default profile (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

這些命令使提案與註冊管理機構章程保持一致，並提供
治理討論中所需的規范元數據。

## 所需的元數據

|領域|描述 |示例 (`sorafs.sf1@1.0.0`) |
|--------|-------------|------------------------------|
| `namespace` |相關配置文件的邏輯分組。 | `sorafs` |
| `name` |人類可讀的標籤。 | `sf1` |
| `semver` |參數集的語義版本字符串。 | `1.0.0` |
| `profile_id` |配置文件登陸後分配的單調數字標識符。保留下一個 ID，但不要重複使用現有號碼。 | `1` |
| `profile_aliases` |在談判期間向客戶公開的可選附加句柄。始終將規範句柄作為第一個條目。 | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` |最小塊長度（以字節為單位）。 | `65536` |
| `profile.target_size` |目標塊長度（以字節為單位）。 | `262144` |
| `profile.max_size` |最大塊長度（以字節為單位）。 | `524288` |
| `profile.break_mask` |滾動哈希（十六進制）使用的自適應掩碼。 | `0x0000ffff` |
| `profile.polynomial` |齒輪多項式常數（十六進制）。 | `0x3da3358b4dc173` |
| `gear_seed` |用於派生 64KiB 齒輪表的種子。 | `sorafs-v1-gear` |
| `chunk_multihash.code` |每個塊摘要的多重哈希代碼。 | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` |規範固定裝置包的摘要。 | `13fa...c482` |
| `fixtures_root` |包含重新生成的裝置的相對目錄。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` |確定性 PoR 採樣的種子 (`splitmix64`)。 | `0xfeedbeefcafebabe`（示例）|

元數據必須同時出現在提案文檔和生成的提案文檔中
固定裝置，以便註冊表、CLI 工具和治理自動化可以確認
無需手動交叉引用的值。如有疑問，請運行 chunk-store 並
帶有 `--json-out=-` 的清單 CLI，用於將計算的元數據流式傳輸以供審核
筆記。

### CLI 和註冊表接觸點

- `sorafs_manifest_chunk_store --profile=<handle>` – 重新運行塊元數據，
  清單摘要，PoR 使用建議的參數進行檢查。
- `sorafs_manifest_chunk_store --json-out=-` – 將塊存儲報告流式傳輸至
  用於自動比較的標準輸出。
- `sorafs_manifest_stub --chunker-profile=<handle>` – 確認艙單和 CAR
  計劃嵌入規範句柄和別名。
- `sorafs_manifest_stub --plan=-` – 反饋之前的 `chunk_fetch_specs`
  來驗證更改後的偏移量/摘要。

在提案中記錄命令輸出（摘要、PoR 根、清單哈希）
這樣審稿人就可以逐字重現它們。

## 確定性和驗證清單

1. **重新生成燈具**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **運行奇偶校驗套件** – `cargo test -p sorafs_chunker` 和
   跨語言差異工具 (`crates/sorafs_chunker/tests/vectors.rs`) 必須是
   綠色，新裝置就位。
3. **重放模糊/背壓語料庫** – 執行 `cargo fuzz list` 和
   針對再生資產的流式處理（`fuzz/sorafs_chunker`）。
4. **驗證可檢索性證明見證** – 運行
   `sorafs_manifest_chunk_store --por-sample=<n>` 使用建議的配置文件和
   確認根與夾具清單匹配。
5. **CI 試運行** – 本地調用 `ci/check_sorafs_fixtures.sh`；腳本
   新的燈具和現有的 `manifest_signatures.json` 應該會成功。
6. **跨運行時確認** – 確保 Go/TS 綁定消耗重新生成的
   JSON 並發出相同的塊邊界和摘要。

在提案中記錄命令和生成的摘要，以便工具工作組
可以重新運行它們而無需猜測。

### 清單/PoR 確認

重新生成固定裝置後，運行完整的清單管道以確保 CAR
元數據和 PoR 證明保持一致：

```bash
# Validate chunk metadata + PoR with the new profile
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generate manifest + CAR and capture chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-run using the saved fetch plan (guards against stale offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

將輸入文件替換為您的裝置使用的任何代表性語料庫
（例如，1GiB 確定性流）並將生成的摘要附加到
提案。

## 提案模板

提案作為已簽入的 `ChunkerProfileProposalV1` Norito 記錄提交
`docs/source/sorafs/proposals/`。下面的 JSON 模闆說明了預期的結果
形狀（根據需要替換您的值）：


提供匹配的 Markdown 報告 (`determinism_report`)，以捕獲
命令輸出、塊摘要以及驗證過程中遇到的任何偏差。

## 治理工作流程

1. **提交帶有提案+固定裝置的 PR。 ** 包括生成的資產、
   Norito提案，並更新為`chunker_registry_data.rs`。
2. **工具工作組審查。 ** 審查人員重新運行驗證清單並確認
   該提案符合註冊管理機構規則（無 ID 重用，滿足確定性）。
3. **理事會信封。 ** 一旦獲得批准，理事會成員簽署提案摘要
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) 並附加它們
   與燈具一起存儲的配置文件信封的簽名。
4. **註冊表發布。 ** 合併會影響註冊表、文檔和固定裝置。的
   默認 CLI 保留在以前的配置文件上，直到治理聲明
   遷移準備就緒。
5. **棄用跟踪。 ** 在遷移窗口之後，將註冊表更新為
   分類帳。

## 創作技巧

- 甚至更喜歡二次冪邊界以最小化邊緣情況分塊行為。
- 避免在未協調清單和網關的情況下更改多重哈希代碼
- 保持齒輪表種子可讀但全球唯一，以簡化審核
  踪跡。
- 將任何基準測試工件（例如吞吐量比較）存儲在
  `docs/source/sorafs/reports/` 供將來參考。

有關推出期間的運營預期，請參閱遷移分類賬
（`docs/source/sorafs/migration_ledger.md`）。有關運行時一致性規則，請參閱
`docs/source/sorafs/chunker_conformance.md`。