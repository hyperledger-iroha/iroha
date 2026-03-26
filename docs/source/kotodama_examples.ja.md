<!-- Japanese translation of docs/source/kotodama_examples.md -->

---
lang: ja
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
translator: manual
---

# Kotodama サンプル概要

このページではコンパクトな Kotodama サンプルと、それらが IVM のシステムコールおよびポインター ABI 引数へどのようにマッピングされるかを示します。併せて次も参照してください。
- 実行可能なソース: `examples/`
- 正準システムコール ABI: `docs/source/ivm_syscalls.md`
- 言語仕様全文: `kotodama_grammar.md`

## Hello + Account Detail

ソース: `examples/hello/hello.ko`

```kotodama
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

マッピング（ポインター ABI）:
- `authority()` → `SCALL 0xA4`（ホストが `&AccountId` を `r10` に書き込む）
- `set_account_detail(a, k, v)` → `r10=&AccountId`, `r11=&Name`, `r12=&Json` に移動後、`SCALL 0x1A`

## Asset Transfer

ソース: `examples/transfer/transfer.ko`

```kotodama
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ"),
      account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

マッピング（ポインター ABI）:
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount` に配置後 `SCALL 0x24`

## NFT Create + Transfer

ソース: `examples/nft/nft.ko`

```kotodama
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let recipient = account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

マッピング（ポインター ABI）:
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`、`SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`、`SCALL 0x26`

## 決定論的な Map 反復（設計）

決定論的なマップ for-each には境界が必要です。複数件の反復には state マップが必要で、コンパイラは `.take(n)` もしくは事前宣言された最大長を受け付けます。

```kotodama
// 設計例（反復には境界と state 保存が必須）
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

セマンティクス:
- 反復集合はループ開始時点のスナップショットです。順序はキーの Norito バイト列に基づく辞書順昇順で確定しています。
- ループ中に `M` を構造的に変更すると `E_ITER_MUTATION` でトラップします。
- 境界が無い場合、コンパイラは `E_UNBOUNDED_ITERATION` を発生させます。

## ワイド opcode によるチャンク化フレーム更新

Kotodama のワイド opcode ヘルパーは IVM のワイドエンコーディングで使われる 8 ビットオペランド配置を対象としています。128 ビット値を移動するロード・ストア命令は第 3 オペランドスロットを高位レジスタに再利用するため、ベースレジスタに目的アドレスを事前に保持させる必要があります。ロード／ストアを実行する前に `ADDI` でベースを調整してください。

```rust
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```

チャンク化されたフレーム更新ではベースを 16 バイトずつ進め、`STORE128` がコミットするレジスタペアが必須アライン境界に落ちるよう保証します。同じパターンは `LOAD128` にも適用され、各ロードの前に目的のストライドで `ADDI` を発行することで、第 3 オペランドスロットに束縛された高位レジスタを維持します。不正なアドレスアラインは `VMError::MisalignedAccess` を発生させ、`crates/ivm/tests/wide_memory128.rs` で検証されている VM の挙動と一致します。

これらの 128 ビットヘルパーを発行するプログラムはベクトル機能の有効化を宣言する必要があります。Kotodama コンパイラは `LOAD128` / `STORE128` を検出すると自動的に `VECTOR` モードビットをセットします。ビットが無効なまま命令を実行した場合、VM は `VMError::VectorExtensionDisabled` でトラップします。

## ワイド条件分岐のローアリング

Kotodama が `if` / `else`（または三項演算子）をワイドバイトコードへローアリングする際、固定の `BNE cond, zero, +2` と 2 つの `JAL` で構成されるシーケンスを生成します。

1. 短い `BNE` により条件分岐が 8 ビット即値幅に収まり、フォールスルー側の `JAL` を飛び越えます。
2. 最初の `JAL` は条件が偽のときに実行される `else` ブロックを指します。
3. 2 番目の `JAL` は条件が真の場合に実行される `then` ブロックへジャンプします。

このパターンにより、条件分岐のオフセットは常に ±127 ワード以内に収まります。同時に、`then` / `else` ブロック本体はワイド `JAL` を介して任意に大きなサイズを扱えます。回帰テスト `crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` がこのシーケンスを固定化しています。

### ローアリング例

```kotodama
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

上記は次のワイド命令スケルトンへとコンパイルされます（レジスタ番号と絶対オフセットは関数コンテキストに依存）。

```
BNE cond_reg, x0, +2    # 条件が真ならフォールスルーの JAL をスキップ
JAL x0, else_offset     # 条件が偽の場合に実行
JAL x0, then_offset     # 条件が真の場合に実行
```

続く命令で定数を生成し戻り値を書き込みます。`BNE` が最初の `JAL` を飛び越えるため、条件分岐のオフセットは常に `+2` ワードとなり、ブロック本体が拡張されてもレンジ内に収まります。
