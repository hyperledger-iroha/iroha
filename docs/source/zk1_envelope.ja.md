<!-- Japanese translation of docs/source/zk1_envelope.md -->

---
lang: ja
direction: ltr
source: docs/source/zk1_envelope.md
status: complete
translator: manual
---

# ZK1 エンベロープ形式（証明／検証鍵コンテナ）

本ドキュメントは、テストやヘルパーで使用する ZK1 エンベロープ形式を定義します。ZK1 はバックエンド非依存に証明・検証鍵・公開インスタンスを運べる TLV コンテナで、4 バイトのマジックヘッダと型付きレコードの列で構成されます。パーサは決定的で安全性のためサイズ上限を強制します。

## コンテナ構造

- マジック: ASCII `ZK1\0`（4 バイト）
- 続いて 0 個以上の TLV:
  - `tag[4]`（ASCII）
  - `len[u32 LE]`
  - `payload[len]`

## 認識済み TLV

- `PROF`: 証明トランスクリプトの生バイト列（ZK1 では不透明）。検証器がペイロードを解釈。
- `IPAK`: Halo2 IPA パラメータ（Pasta 用／透過型）。ペイロードは `u32 k`（ドメインサイズ `N = 2^k` の指数）。検証器は `Params::<EqAffine>::new(k)` を構築。
- `H2VK`: Halo2 検証鍵バイト列（選択回路向け、処理済み形式推奨）。
- `I10P`: Pasta Fp 用のインスタンス列ブロック。レイアウトは `cols[u32] || rows[u32] || rows*cols*32` バイト。

注意:
- `I10P` は正規 32 バイト表現のみ受理。非正規値はデコーダが拒否。
- 複数インスタンス列は 1 つの TLV にパックすべき。テストでは単列で簡略化可。
- ZK1 自体はバックエンド非依存。証明／鍵には別途バックエンドタグ（例: `halo2/pasta/tiny-add-v1`、`halo2/pasta/tiny-add-public-v1`、`halo2/pasta/tiny-add-2rows-v1`）を添付。
- テストでは `tiny-add-v1`、`tiny-add-public-v1`、`tiny-add-2rows-v1` の決定的フィクスチャ証明/VK を生成する。他の回路 ID は実 VK/Proof を供給しない限りプレースホルダのペイロードを使用する。

## Rust 例

### Pasta/IPA（透過型）

```rust
let mut vk_env = zk1::wrap_start();
zk1::wrap_append_ipa_k(&mut vk_env, 5); // k = 5

let mut prf_env = zk1::wrap_start();
zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
zk1::wrap_append_instances_pasta_fp(&[public_scalar], &mut prf_env);
```

## ネガティブケース（テスト）

ZK1 デコーダ／検証器は不正なエンベロープを拒否します（バックエンドタグ不一致、インスタンスブロック切り捨て、非正規フィールド値など）。
