---
lang: zh-hant
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ 傳輸小工具設計

# 概述

當前的 FASTPQ 規劃器記錄 `TransferAsset` 指令中涉及的每個原始操作，這意味著每次傳輸分別支付餘額算術、哈希輪次和 SMT 更新。為了減少每次傳輸的跟踪行數，我們引入了一個專用小工具，該小工具僅在主機繼續執行規範狀態轉換時驗證最少的算術/提交檢查。

- **範圍**：通過現有 Kotodama/IVM `TransferAsset` 系統調用表面發出單次傳輸和小批量。
- **目標**：通過共享查找表並將每次傳輸算術壓縮為緊湊的約束塊，減少大容量傳輸的 FFT/LDE 列佔用空間。

# 架構

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## 成績單格式

主機每次系統調用都會發出 `TransferTranscript`：

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` 將轉錄本與事務入口點哈希相關聯以實現重播保護。
- `authority_digest` 是主機在排序簽名者/仲裁數據上的哈希值；該小工具會檢查相等性，但不會重做簽名驗證。具體來說，主機 Norito 對 `AccountId`（已嵌入規範的多重簽名控制器）進行編碼，並使用 Blake2b-256 對 `b"iroha:fastpq:v1:authority|" || encoded_account` 進行哈希處理，存儲結果 `Hash`。
- `poseidon_preimage_digest` = Poseidon(account_from || account_to || asset || amount || batch_hash);確保小工具重新計算與主機相同的摘要。原像字節使用裸 Norito 編碼構造為 `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash`，然後將其傳遞給共享 Poseidon2 助手。此摘要對於單增量轉錄本存在，而對於多增量批次則省略。

所有字段都通過 Norito 進行序列化，因此現有的確定性保證有效。
`from_path` 和 `to_path` 均使用以下命令作為 Norito blob 發出
`TransferMerkleProofV1` 架構：`{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`。
未來版本可以擴展模式，同時證明者強制執行版本標籤
解碼之前。 `TransitionBatch` 元數據嵌入 Norito 編碼的轉錄本
`transfer_transcripts` 密鑰下的向量，以便證明者可以解碼見證人
無需執行帶外查詢。公共輸入（`dsid`、`slot`、根、
`perm_root`、`tx_set_hash`) 被攜帶在 `FastpqTransitionBatch.public_inputs` 中，
留下元數據用於條目哈希/轉錄計數簿記。直到主機管道
落地後，證明者從密鑰/餘額對中綜合得出證明，因此行
即使轉錄本省略了可選字段，也始終包含確定性 SMT 路徑。

## 小工具佈局

1. **平衡算術塊**
   - 輸入：`from_balance_before`、`amount`、`to_balance_before`。
   - 檢查：
     - `from_balance_before >= amount`（具有共享 RNS 分解的範圍小工具）。
     - `from_balance_after = from_balance_before - amount`。
     - `to_balance_after = to_balance_before + amount`。
   - 打包到一個自定義門中，因此所有三個方程都消耗一個行組。2. **波塞冬承諾塊**
   - 使用其他小工具中已使用的共享 Poseidon 查找表重新計算 `poseidon_preimage_digest`。跟踪中沒有每次傳輸的波塞冬彈。

3. **默克爾路徑塊**
   - 通過“配對更新”模式擴展現有的 Kaigi SMT 小工具。兩個葉子（發送者、接收者）共享同級哈希的同一列，從而減少重複的行。

4. **權威摘要檢查**
   - 主機提供的摘要和見證值之間的簡單相等約束。簽名保留在他們的專用小工具中。

5. **批量循環**
   - 程序在 `transfer_asset` 構建器循環之前調用 `transfer_v1_batch_begin()`，之後調用 `transfer_v1_batch_end()`。當示波器處於活動狀態時，主機會緩衝每次傳輸並將其作為單個 `TransferAssetBatch` 重播，每批重用一次 Poseidon/SMT 上下文。每個附加增量僅添加算術和兩個葉檢查。轉錄解碼器現在接受多增量批次並將其顯示為 `TransferGadgetInput::deltas`，因此計劃者可以折疊見證人而無需重新讀取 Norito。已經方便使用 Norito 有效負載的合約（例如 CLI/SDK）可以通過調用 `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` 來完全跳過範圍，這會在一個系統調用中向主機提供完全編碼的批次。

# 主機和證明者變更|層|變化|
|--------|---------|
| `ivm::syscalls` |添加 `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`)，以便程序可以將多個 `transfer_v1` 系統調用括起來，而不會發出中間 ISI，再加上 `transfer_v1_batch_apply` (`0x2B`)預編碼批次。 |
| `ivm::host` 和測試 |當範圍處於活動狀態時，核心/默認主機將 `transfer_v1` 視為批量追加，表面 `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` 和模擬 WSV 主機在提交之前緩衝條目，因此回歸測試可以斷言確定性平衡更新。 【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【板條箱/ivm/tests/wsv_host_pointer_tlv.rs:219】【板條箱/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` |狀態轉換後發出 `TransferTranscript`，在 `StateBlock::capture_exec_witness` 期間使用顯式 `public_inputs` 構建 `FastpqTransitionBatch` 記錄，並運行 FASTPQ 驗證器通道，以便 Torii/CLI 工具和 Stage6 後端接收規範`TransitionBatch` 輸入。 `TransferAssetBatch` 將順序傳輸分組為單個轉錄本，省略多增量批次的海神摘要，以便小工具可以確定性地跨條目迭代。 |
| `fastpq_prover` | `gadgets::transfer` 現在為規劃器 (`crates/fastpq_prover/src/gadgets/transfer.rs`) 驗證多增量轉錄本（平衡算術 + Poseidon 摘要）並顯示結構化見證（包括佔位符配對的 SMT blob）。 `trace::build_trace` 從批次元數據中解碼這些轉錄本，拒絕缺少 `transfer_transcripts` 有效負載的傳輸批次，將經過驗證的見證附加到 `Trace::transfer_witnesses`，並且 `TracePolynomialData::transfer_plan()` 使聚合計劃保持活動狀態，直到規劃器使用小工具 (`crates/fastpq_prover/src/trace.rs`)。行計數回歸線束現在通過 `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) 提供，覆蓋多達 65536 行填充的場景，而成對的 SMT 佈線仍然落後於 TF-3 批處理輔助里程碑（佔位符使走線佈局保持穩定，直到交換落地）。 |
| Kotodama |將 `transfer_batch((from,to,asset,amount), …)` 幫助程序降低為 `transfer_v1_batch_begin`、順序 `transfer_asset` 調用和 `transfer_v1_batch_end`。每個元組參數必須遵循 `(AccountId, AccountId, AssetDefinitionId, int)` 形狀；單一轉讓保留現有的建設者。 |

Kotodama 用法示例：

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` 執行與單個 `Transfer::asset_numeric` 調用相同的權限和算術檢查，但將所有增量記錄在單個 `TransferTranscript` 內。多增量轉錄本會忽略海神摘要，直到每個增量承諾進入後續階段。 Kotodama 構建器現在自動發出開始/結束系統調用，因此合約可以部署批量傳輸，而無需手動編碼 Norito 有效負載。

## 行數回歸工具

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) 使用可配置的選擇器計數合成 FASTPQ 轉換批次，並報告生成的 `row_usage` 摘要（`total_rows`、每個選擇器計數、比率）以及填充的長度/log2。通過以下方式捕獲 65536 行天花板的基準：

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```發出的 JSON 鏡像了 `iroha_cli audit witness` 現在默認發出的 FASTPQ 批處理工件（通過 `--no-fastpq-batches` 來抑制它們），因此 `scripts/fastpq/check_row_usage.py` 和 CI 門可以在驗證計劃器更改時將合成運行與之前的快照進行比較。

# 推出計劃

1. **TF-1（轉錄管道）**： ✅ `StateTransaction::record_transfer_transcripts` 現在為每個 `TransferAsset`/批次發出 Norito 轉錄本，`sumeragi::witness::record_fastpq_transcript` 將它們存儲在全局見證中，`StateBlock::capture_exec_witness` 構建 `fastpq_batches`為操作員和驗證通道提供顯式 `public_inputs`（如果您需要更精簡的，請使用 `--no-fastpq-batches`輸出).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2（小工具實現）**： ✅ `gadgets::transfer` 現在驗證多增量轉錄本（平衡算術 + Poseidon 摘要），在主機省略時合成配對的 SMT 證明，通過 `TransferGadgetPlan` 公開結構化見證，並且 `trace::build_trace` 將這些見證線程到 `Trace::transfer_witnesses`，同時從校樣填充 SMT 列。 `fastpq_row_bench` 捕獲 65536 行回歸工具，因此規劃人員無需重放即可跟踪行使用情況 Norito有效負載。 【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3（批處理助手）**：啟用批處理系統調用 + Kotodama 構建器，包括主機級順序應用程序和小工具循環。
4. **TF-4（遙測和文檔）**：更新 `fastpq_plan.md`、`fastpq_migration_guide.md` 和儀表板架構，以顯示傳輸行與其他小工具的分配。

# 開放式問題

- **域限制**：當前的 FFT 規劃器對於超過 21⁴ 行的跟踪會出現恐慌。 TF-2 應該提高域大小或記錄減少的基準目標。
- **多資產批次**：初始小工具假定每個增量具有相同的資產 ID。如果我們需要異構批次，我們必須確保 Poseidon 見證人每次都包含該資產，以防止跨資產重放。
- **權限摘要重用**：從長遠來看，我們可以將相同的摘要重用於其他授權操作，以避免每個系統調用重新計算簽名者列表。


該文檔跟踪設計決策；當里程碑落地時，使其與路線圖條目保持同步。