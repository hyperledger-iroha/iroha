<!-- Japanese translation of docs/source/examples/smt_update.md -->

---
lang: ja
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
translator: manual
---

# 疎メルクル更新の例

本例は Stage 2 パイプラインにおいて FASTPQ トレースが `neighbour_leaf` 列を使って非会員証明をエンコードする方法を示します。木構造は Poseidon2 フィールド要素上のバイナリ疎メルクルツリーで、キーは正規化された 32 バイト LE 文字列からフィールド要素にハッシュされ、最上位ビットが各レベルの枝選択を決めます。

## シナリオ

- 事前状態
  - `asset::alice::rose` → ハッシュキー `0x12b7...`、値 `0x0000_0000_0000_0005`
  - `asset::bob::rose`   → ハッシュキー `0x1321...`、値 `0x0000_0000_0000_0003`
- 更新要求: `asset::carol::rose` に値 2 を挿入。
- Carol の正規キーは 5 ビット接頭辞 `0b01011`。近傍は `0b01010`（Alice）と `0b01101`（Bob）。

`0b01011` で始まる葉が存在しないため、証明は区間 `(alice, bob)` が空であることを示す追加情報が必要です。Stage 2 では `path_bit_{level}`、`sibling_{level}`、`node_in_{level}`、`node_out_{level}`（`level = 0..31`）の各列に値を配置します。値はすべて Poseidon2 フィールド要素のリトルエンディアン表現です。

| level | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` | 備考 |
| --- | --- | --- | --- | --- | --- |
| 0 | 1 | `0x241f...`（Alice の葉） | `0x0000...` | `0x4b12...`（新値 2） | 挿入: 0 から開始し新値を設定 |
| 1 | 1 | `0x7d45...`（空右枝） | Poseidon2(`node_out_0`, `sibling_0`) | Poseidon2(`sibling_1`, `node_out_1`) | 接頭辞ビット 1 に従う |
| 2 | 0 | `0x03ae...`（Bob 分岐） | Poseidon2(`node_out_1`, `sibling_1`) | Poseidon2(`node_in_2`, `sibling_2`) | ビット 0 で左右入れ替え |
| 3 | 1 | `0x9bc4...` | Poseidon2(`node_out_2`, `sibling_2`) | Poseidon2(`sibling_3`, `node_out_3`) | 上位レベルで順次ハッシュ |
| 4 | 0 | `0xe112...` | Poseidon2(`node_out_3`, `sibling_3`) | Poseidon2(`node_in_4`, `sibling_4`) | ルートでポスト状態ルート生成 |

`neighbour_leaf` には Bob の葉（`key=0x1321...`, `value=3`, `hash=Poseidon2(key,value)=0x03ae...`）を格納します。AIR 検証では以下をチェックします:

1. 提供された neighbour がレベル 2 の sibling と一致する。
2. neighbour キーは挿入キーより辞書順で大きく、左 sibling（Alice）は小さい。
3. 挿入葉を neighbour に置き換えると事前ルートが再現される。

これにより、更新前に `(0b01010, 0b01101)` 区間に葉が存在しなかったことが証明されます。FASTPQ トレースを生成する実装はこのレイアウトをそのまま利用できます。数値は参考値であり、完全な JSON 証人を出力する際は上記表と同じ（level 添字付きの）列を Norito の JSON ヘルパーでシリアライズしてください。
