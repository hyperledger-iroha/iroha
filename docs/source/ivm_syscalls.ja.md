<!-- Japanese translation of docs/source/ivm_syscalls.md -->

---
lang: ja
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
translator: manual
---

# IVM システムコール ABI

本ドキュメントは、IVM におけるシステムコール番号、ポインター ABI の呼び出し規約、予約済み番号領域、そして Kotodama のローアリングから利用されるコントラクト向けシステムコールの正準表を定義します。内容は `ivm.md`（アーキテクチャ）および `kotodama_grammar.md`（言語仕様）を補完します。

## バージョン管理
- 認識されるシステムコール集合はバイトコードヘッダーの `abi_version` フィールドに依存します。初回リリースでは `abi_version = 1` のみを受け付け、他の値はアドミッションで拒否されます。アクティブな `abi_version` に存在しない番号は決定論的に `E_SCALL_UNKNOWN` でトラップします。
- ランタイムアップグレードは `abi_version = 1` を維持し、syscall や pointer‑ABI 型を拡張しません。
- システムコールのガスコストは、バイトコードヘッダーのバージョンに結びついたバージョン管理付きガススケジュールの一部です。詳細は `ivm.md`（ガスポリシー）を参照してください。

## 番号レンジ
- `0x00..=0x1F`: VM コア／ユーティリティ（開発用ヘルパー。`CoreHost` では利用不可）。
- `0x20..=0x5F`: Iroha コア ISI ブリッジ（ABI v1 で安定）。
- `0x60..=0x7F`: プロトコル機能でゲートされる拡張 ISI（有効時も ABI v1 の一部）。
- `0x80..=0xFF`: ホスト／暗号ヘルパーと予約スロット。ABI v1 の allowlist に含まれる番号のみ許可。

## Durable ヘルパー（ABI v1）
- Durable 状態ヘルパーのシステムコール（0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*、JSON/SCHEMA エンコード／デコード）は V1 ABI の一部であり、`abi_hash` の計算に含まれます。
- CoreHost は STATE_{GET,SET,DEL} を WSV に裏付けられた永続スマートコントラクト状態へ接続します。dev/test ホストはオーバーレイやローカル永続化を使ってもよいが、システムコールの意味論は同一でなければなりません。

## ポインター ABI の呼び出し規約（スマートコントラクト向けシステムコール）
- 引数はレジスタ `r10+` に生の `u64` 値として配置するか、INPUT 領域内の不変な Norito TLV エンベロープ（例: `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`）へのポインターとして渡します。
- スカラ戻り値はホストが返す `u64` です。ポインター戻り値はホストが `r10` に書き込みます。

## 正準システムコール表（抜粋）

| Hex  | 名前                              | 引数（`r10+`）                                                            | 戻り値       | ガス（基礎 + 変動）               | 備考                                   |
|------|-----------------------------------|---------------------------------------------------------------------------|--------------|-----------------------------------|----------------------------------------|
| 0x1A | SET_ACCOUNT_DETAIL                | `&AccountId`, `&Name`, `&Json`                                           | `u64=0`      | `G_set_detail + bytes(val)`       | アカウントにディテールを設定する      |
| 0x22 | MINT_ASSET                        | `&AccountId`, `&AssetDefinitionId`, `amount:u64`                         | `u64=0`      | `G_mint`                          | 指定量をアカウントへ鋳造する          |
| 0x23 | BURN_ASSET                        | `&AccountId`, `&AssetDefinitionId`, `amount:u64`                         | `u64=0`      | `G_burn`                          | 指定量をアカウントから焼却する        |
| 0x24 | TRANSFER_ASSET                    | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `amount:u64` | `u64=0`      | `G_transfer`                      | アカウント間で資産を転送する          |
| 0x29 | TRANSFER_V1_BATCH_BEGIN           | –                                                                        | `u64=0`      | `G_transfer`                      | FASTPQ 転送バッチを開始し後続の転送を集約 |
| 0x2A | TRANSFER_V1_BATCH_END             | –                                                                        | `u64=0`      | `G_transfer`                      | バッチを閉じて 1 つの命令としてコミット |
| 0x2B | TRANSFER_V1_BATCH_APPLY           | `r10=&NoritoBytes(TransferAssetBatch)`                                   | `u64=0`      | `G_transfer`                      | Norito エンコード済みのバッチを 1 回の呼び出しで適用 |
| 0x25 | NFT_MINT_ASSET                    | `&NftId`, `&AccountId(owner)`                                           | `u64=0`      | `G_nft_mint_asset`                | 新しい NFT を登録する                  |
| 0x26 | NFT_TRANSFER_ASSET                | `&AccountId(from)`, `&NftId`, `&AccountId(to)`                           | `u64=0`      | `G_nft_transfer_asset`            | NFT の所有権を移転する                |
| 0x27 | NFT_SET_METADATA                  | `&NftId`, `&Json`                                                       | `u64=0`      | `G_nft_set_metadata`              | NFT メタデータを更新する              |
| 0x28 | NFT_BURN_ASSET                    | `&NftId`                                                                | `u64=0`      | `G_nft_burn_asset`                | NFT を焼却（削除）する               |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS         | –                                                                        | `u64=count`  | `G_create_nfts_for_all`           | ヘルパー。機能フラグでゲート          |
| 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64`                                                              | `u64=prev`   | `G_set_depth`                     | 管理者向け。機能フラグでゲート        |
| 0xA4 | GET_AUTHORITY                     | –（ホストが結果を書き込む）                                                | `&AccountId` | `G_get_auth`                      | 現在の authority を `r10` に書き込む    |
| 0xF7 | GET_MERKLE_PATH                   | `addr:u64`, `out_ptr:u64`, 任意 `root_out:u64`                           | `u64=len`    | `G_mpath + len`                  | 経路（葉→ルート）と必要ならルートを書き込む |
| 0xFA | GET_MERKLE_COMPACT                | `addr:u64`, `out_ptr:u64`, 任意 `depth_cap:u64`, 任意 `root_out:u64`     | `u64=depth`  | `G_mpath + depth`                | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` 形式 |
| 0xFF | GET_REGISTER_MERKLE_COMPACT       | `reg_index:u64`, `out_ptr:u64`, 任意 `depth_cap:u64`, 任意 `root_out:u64`| `u64=depth`  | `G_mpath + depth`                | レジスタコミットメント用の同形式       |

## 備考
- すべてのポインター引数は INPUT 領域内の Norito TLV エンベロープを参照し、初回デリファレンス時に検証されます（不正な場合は `E_NORITO_INVALID`）。
- すべてのミューテーションは Iroha 標準のエグゼキューター（`CoreHost`）経由で適用され、VM が直接書き換えることはありません。
- 正確なガス定数（`G_*`）はアクティブなガススケジュールで定義されています。詳細は `ivm.md` を参照してください。

## エラー
- `E_SCALL_UNKNOWN`: アクティブな `abi_version` で認識されないシステムコール番号。
- 入力検証エラーは VM トラップとして伝播します（例: 不正な TLV に対する `E_NORITO_INVALID`）。

## クロスリファレンス
- アーキテクチャと VM セマンティクス: `ivm.md`
- 言語およびビルトイン対応表: `docs/source/kotodama_grammar.md`

## 生成メモ
- システムコール定数の完全な一覧は以下で生成できます。
  - `make docs-syscalls` → `docs/source/ivm_syscalls_generated.md` を生成
  - `make check-docs` → 生成済みテーブルが最新であることを検証（CI 向け）
- 上記の抜粋は、コントラクトが依存すべき安定した正準表として維持されます。

## 管理系／ロール向け TLV 例（モックホスト）

このセクションでは、テストで利用されるモック WSV ホストが受け付ける管理系システムコールの TLV 形状と最小 JSON ペイロードを示します。すべての引数は pointer-ABI を満たし（INPUT 内の Norito エンベロープ）、NFT やロール等を扱う本番ホストはよりリッチなスキーマを要求する場合があります。

- REGISTER_PEER / UNREGISTER_PEER
  - 引数: `r10=&Json`
  - JSON 例: `{ "peer": "peer-id-or-info" }`

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - 引数: `r10=&Json`
    - 最小 JSON: `{ "name": "t1" }`（追加フィールドはモックが無視）
  - REMOVE_TRIGGER:
    - 引数: `r10=&Name`（トリガー名）
  - SET_TRIGGER_ENABLED:
    - 引数: `r10=&Name`, `r11=enabled:u64`（0 = 無効、非 0 = 有効）

- ロール系: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - 引数: `r10=&Name`（ロール名）、`r11=&Json`（権限集合）
    - JSON は `
    JSON は `"perms"` または `"permissions"` のキーで、権限名の文字列配列を受け付けます。
    - 例:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:ed0120...@wonder", "transfer_asset:rose#wonder" ] }`
    - モックがサポートする権限名プレフィックス:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - 引数: `r10=&Name`
    - いずれかのアカウントにロールが割り当てられている場合は失敗します。
  - GRANT_ROLE / REVOKE_ROLE:
    - 引数: `r10=&AccountId`（対象）、`r11=&Name`（ロール名）

- 解除系（ドメイン／アカウント／資産）の不変条件（モック）
  - UNREGISTER_DOMAIN (`r10=&DomainId`): ドメインにアカウントまたは資産定義が残っていると失敗。
  - UNREGISTER_ACCOUNT (`r10=&AccountId`): 残高が非ゼロ、または NFT を保有していると失敗。
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`): 該当資産に残高が存在すると失敗。

## 補足
- 上記の例はテストで用いるモック WSV ホストを前提としています。本番ノードのホストは、より厳格な管理スキーマや追加検証を要求する可能性があります。ただし pointer-ABI の規約（INPUT 内の TLV、バージョン 1、型 ID の一致、ペイロードハッシュの検証）は常に適用されます。
