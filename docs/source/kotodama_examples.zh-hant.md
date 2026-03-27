---
lang: zh-hant
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama 示例概述

本頁顯示了簡潔的 Kotodama 示例以及它們如何映射到 IVM 系統調用和指針 ABI 參數。另請參閱：
- `examples/` 用於可運行源
- `docs/source/ivm_syscalls.md` 用於規範系統調用 ABI
- `kotodama_grammar.md` 完整的語言規範

## 您好 + 帳戶詳細信息

來源：`examples/hello/hello.ko`

```
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

映射（指針-ABI）：
- `authority()` → `SCALL 0xA4`（主機將 `&AccountId` 寫入 `r10`）
- `set_account_detail(a, k, v)` → 移動 `r10=&AccountId`、`r11=&Name`、`r12=&Json`，然後移動 `SCALL 0x1A`

## 資產轉移

來源：`examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

映射（指針-ABI）：
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`、`r11=&AccountId(to)`、`r12=&AssetDefinitionId(def)`、`r13=amount`，然後是 `SCALL 0x24`

## NFT創建+轉移

來源：`examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let recipient = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

映射（指針-ABI）：
- `nft_mint_asset(id, owner)` → `r10=&NftId`、`r11=&AccountId(owner)`、`SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`、`r11=&NftId`、`r12=&AccountId(to)`、`SCALL 0x26`

## 指針 Norito 幫助程序

指針值持久狀態需要在類型化 TLV 與
`NoritoBytes` 主機持續存在的信封。 Kotodama 現在連接這些助手
直接通過編譯器，因此構建者可以使用指針默認值和映射
無需手動 FFI 粘合的查找：

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

降低：

- 發布類型化 TLV 後，指針默認發出 `POINTER_TO_NORITO`，因此
  主機接收規範的 `NoritoBytes` 有效負載進行存儲。
- 讀取使用 `POINTER_FROM_NORITO` 執行相反操作，提供
  `r11` 中預期的指針類型 ID。
- 兩條路徑都會自動將文字 TLV 發佈到 INPUT 區域，從而允許
  透明地混合字符串文字和運行時指針的契約。

有關運行時回歸，請參閱 `crates/ivm/tests/kotodama_pointer_args.rs`
針對 `MockWorldStateView` 進行往返練習。

## 確定性映射迭代（設計）

每個的確定性映射都需要一個界限。多入口迭代需要狀態圖；編譯器接受 `.take(n)` 或聲明的最大長度。

```
// design example (iteration requires bounds and state storage)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

語義：
- 迭代集是循環入口處的快照；順序按密鑰的 Norito 字節字典順序排列。
- `E_ITER_MUTATION` 循環陷阱中 `M` 的結構突變。
- 如果沒有限制，編譯器會發出 `E_UNBOUNDED_ITERATION`。

## 編譯器/主機內部（Rust，不是 Kotodama 源）

下面的代碼片段位於工具鏈的 Rust 端。它們說明了編譯器幫助程序和 VM 降低機制，並且**不是**有效的 Kotodama `.ko` 源。

## 寬操作碼分塊幀更新

Kotodama 的寬操作碼幫助程序針對 IVM 使用的 8 位操作數佈局
寬編碼。移動 128 位值的加載和存儲重用第三個操作數
高位寄存器的插槽，因此基址寄存器必須已經保存了最終的
地址。在發出加載/存儲之前，使用 `ADDI` 調整底座：

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```分塊幀更新以 16 字節為步長推進基數，確保寄存器
由 `STORE128` 提交的對落在所需的對齊邊界上。一樣的
模式適用於 `LOAD128`；之前發出具有所需步幅的 `ADDI`
每次加載都將高目標寄存器綁定到第三個操作數槽。
未對齊的地址陷阱為 `VMError::MisalignedAccess`，與 VM 匹配
在 `crates/ivm/tests/wide_memory128.rs` 中執行的行為。

發出這些 128 位幫助器的程序必須通告向量功能。
Kotodama 編譯器在任何時候都會自動啟用 `VECTOR` 模式位
`LOAD128`/`STORE128` 出現； VM 陷阱
`VMError::VectorExtensionDisabled` 如果程序嘗試執行它們
沒有設置該位。

## 寬條件分支降低

當 Kotodama 將 `if`/`else` 或三元分支降低為寬字節碼時，它會發出
修復了 `BNE cond, zero, +2` 序列，後跟一對 `JAL` 指令：

1. 短 `BNE` 將條件分支保持在 8 位立即通道內
   通過跳過失敗 `JAL`。
2. 第一個 `JAL` 的目標是 `else` 塊（當條件滿足時執行
   假）。
3. 第二個 `JAL` 跳轉到 `then` 塊（當條件為
   正確）。

這種模式保證條件檢查永遠不需要編碼更大的偏移量
超過 ±127 個字，同時仍支持 `then` 的任意大主體
和 `else` 通過寬 `JAL` 幫助器進行塊。參見
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` 為
鎖定序列的回歸測試。

### 降低示例

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

編譯為以下寬指令框架（寄存器號和
絕對偏移量取決於封閉函數）：

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

後續指令具體化常量並寫入返回值。
因為 `BNE` 跳過第一個 `JAL`，所以條件偏移量始終為
`+2`字，即使塊體擴展時也將分支保持在範圍內。