---
lang: zh-hant
direction: ltr
source: docs/source/governance_vote_tally.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ebff8477d06e2aac8840988d31762704d05ded353d3f900a87db3ea5091e718
source_last_modified: "2026-01-04T08:19:26.508527+00:00"
translation_last_reviewed: 2026-02-07
title: Governance ZK Vote Tally
translator: machine-google-reviewed
---

## 概述

Iroha 的治理計數流程依賴於 Halo2/IPA 電路，該電路驗證比特投票承諾及其在合格選民集中的成員資格。本註釋捕獲電路參數、公共輸入和審核裝置，以便審核人員可以重新生成測試中使用的驗證密鑰和證明。

## 電路總結

- **電路標識符**：`halo2/pasta/vote-bool-commit-merkle8-v1`
- **實現**：`VoteBoolCommitMerkle::<8>` 中的 `iroha_core::zk::depth`
- **域名大小**：`k = 6`
- **後端**：透明Halo2/IPA over Pasta（ZK1信封：`IPAK` + `H2VK`用於VK，`PROF` + `I10P`用於校樣）
- **見證形狀**：
  - 選票位 `v ∈ {0,1}`
  - 隨機標量 `ρ`
  - Merkle 路徑的八個同級標量
  - 方向位（參考見證中全為零）
- **Merkle 壓縮器**：`H(x, y) = 2·(x + 7)^5 + 3·(y + 13)^5 (mod p)`，其中 `p` 是 Pasta 標量模數
- **公共投入**：
  - 第 0 列：`commit`
  - 第 1 列：Merkle 根
  - 通過 `I10P` TLV 暴露（`cols = 2`、`rows = 1`）

### 電路佈局

- **建議欄**：
  - `v` – 選票位限制為布爾值。
  - `ρ` – 投票承諾中使用的致盲標量。
  - `sibling[i]` for `i ∈ [0, 7]` – 深度 `i` 處的 Merkle 路徑元素。
  - `dir[i]` 用於 `i ∈ [0, 7]` – 方向位選擇左 (`0`) 或右 (`1`) 分支。
  - `node[i]` for `i ∈ [0, 7]` – 深度 `i` 後的 Merkle 累加器。
- **實例列**：
  - `commit` – 選民發布的公開承諾。
  - `root` – 合格選民集的 Merkle 根。
- **選擇器**：`s_vote` 啟用單個填充行上的門。

所有建議單元格都分配在該區域的第一行（也是唯一一行）；該電路使用 `SimpleFloorPlanner`。

### 門系統

令 `H` 為上面定義的壓縮器，`prev_0 = H(v, ρ)` 為。門強制執行：

1. `s_vote · v · (v - 1) = 0` – 布爾選票位。
2. `s_vote · (H(v, ρ) - commit) = 0` – 承諾一致性。
3. 對於每個深度 `i`：
   - `s_vote · dir[i] · (dir[i] - 1) = 0` – 布爾路徑方向。
   - `left = H(prev_i, sibling[i])`
   - `right = H(sibling[i], prev_i)`
   - `expected = (1 - dir[i]) · left + dir[i] · right`
   - `s_vote · (node[i] - expected) = 0`
   - `prev_{i+1} = node[i]`
4. `s_vote · (prev_8 - root) = 0` – 累加器等於公共 Merkle 根。

壓縮器僅使用五次形狀；不需要查找表。所有算術都在 Pasta 標量字段中執行，行計數 `k = 6` 分配 `2^k = 64` 行 — 僅填充第 0 行。

### 規範夾具

確定性工具 (`zk_testkit::vote_merkle8_bundle`) 向見證人填充：

- `v = 1`
- `ρ = 12345`
- `sibling[i] = 10 + i` 為 `i ∈ [0, 7]`
- `dir[i] = 0`
- `node[i] = H(node[i-1], sibling[i])` 與 `node[-1] = H(v, ρ)`

這產生了公共價值觀：

```text
commit = 0x20574662a58708e02e0000000000000000000000000000000000000000000000
root   = 0xb63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817
```

驗證密鑰註冊表中記錄的 `public_inputs_schema_hash` 是 `blake2b-256(commit_bytes || root_bytes)`，最低有效位強制為 `1`，產生：

```text
public_inputs_schema_hash = 0xfae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3
```

### 驗證密鑰記錄

治理將驗證者註冊為：- `backend = "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1"`
- `circuit_id = "halo2/pasta/vote-bool-commit-merkle8-v1"`
- `backend tag = BackendTag::Halo2IpaPasta`
- `curve = "pallas"`
- `public_inputs_schema_hash = 0xfae4…64d3`
- `commitment = sha256(backend || vk_bytes)`（32字節摘要）

規範捆綁包包括一個內聯驗證密鑰 (`key = Some(VerifyingKeyBox { … })`) 以及證明信封。 `vk_len`、`max_proof_bytes` 和可選元數據 URI 是從生成的工件中填充的。

## 參考賽程

使用 `cargo xtask zk-vote-tally-bundle --print-hashes` 重新生成集成測試消耗的內聯驗證密鑰和證明包（默認情況下輸出位於 `fixtures/zk/vote_tally/` 中）。該命令打印一個簡短的摘要（`backend`、`commit`、`root`、模式哈希、長度）以及可選的文件哈希，以便審核員可以捕獲證明註釋。傳遞 `--summary-json -` 以發出與 JSON 相同的數據（或提供將其寫入磁盤的路徑）。傳遞 `--attestation attestation.json`（或標準輸出的 `-`）來編寫 Norito JSON 清單，其中包含摘要以及每個捆綁包工件的 Blake2b-256 摘要和大小，以便可以將證明數據包與固定裝置一起存檔。使用 `--verify` 運行時，提供 `--attestation <path>` 檢查清單的捆綁包元數據和工件長度是否與新生成的捆綁包相匹配（它不會比較每次運行的證明摘要，該摘要會隨轉錄隨機性而變化）。

重新生成規範的裝置和清單：

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --print-hashes \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

驗證簽入的工件是否保持最新（要求固定裝置目錄包含基線包）：

```bash
cargo xtask zk-vote-tally-bundle \
  --out fixtures/zk/vote_tally \
  --verify \
  --attestation fixtures/zk/vote_tally/bundle.attestation.json
```

清單示例：

```jsonc
{
  "generated_unix_ms": 3513801751697071715,
  "hash_algorithm": "blake2b-256",
  "bundle": {
    "backend": "halo2/pasta/ipa-v1/vote-bool-commit-merkle8-v1",
    "circuit_id": "halo2/pasta/vote-bool-commit-merkle8-v1",
    "commit_hex": "20574662a58708e02e0000000000000000000000000000000000000000000000",
    "root_hex": "b63752ff429362c3a9b3cd5966c23567fdb757ce3b38af724b9303a5ea2f5817",
    "public_inputs_schema_hash_hex": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
    "vk_commitment_hex": "6f4749f5f75fee2a40880d4798123033b2b8036284225bad106b04daca5fb10e",
    "vk_len": 66,
    "proof_len": 2748
  },
  "artifacts": [
    {
      "file": "vote_tally_meta.json",
      "len": 522,
      "blake2b_256": "5d0030856f189033e5106415d885fbb2e10c96a49c6115becbbff8b7fd992b77"
    },
    {
      "file": "vote_tally_proof.zk1",
      "len": 2748,
      "blake2b_256": "01449c0599f9bdef81d45f3be21a514984357a0aa2d7fcf3a6d48be6307010bb"
    },
    {
      "file": "vote_tally_vk.zk1",
      "len": 66,
      "blake2b_256": "2fd5859365f1d9576c5d6836694def7f63149e885c58e72f5c4dff34e5005d6b"
    }
  ]
}
```

將當前清單存儲在規範工件旁邊（例如 `fixtures/zk/vote_tally/bundle.attestation.json`）。上游存儲庫將此目錄保持為空，以避免提交大型二進制包，因此在依賴 `--verify` 之前在本地播種。

`generated_unix_ms` 是從承諾/驗證密鑰指紋確定性派生的，因此它在再生過程中保持穩定。生成器使用固定的 ChaCha20 轉錄本，因此元數據、驗證密鑰和證明信封哈希是可重現的。現在，任何摘要不匹配都表明必須調查漂移。審核員應記錄發出的值以及他們證明的偽影。

工作流程提醒：

1. 運行 `cargo xtask zk-vote-tally-bundle --out fixtures/zk/vote_tally --print-hashes --attestation fixtures/zk/vote_tally/bundle.attestation.json` 在本地為捆綁包做種。
2. 根據需要提交或歸檔生成的工件。
3. 在後續重新生成時使用 `--verify` 以確保證明與規範包匹配。

該任務在內部運行 `xtask/src/vote_tally.rs` 中的確定性生成器，其中：

1. 對證人進行採樣（`v = 1`、`ρ = 12345`、兄弟姐妹 `10..17`）
2. 運行 `keygen_vk`/`keygen_pk`
3. 生成 Halo2 證明並將其包裝在 ZK1 信封中（包括公共實例）
4. 使用適當的 `public_inputs_schema_hash` 發出驗證密鑰記錄

## 篡改覆蓋率

`crates/iroha_core/tests/zk_vote_tally_audit.rs` 加載捆綁包並檢查：

- 真實證據與捆綁的內聯 VK 進行驗證。
- 翻轉承諾列中的任何字節都會導致驗證失敗。
- 翻轉根列中的任何字節都會導致驗證失敗。這些回歸測試保證 Torii（和主機）拒絕其公共輸入在證明生成後被篡改的信封。

使用以下命令在本地運行回歸：

```bash
cargo test -p iroha_core zk_vote_tally_audit -- --nocapture
```

## 審核清單

1. 檢查 `VoteBoolCommitMerkle::<8>` 的約束完整性和常數選擇。
2. 重新運行 `cargo xtask zk-vote-tally-bundle --verify --print-hashes` 以重現 VK/proof 並確認記錄的哈希值。
3. 確認 Torii 的計數處理程序使用相同的後端標識符和信封佈局。
4. 執行篡改回歸以確保變異證明無法通過驗證。
5. 對 `bundle.attestation.json` 輸出 (Blake2b-256) 進行散列和八卦，以便審閱者可以在其證明旁邊記錄規范清單。