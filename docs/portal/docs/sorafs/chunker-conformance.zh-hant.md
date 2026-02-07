---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d948fcd78a564487591aeba23d4587de337913984fd3a5861a83f2a9a23887d9
source_last_modified: "2026-01-05T09:28:11.855022+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-conformance
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
---

:::注意規範來源
:::

本指南規定了每個實施必須遵循的要求
與 SoraFS 確定性分塊配置文件 (SF1) 一致。它還
記錄再生工作流程、簽名策略和驗證步驟，以便
跨 SDK 的燈具消費者保持同步。

## 規範配置文件

- 型材手柄：`sorafs.sf1@1.0.0`
- 輸入種子（十六進制）：`0000000000dec0ded`
- 目標大小：262144 字節 (256KiB)
- 最小大小：65536 字節 (64KiB)
- 最大大小：524288 字節 (512KiB)
- 滾動多項式：`0x3DA3358B4DC173`
- 齒輪表種子：`sorafs-v1-gear`
- 中斷掩碼：`0x0000FFFF`

參考實現：`sorafs_chunker::chunk_bytes_with_digests_profile`。
任何 SIMD 加速都必須產生相同的邊界和摘要。

## 夾具捆綁包

`cargo run --locked -p sorafs_chunker --bin export_vectors` 重新生成
固定裝置並在 `fixtures/sorafs_chunker/` 下發出以下文件：

- `sf1_profile_v1.{json,rs,ts,go}` — Rust 的規範塊邊界，
  TypeScript 和 Go 消費者。每個文件都將規範句柄通告為
  `profile_aliases` 中的第一個（也是唯一一個）條目。該順序由以下機構強制執行
  `ensure_charter_compliance` 並且不得更改。
- `manifest_blake3.json` — BLAKE3 驗證的清單涵蓋每個夾具文件。
- `manifest_signatures.json` — 艙單上的理事會簽名 (Ed25519)
  消化。
- `sf1_profile_v1_backpressure.json` 和 `fuzz/` 內的原始語料庫 —
  chunker 背壓測試使用的確定性流場景。

### 簽名政策

燈具再生**必須**包含有效的理事會簽名。發電機
拒絕無符號輸出，除非顯式傳遞 `--allow-unsigned`（預期
僅用於本地實驗）。簽名信封僅供附加，
對每個簽名者進行重複數據刪除。

添加理事會簽名：

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## 驗證

CI 助手 `ci/check_sorafs_fixtures.sh` 重放生成器
`--locked`。如果夾具漂移或簽名丟失，作業就會失敗。使用
在每晚工作流程中以及提交夾具更改之前執行此腳本。

手動驗證步驟：

1. 運行 `cargo test -p sorafs_chunker`。
2.本地調用`ci/check_sorafs_fixtures.sh`。
3. 確認 `git status -- fixtures/sorafs_chunker` 乾淨。

## 升級劇本

當提出新的 chunker 配置文件或更新 SF1 時：

另請參閱：[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
元數據要求、提案模板和驗證清單。

1. 起草具有新參數的 `ChunkProfileUpgradeProposalV1`（請參閱 RFC SF-1）。
2. 通過 `export_vectors` 重新生成裝置並記錄新的清單摘要。
3. 以所需的理事會法定人數簽署清單。所有簽名必須
   附加到 `manifest_signatures.json`。
4. 更新受影響的 SDK 固定裝置 (Rust/Go/TS) 並確保跨運行時奇偶校驗。
5. 如果參數發生變化，重新生成模糊語料庫。
6. 使用新的配置文件句柄、種子和摘要更新本指南。
7. 提交更改以及更新的測試和路線圖更新。

不遵循此過程而影響塊邊界或摘要的更改
無效，不得合併。