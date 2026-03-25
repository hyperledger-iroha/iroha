---
lang: zh-hans
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

本页显示了简洁的 Kotodama 示例以及它们如何映射到 IVM 系统调用和指针 ABI 参数。另请参阅：
- `examples/` 用于可运行源
- `docs/source/ivm_syscalls.md` 用于规范系统调用 ABI
- `kotodama_grammar.md` 完整的语言规范

## 您好 + 帐户详细信息

来源：`examples/hello/hello.ko`

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

映射（指针-ABI）：
- `authority()` → `SCALL 0xA4`（主机将 `&AccountId` 写入 `r10`）
- `set_account_detail(a, k, v)` → 移动 `r10=&AccountId`、`r11=&Name`、`r12=&Json`，然后移动 `SCALL 0x1A`

## 资产转移

来源：`examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
      account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

映射（指针-ABI）：
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`、`r11=&AccountId(to)`、`r12=&AssetDefinitionId(def)`、`r13=amount`，然后是 `SCALL 0x24`

## NFT创建+转移

来源：`examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let recipient = account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

映射（指针-ABI）：
- `nft_mint_asset(id, owner)` → `r10=&NftId`、`r11=&AccountId(owner)`、`SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`、`r11=&NftId`、`r12=&AccountId(to)`、`SCALL 0x26`

## 指针 Norito 帮助程序

指针值持久状态需要在类型化 TLV 与
`NoritoBytes` 主机持续存在的信封。 Kotodama 现在连接这些助手
直接通过编译器，因此构建者可以使用指针默认值和映射
无需手动 FFI 粘合的查找：

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

降低：

- 发布类型化 TLV 后，指针默认发出 `POINTER_TO_NORITO`，因此
  主机接收规范的 `NoritoBytes` 有效负载进行存储。
- 读取使用 `POINTER_FROM_NORITO` 执行相反操作，提供
  `r11` 中预期的指针类型 ID。
- 两条路径都会自动将文字 TLV 发布到 INPUT 区域，从而允许
  透明地混合字符串文字和运行时指针的契约。

有关运行时回归，请参阅 `crates/ivm/tests/kotodama_pointer_args.rs`
针对 `MockWorldStateView` 进行往返练习。

## 确定性映射迭代（设计）

每个的确定性映射都需要一个界限。多入口迭代需要状态图；编译器接受 `.take(n)` 或声明的最大长度。

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

语义：
- 迭代集是循环入口处的快照；顺序按密钥的 Norito 字节字典顺序排列。
- `E_ITER_MUTATION` 循环陷阱中 `M` 的结构突变。
- 如果没有限制，编译器会发出 `E_UNBOUNDED_ITERATION`。

## 编译器/主机内部（Rust，不是 Kotodama 源）

下面的代码片段位于工具链的 Rust 端。它们说明了编译器帮助程序和 VM 降低机制，并且**不是**有效的 Kotodama `.ko` 源。

## 宽操作码分块帧更新

Kotodama 的宽操作码帮助程序针对 IVM 使用的 8 位操作数布局
宽编码。移动 128 位值的加载和存储重用第三个操作数
高位寄存器的插槽，因此基址寄存器必须已经保存了最终的
地址。在发出加载/存储之前，使用 `ADDI` 调整底座：

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```分块帧更新以 16 字节为步长推进基数，确保寄存器
由 `STORE128` 提交的对落在所需的对齐边界上。一样的
模式适用于 `LOAD128`；之前发出具有所需步幅的 `ADDI`
每次加载都将高目标寄存器绑定到第三个操作数槽。
未对齐的地址陷阱为 `VMError::MisalignedAccess`，与 VM 匹配
在 `crates/ivm/tests/wide_memory128.rs` 中执行的行为。

发出这些 128 位帮助器的程序必须通告向量功能。
Kotodama 编译器在任何时候都会自动启用 `VECTOR` 模式位
`LOAD128`/`STORE128` 出现； VM 陷阱
`VMError::VectorExtensionDisabled` 如果程序尝试执行它们
没有设置该位。

## 宽条件分支降低

当 Kotodama 将 `if`/`else` 或三元分支降低为宽字节码时，它会发出
修复了 `BNE cond, zero, +2` 序列，后跟一对 `JAL` 指令：

1. 短 `BNE` 将条件分支保持在 8 位立即通道内
   通过跳过失败 `JAL`。
2. 第一个 `JAL` 的目标是 `else` 块（当条件满足时执行
   假）。
3. 第二个 `JAL` 跳转到 `then` 块（当条件为
   正确）。

这种模式保证条件检查永远不需要编码更大的偏移量
超过 ±127 个字，同时仍支持 `then` 的任意大主体
和 `else` 通过宽 `JAL` 帮助器进行块。参见
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` 为
锁定序列的回归测试。

### 降低示例

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

编译为以下宽指令框架（寄存器号和
绝对偏移量取决于封闭函数）：

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

后续指令具体化常量并写入返回值。
因为 `BNE` 跳过第一个 `JAL`，所以条件偏移量始终为
`+2`字，即使块体扩展时也将分支保持在范围内。