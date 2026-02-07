---
lang: zh-hant
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 字節碼頭


魔法
- 4 個字節：偏移量 0 處的 ASCII `IVM\0`。

佈局（當前）
- 偏移量和大小（總共 17 個字節）：
  - 0..4：魔法 `IVM\0`
  - 4：`version_major: u8`
  - 5：`version_minor: u8`
  - 6：`mode: u8`（功能位；見下文）
  - 7：`vector_length: u8`
  - 8..16：`max_cycles: u64`（小端）
  - 16：`abi_version: u8`

模式位
- `ZK = 0x01`、`VECTOR = 0x02`、`HTM = 0x04`（保留/功能門控）。

字段（含義）
- `abi_version`：系統調用表和指針 ABI 架構版本。
- `mode`：ZK 跟踪/VECTOR/HTM 的功能位。
- `vector_length`：向量操作的邏輯向量長度（0 → 未設置）。
- `max_cycles`：ZK 模式和准入中使用的執行填充邊界。

註釋
- 字節序和佈局由實現定義並綁定到 `version`。上面的在線佈局反映了 `crates/ivm_abi/src/metadata.rs` 中的當前實現。
- 最小讀者可以依賴此佈局來獲取當前工件，並應通過 `version` 門控處理未來的更改。
- 每個主機可選擇硬件加速（SIMD/Metal/CUDA）。運行時從 `iroha_config` 讀取 `AccelerationConfig` 值：`enable_simd` 在為 false 時強制標量回退，而 `enable_metal` 和 `enable_cuda` 即使在編譯時也會對其各自的後端進行門控。這些切換在創建 VM 之前通過 `ivm::set_acceleration_config` 應用。
- 移動 SDK (Android/Swift) 具有相同的旋鈕； `IrohaSwift.AccelerationSettings`
  調用 `connect_norito_set_acceleration_config`，因此 macOS/iOS 版本可以選擇使用 Metal /
  NEON 同時保持確定性回退。
- 操作員還可以通過導出 `IVM_DISABLE_METAL=1` 或 `IVM_DISABLE_CUDA=1` 來強制禁用特定後端進行診斷。這些環境覆蓋優先於配置，並使虛擬機保持在確定性 CPU 路徑上。

持久狀態助手和 ABI 表面
- 持久狀態幫助程序系統調用（0x50–0x5A：STATE_{GET,SET,DEL}、ENCODE/DECODE_INT、BUILD_PATH_* 和 JSON/SCHEMA 編碼/解碼）是 V1 ABI 的一部分，並包含在 `abi_hash` 計算中。
- CoreHost 將 STATE_{GET,SET,DEL} 連接到 WSV 支持的持久智能合約狀態；開發/測試主機可以使用覆蓋或本地持久性，但必須保留相同的可觀察行為。

驗證
- 節點准入僅接受 `version_major = 1` 和 `version_minor = 0` 標頭。
- `mode` 必須僅包含已知位：`ZK`、`VECTOR`、`HTM`（拒絕未知位）。
- `vector_length` 是建議性的，即使未設置 `VECTOR` 位，也可能為非零；准入僅強制執行上限。
- 支持的 `abi_version` 值：第一個版本僅接受 `1` (V1)；其他值在入學時被拒絕。

### 政策（生成）
以下政策摘要是在實施過程中生成的，不應手動編輯。<!-- BEGIN GENERATED HEADER POLICY -->
|領域|政策 |
|---|---|
|主要版本 | 1 |
|次要版本 | 0 |
|模式（已知位）| 0x07（ZK=0x01，矢量=0x02，HTM=0x04）|
| abi_版本 | 1 |
|向量長度| 0 或 1..=64（建議；與 VECTOR 位無關）|
<!-- END GENERATED HEADER POLICY -->

### ABI 哈希（生成）
下表是根據實現生成的，並列出了受支持策略的規範 `abi_hash` 值。

<!-- BEGIN GENERATED ABI HASHES -->
|政策 | abi_hash（十六進制）|
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- 小更新可能會在 `feature_bits` 後面添加指令並保留操作碼空間；主要更新可能會更改編碼或僅與協議升級一起刪除/重新調整用途。
- 系統調用範圍穩定；對於活動的 `abi_version` 未知，產生 `E_SCALL_UNKNOWN`。
- Gas Schedule 與 `version` 綁定，並且需要更改時的黃金向量。

檢查工件
- 使用 `ivm_tool inspect <file.to>` 獲得穩定的標頭字段視圖。
- 對於開發，示例/包括一個小型 Makefile 目標 `examples-inspect`，該目標對構建的工件運行檢查。

示例（Rust）：最小魔法+尺寸檢查

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

注意：超出魔法範圍的確切標題佈局是版本化的並且是實現定義的；更喜歡 `ivm_tool inspect` 以獲得穩定的字段名稱和值。