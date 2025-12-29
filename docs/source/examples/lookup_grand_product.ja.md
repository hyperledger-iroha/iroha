<!-- Japanese translation of docs/source/examples/lookup_grand_product.md -->

---
lang: ja
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
translator: manual
---

# ルックアップ Grand-Product の例

この例は Stage 2 パイプラインで採用している FASTPQ 権限ルックアップ引数を解説します。プルーバーは選択子 (`s_perm`) と目撃値 (`perm_hash`) を低次数延長 (LDE) ドメイン上で評価し、累積グランドプロダクト `Z_i` を更新します。得られた系列を Poseidon でハッシュして `fastpq:v1:lookup:product` ドメインとしてトランスクリプトに吸収しつつ、最終的な `Z_i` はコミット済みの権限テーブル積 `T` と一致させます。

小さなバッチで selector が次のように設定されているとします。

| row | `s_perm` | `perm_hash` |
| --- | --- | --- |
| 0 | 1 | `0x019a...`（ロール auditor を付与、パーミッション transfer_asset） |
| 1 | 0 | `0xabcd...`（権限変更なし） |
| 2 | 1 | `0x42ff...`（ロール auditor を剥奪、パーミッション burn_asset） |

トランスクリプトから得られる Fiat-Shamir ルックアップチャレンジを `gamma = 0xdead...` とします。プルーバーは `Z_0 = 1` から始め、各行を折り畳みます。

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1  (セレクタが 0)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

`s_perm = 0` の行はアキュムレータに影響しません。トレース処理後、プルーバーは `[Z_1, Z_2, …]` を Poseidon でハッシュしてトランスクリプトに公開し、同時に境界条件の検証用として `Z_final = Z_3` も保持します。

一方、テーブル側ではコミット済みのパーミッション Merkle ツリーがスロット時点の権限集合を表し、ベリファイア（もしくは証人生成時のプルーバー）は以下を計算します。

```
T = Π (entry.hash + gamma)
```

プロトコルは境界条件 `Z_final / T = 1` を強制します。トレースがテーブルに存在しない権限を導入した、または存在する権限を欠落させた場合、比率が 1 から逸脱しベリファイアは拒否します。両側とも Goldilocks 体で `(値 + gamma)` を掛け合わせるため、CPU/GPU バックエンドを切り替えても結果は一致します。

Norito JSON でフィクスチャ化する際は、各行の `perm_hash`、セレクタ、更新後アキュムレータを記録します。

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

`0x...` のプレースホルダは自動テストを生成する際に具体的な Goldilocks フィールド要素へ置き換えてください。Stage 2 のフィクスチャはこの配列を Poseidon でハッシュした値も保持しますが、JSON 構造は同一なのでテストベクタのテンプレートとして再利用できます。
