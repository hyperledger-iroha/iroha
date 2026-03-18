<!-- Japanese translation of docs/source/zk_envelopes.md -->

---
lang: ja
direction: ltr
source: docs/source/zk_envelopes.md
status: complete
translator: manual
---

# ZK エンベロープ（Norito）

このページでは、Iroha 2 コードベースのネイティブ検証器が使用する Norito エンコードのエンベロープ形式を定義します。エンベロープはバージョン付きで決定的、かつクライアント／IVM／ノード間で移植可能です。

対象範囲（現時点）
- IPA（透過型・トラステッドセットアップなし）: Halo2 風フローで用いる多項式開平証明。エンベロープ型: `OpenVerifyEnvelope`
- STARK（FRI 系）: SHA-256 マークルコミットメントと決定的トランスクリプトを用いた 2^k ドメインでの多段 Fold 一貫性証明

バックエンドタグ
- IPA (Pallas, native): `halo2/ipa-v1/poly-open`
- IPA (BN254): `halo2/ipa/ipa-v1/poly-open`
- IPA (Goldilocks): `halo2/goldilocks-ipa-v1/poly-open`
- STARK (native): `stark/fri-v1/<profile>`（例: `stark/fri-v1/sha256-goldilocks-v1`）

一般的な注意事項
- エンベロープおよびネストされたペイロードのエンコードには Norito を用います。特に記載のない限り、スカラーは構造体型に従ったリトルエンディアンです。
- 決定性: チャレンジは固定のトランスクリプトラベルとバイト列から導出されます。ハッシュは STARK エンベロープでは SHA-256、IPA では SHA3 を使用。
- サイズ制限と検証: 実装者はベクタ長の上限を設定し、不正なペイロードは早期に拒否する必要があります（現在の制限はコード参照）。

## IPA: 多項式開平エンベロープ

`crates/iroha_zkp_halo2` に実装されたワイヤ型

- `IpaParams`（透過パラメータ）
  - `version: u16` — フォーマットバージョン（現在 1）
  - `curve_id: u16` — バックエンド ID（`1 = Pallas`, `2 = Goldilocks`）
  - `n: u32` — ベクタ長（2 の累乗）
  - `g: Vec<[u8; 32]>` — G ベクタ（圧縮曲線ポイント）
  - `h: Vec<[u8; 32]>` — H ベクタ（圧縮曲線ポイント）
  - `u: [u8; 32]` — U ジェネレータ（圧縮曲線ポイント）

- `IpaProofData`
  - `version: u16` — 現在 1
  - `l: Vec<[u8; 32]>` — 各ラウンドの L コミットメント
  - `r: Vec<[u8; 32]>` — 各ラウンドの R コミットメント
  - `a_final: [u8; 32]` — 証人ベクタに対する最終スカラー
  - `b_final: [u8; 32]` — 公開ベクタに対する最終スカラー

- `PolyOpenPublic`
  - `version: u16`
  - `curve_id: u16`
  - `n: u32`
  - `z: [u8; 32]` — 評価点（正規フィールド表現）
  - `t: [u8; 32]` — 主張する f(z)
  - `p_g: [u8; 32]` — `g` 下での係数コミットメント

- `OpenVerifyEnvelope`
  - `params: IpaParams`
  - `public: PolyOpenPublic`
  - `proof: IpaProofData`
  - `transcript_label: String` — 証明者・検証者双方が共有するラベル

ネイティブ IPA 検証器の挙動
- 公開ベクタ `b = [1, z, z^2, ..., z^{n-1}]` を再構築。
- トランスクリプトの各ラウンドを再生してジェネレータを折り畳み、Q を更新。
- 最終関係式が `(a_final, b_final)` で成立することを確認。
- トランスクリプトは crate 定義の DST の下で SHA3-256 を利用。
- バッチヘルパー: `verify_open_batch` はデフォルト設定で複数エンベロープを検証し、`verify_open_batch_with_options` は `BatchOptions` を受け取りシーケンシャル実行や rayon 並列最大数 (`Parallelism::{Sequential, Auto, Limited}`) を制御。

Rust 例:
```rust
use iroha_zkp_halo2::{Params, Polynomial, PrimeField64, Transcript, norito_helpers as nh};
let n = 8; let params = Params::new(n).unwrap();
let coeffs = (0..n).map(|i| PrimeField64::from((i+1) as u64)).collect();
let poly = Polynomial::from_coeffs(coeffs);
let p_g = poly.commit(&params).unwrap();
let z = PrimeField64::from(3u64);
let mut tr = Transcript::new("IROHA-TEST-IPA");
let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
let env = iroha_zkp_halo2::OpenVerifyEnvelope {
    params: nh::params_to_wire(&params),
    public: nh::poly_open_public::<iroha_zkp_halo2::backend::pallas::PallasBackend>(
        n,
        z,
        t,
        p_g,
    ),
    proof: nh::proof_to_wire(&proof),
    transcript_label: "IROHA-TEST-IPA".into(),
};
let bytes = norito::to_bytes(&env).unwrap();
```

JSON 風アノテーション:
```jsonc
{
  "params": {
    "version": 1,
    "curve_id": 1,
    "n": 8,
    "g": ["0x...", "0x..."],
    "h": ["0x...", "0x..."],
    "u": "0x..."
  },
  "public": {
    "version": 1,
    "curve_id": 1,
    "n": 8,
    "z": "0x...",
    "t": "0x...",
    "p_g": "0x..."
  },
  "proof": {
    "version": 1,
    "l": ["0x...", "0x..."],
    "r": ["0x...", "0x..."],
    "a_final": "0x...",
    "b_final": "0x..."
  },
  "transcript_label": "IROHA-TEST-IPA"
}
```

## STARK: FRI スタイル多段 Fold エンベロープ

ハッシュとトランスクリプト
- 葉: `LEAF || u64_le(value)` を SHA-256 でハッシュ。
- 内部ノード: SHA-256(left || right)。
- 各層チャレンジ `r_k = H(label || version || n_log2 || root_k)` をフィールドに対応付け。
- フィールド: Goldilocks 型素数 `p = 2^64 - 2^32 + 1`（テストバックエンド）。

`crates/iroha_core::zk_stark` に実装されたワイヤ型

- `StarkFriParamsV1`
  - `version: u16`
  - `n_log2: u8` — 評価ドメインの log2 サイズ

- `MerklePath`
  - `dirs: Vec<u8>` — 進行方向ビット（下位ビットが最下層）
  - `siblings: Vec<[u8; 32]>` — 葉からルートまでのシブリングハッシュ

- `StarkCommitmentsV1`
  - `version: u16`
  - `roots: Vec<[u8; 32]>` — 0…L レイヤのルート
  - `comp_root: Option<[u8; 32]>` — 合成葉のルート（クエリインデックスごとに 1 つ）

- `FoldDecommitV1`
  - `j: u32` — レイヤでのインデックス
  - `y0: u64`, `y1: u64` — 位置 (2*j, 2*j+1) の値
  - `path_y0`, `path_y1`: MerklePath
  - `z: u64` — 折り畳み値 `y0 + r_k*y1`
  - `path_z`: MerklePath — `roots[k+1]` 下の `z` 証明

- `StarkProofV1`
  - `version: u16`
  - `commits: StarkCommitmentsV1`
  - `queries: Vec<Vec<FoldDecommitV1>>` — クエリごとのチェーン
  - `comp_values: Option<Vec<StarkCompositionValueV1>>`

- `StarkCompositionValueV1`
  - `leaf: u64`
  - `constant: u64`
  - `z_coeff: u64`
  - `aux_terms: Vec<StarkCompositionTermV1>`
  - `path: MerklePath`

- `StarkCompositionTermV1`
  - `wire_index: u32`
  - `value: u64`
  - `coeff: u64`

- `StarkVerifyEnvelopeV1`
  - `params: StarkFriParamsV1`
  - `proof: StarkProofV1`
  - `transcript_label: String`

ネイティブ STARK 検証器の挙動
- クエリごとに折り畳みを再生:
  - `y0`, `y1` を `roots[k]` 下で検証し、`z` を `roots[k+1]` 下で検証。
  - フィールド内で `z == y0 + r_k*y1` を確認。
- `comp_root` が存在する場合、合成葉とパスを検証し `constant + z_coeff*z_final + Σ coeff_i*value_i` と一致することを確認。`wire_index` は昇順でなければならない。

Rust 例:
```rust
use iroha_core::zk_stark::*;
let n_log2 = 3u8;
let env = StarkVerifyEnvelopeV1 { /* params, proof, transcript */ };
let bytes = norito::to_bytes(&env).unwrap();
```

JSON 風（省略）:
```jsonc
{
  "params": {
    "version": 1,
    "n_log2": 3
  },
  "proof": {
    "version": 1,
    "commits": {
      "version": 1,
      "roots": ["0x...", "0x..."],
      "comp_root": "0x..."
    },
    "queries": [
      [
        { "j": 2, "y0": 42, "y1": 99, "z": 123, ... },
        { "j": 1, "y0": 876, "y1": 111, "z": 222, ... }
      ]
    ],
    "comp_values": [
      {
        "leaf": 555,
        "constant": 7,
        "z_coeff": 3,
        "aux_terms": [
          {"wire_index": 0, "value": 42, "coeff": 9},
          {"wire_index": 1, "value": 99, "coeff": 5}
        ],
        "path": { "dirs": [0b010], "siblings": ["0x...", "0x..."] }
      }
    ]
  },
  "transcript_label": "IROHA-STARK-TEST"
}
```
