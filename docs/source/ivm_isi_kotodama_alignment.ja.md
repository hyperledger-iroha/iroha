<!-- Japanese translation of docs/source/ivm_isi_kotodama_alignment.md -->

---
lang: ja
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
translator: manual
---

# IVM ⇄ ISI ⇄ Data Model ⇄ Kotodama — アラインメントレビュー

このドキュメントは、Iroha Virtual Machine (IVM) の命令セットおよび syscall サーフェスが Iroha Special Instructions (ISI) や `iroha_data_model` とどのように対応しているか、また Kotodama がそれらのスタックへどのようにコンパイルされるかを監査します。現状のギャップを特定し、4 つのレイヤーが決定的かつ扱いやすく整合するための具体的な改善案を提示します。

バイトコードターゲットに関する注意: Kotodama スマートコントラクトは Iroha Virtual Machine (IVM) バイトコード（`.to`）へコンパイルされます。独立したアーキテクチャとして “risc5”/RISC-V をターゲットにはしていません。本書で言及される RISC-V 風のエンコーディングは IVM の混合命令形式に含まれるものであり、実装上の詳細にすぎません。

## 範囲と参照元
- IVM: `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` と `crates/ivm/docs/*`。
- ISI／データモデル: `crates/iroha_data_model/src/isi/*`、`crates/iroha_core/src/smartcontracts/isi/*`、および `docs/source/data_model_and_isi_spec.md`。
- Kotodama: `crates/kotodama_lang/src/*`、`crates/ivm/docs/*` のドキュメント。
- コア統合: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`。

用語
- 「ISI」: ワールドステートを変更するビルトイン命令型（例: RegisterAccount, Mint, Transfer）。エグゼキュータ経由で実行されます。
- 「Syscall」: IVM の `SCALL`（8 ビット番号）で、台帳操作をホストへ委譲します。

---

## 現状のマッピング（実装ベース）

### IVM 命令
- 算術、メモリ、制御フロー、暗号、ベクタ、ZK 補助命令は `instruction.rs` に定義され `ivm.rs` で実装されています。これらは自己完結的で決定的です。アクセラレーション経路（SIMD／Metal／CUDA）には CPU フォールバックが用意されています。
- システム／ホスト境界は `SCALL`（オペコード 0x60）経由です。番号は `syscalls.rs` に列挙され、ワールド操作（ドメイン／アカウント／アセットの登録・削除、ミント／バーン／トランスファ、ロールやパーミッション、トリガ関連）に加えて、`GET_PRIVATE_INPUT`、`COMMIT_OUTPUT`、`GET_MERKLE_PATH` などのヘルパーが含まれます。

### ホストレイヤー
- トレイト `IVMHost::syscall(number, &mut IVM)` は `host.rs` にあります。
- `DefaultHost` は非台帳系ヘルパー（alloc、ヒープ拡張、入出力、ZK プルーフヘルパ、機能検出）のみ実装し、ワールドステートの変更は行いません。
- デモ用の `WsvHost` が `mock_wsv.rs` に存在し、アセット操作のサブセット（Transfer/Mint/Burn）を小さなメモリ上 WSV にマッピングします。`AccountId`／`AssetDefinitionId` をレジスタ x10..x13 の整数 → ID マップ経由で扱うアドホックな実装です。

### ISI とデータモデル
- ビルトイン ISI 型とセマンティクスは `iroha_core::smartcontracts::isi::*` で実装され、`docs/source/data_model_and_isi_spec.md` に記載されています。
- `InstructionBox` は安定した「ワイヤ ID」と Norito エンコーディングを利用するレジストリを使用し、コアでのネイティブ実行ディスパッチが現行のコードパスです。

### IVM のコア統合
- `State::execute_trigger(..)` はキャッシュ済み IVM をクローンし、`CoreHost::with_accounts_and_args` を付与したうえで `load_program` と `run` を呼び出します。
- `CoreHost` は `IVMHost` を実装します。状態を変更する syscall はポインタ ABI の TLV レイアウトでデコードされ、ビルトイン ISI（`InstructionBox`）に変換されてキューへ積まれます。VM が返った後、ホストはこれら ISI を通常のエグゼキュータへ渡すため、パーミッション、インバリアント、イベント、テレメトリはネイティブ実行と同一になります。ワールドステートに触れないヘルパー syscall は引き続き `DefaultHost` に委譲されます。
- `executor.rs` は今もビルトイン ISI をネイティブに実行しており、バリデータエグゼキュータ自体を IVM へ移行するのは将来の課題です。

### Kotodama → IVM
- フロントエンド要素（レキサ／パーサ／最小セマンティクス／IR／レジスタ割り付け）は存在しています。
- コード生成（`kotodama::compiler`）は IVM 命令のサブセットを出力し、アセット操作には `SCALL` を使用します。
  - `MintAsset`: x10=account, x11=asset, x12=&NoritoBytes(Numeric) を設定し、`SCALL SYSCALL_MINT_ASSET`。
  - `BurnAsset`／`TransferAsset` も同様。
- `koto_*_demo.rs` のデモは `WsvHost` を利用し、整数インデックスと ID のマッピングで手軽なテストを実現しています。

---

## ギャップと不整合

1) **コアホストのカバレッジと整合性**  
   - 現状: `CoreHost` はコアに存在し、多くの台帳 syscall を標準パスで実行される ISI へ変換します。とはいえカバレッジは完全ではなく（ロール／パーミッション／トリガの一部 syscall はスタブのまま）、キューイングされた ISI がネイティブ実行と同一のステート／イベントを生むことを保証する整合性テストが必要です。

2) **syscall サーフェスと ISI／データモデルの命名・カバレッジ**  
   - NFT: syscall は `SYSCALL_NFT_*` 形式へ統一され、データモデルの `Nft` 型と整合しました。従来の `*_ASSET_INSTANCE` 系名称は後方互換のため非推奨エイリアスとして残しています。
   - ロール／パーミッション／トリガ: syscall リストは存在しますが、コアで各呼び出しがどの ISI へ結び付くのかを示す参照実装やマッピング表がありません。
   - パラメータ／セマンティクス: 一部 syscall にはパラメータエンコード（型付き ID かポインタか）やガスセマンティクスが明記されていません。一方 ISI セマンティクスは詳細に定義されています。

3) **VM／ホスト境界で型付きデータを受け渡す ABI**  
   - ポインタ ABI の TLV は `CoreHost` の `decode_tlv_typed` でデコードされ、決定的なパスを提供していますが、Kotodama 側ではまだ生のレジスタマッピングを使用しており、正式な ABI ツールチェーンが欠如しています。

4) **Kotodama デモが整数 ID マップに依存**  
   - `WsvHost` は ID をレジスタ上の整数にマップするデモ実装です。これは公式 ABI ではなく、コアと乖離した語彙を生みます。ポインタ＋Norito エンコードされた ID をホストへ渡す正式な経路が必要です。

5) **命名とドキュメントの分断**  
   - IVM ドキュメントは syscall を列挙していますが、データモデルや ISI ドキュメントには同じマッピングが示されていません。Kotodama の README も互換パスを説明していません。

---

## 改善提案

### A. コアホストの整備とテスト
- `CoreHost` と `StateTransaction` の橋渡しを完成させ、すべてのビルトイン ISI がポインタ ABI 経由で呼び出せるようにする。
- ISI ごとに E2E テストを追加し、`InstructionBox` 経由のネイティブ実行と IVM 経由の実行が全く同じステート更新・イベント・エラーを生成することを確認する。
- 影で動く比較テスト（「シャドウモード」）を導入し、VM がエンキューした ISI とネイティブ経路を突き合わせて差分を検出する。

### B. ABI と syscall 仕様の定義
- 資産量は `Numeric` なので NoritoBytes ポインタで渡し、その他の複合型も同様にポインタで渡す。
- すべての `SYSCALL_*` 番号に対応する `InstructionBox` を一覧化するリファレンスを作成する。
- リターン規約（成功で x10=1、失敗で x10=0、必要に応じて `VMError::HostRejected { code }`）を公式に定義する。
- ABI を Kotodama のコード生成と IVM テストに組み込み、整数→ID のデモマップを廃止する。

### C. Norito エンコードの再利用
- コア／Kotodama／テスト間で共有できる Norito エンコードヘルパー（`encode_account_id`, `encode_asset_definition_id` など）を導入する。
- ポインタ ABI の round-trip テストを整備し、VM メモリ→ホスト→ISI→VM メモリの往復が完全に決定的であることを CI で検証する（`crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs`）。

### D. エラーとガスの統一
- ホスト側で変換レイヤーを追加し、`InstructionExecutionError::{Find, Repetition, InvariantViolation, Math, Type, Mintability, InvalidParameter}` を特定の `VMError` コード、もしくは `x10=0/1` と `VMError::HostRejected { code }` を組み合わせた拡張リザルト規約へマッピングする。
- コアで syscall 用のガステーブルを導入し、IVM ドキュメントにも反映する。入力サイズに応じて予測可能で、プラットフォーム非依存なコスト体系を整える。

### E. 決定性と共有プリミティブ
- Merkle 木の統一（roadmap を参照）を完了し、`ivm::merkle_tree` を `iroha_crypto` と同一の葉・証明で置き換える／エイリアス化する。
- `SETVL`／`PARBEGIN`／`PAREND` はエンドツーエンドの決定性確認と決定的スケジューラ戦略が整うまで予約状態を維持し、現状 IVM がこれらのヒントを無視することを文書化する。
- アクセラレーション経路がバイト単位で同一の出力を生成することを保証し、困難な場合はフィーチャーフラグの背後に置き、CPU フォールバックと等価であることをテストで確認する。

### F. Kotodama コンパイラの配線
- コード生成を公式 ABI（B.）に拡張し、Norito エンコードされた ID や複合パラメータをポインタ経由で渡すようにする。整数→ID マップのデモを廃止。
- アセット以外（ドメイン／アカウント／ロール／パーミッション／トリガ）の syscall へ直接マップするビルトインを追加し、明確な名前付けを行う。
- 義務的な `permission(...)` アノテーションやコンパイル時チェックを追加し、静的証明ができない場合はランタイムホストエラーへフォールバックする。
- `crates/ivm/tests/kotodama.rs` に、小さなコントラクトをコンパイルして実行し、Norito 引数をデコードして一時 WSV を変更するテストホストを用いたエンドツーエンドのユニットテストを追加する。

### G. ドキュメントと開発者体験
- `docs/source/data_model_and_isi_spec.md` を更新し、syscall 対応表と ABI メモを追加する。
- `crates/ivm/docs/` に「IVM ホスト統合ガイド」を新設し、実際の `StateTransaction` 上で `IVMHost` を実装する手順を解説する。
- `README.md` やクレートドキュメントで、Kotodama が IVM `.to` バイトコードをターゲットとしていること、そして syscall がワールドステートへの橋渡しであることを明確化する。

---

## 推奨マッピング表（初期ドラフト）

代表的なサブセットです。ホスト実装の過程で最終化し、拡張します。

- `SYSCALL_REGISTER_DOMAIN(id: ptr DomainId)` → ISI `Register<Domain>`
- `SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId)` → ISI `Register<Account>`
- `SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8)` → ISI `Register<AssetDefinition>`
- `SYSCALL_MINT_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric))` → ISI `Mint<Numeric, Asset>`
- `SYSCALL_BURN_ASSET(account: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric))` → ISI `Burn<Numeric, Asset>`
- `SYSCALL_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId, asset: ptr AssetDefinitionId, amount: ptr NoritoBytes(Numeric))` → ISI `Transfer<Asset>`
- `SYSCALL_TRANSFER_V1_BATCH_BEGIN()` / `SYSCALL_TRANSFER_V1_BATCH_END()` → ISI `TransferAssetBatch`（バッチスコープの開始/終了を宣言し、各エントリは `transfer_asset` で順次適用される）
- `SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes<TransferAssetBatch>)` → 既に Norito でエンコードされたバッチを 1 回の呼び出しで適用
- `SYSCALL_NFT_MINT_ASSET(id: ptr NftId, owner: ptr AccountId)` → ISI `Register<Nft>`
- `SYSCALL_NFT_TRANSFER_ASSET(from: ptr AccountId, to: ptr AccountId, id: ptr NftId)` → ISI `Transfer<Nft>`
- `SYSCALL_NFT_SET_METADATA(id: ptr NftId, content: ptr Metadata)` → ISI `SetKeyValue<Nft>`
- `SYSCALL_NFT_BURN_ASSET(id: ptr NftId)` → ISI `Unregister<Nft>`
- `SYSCALL_CREATE_ROLE(id: ptr RoleId, role: ptr Role)` → ISI `Register<Role>`
- `SYSCALL_GRANT_ROLE(account: ptr AccountId, role: ptr RoleId)` → ISI `Grant<Role>`
- `SYSCALL_REVOKE_ROLE(account: ptr AccountId, role: ptr RoleId)` → ISI `Revoke<Role>`
- `SYSCALL_SET_PARAMETER(param: ptr Parameter)` → ISI `SetParameter`

Notes
- 「`ptr T`」は VM メモリに格納された Norito エンコード済みバイト列 T へのポインタをレジスタに保持することを意味します。ホストはそれを対応する `iroha_data_model` 型へデコードします。
- リターン規約: 成功時は `x10=1`、失敗時は `x10=0` を設定し、致命的エラーでは `VMError::HostRejected` を送出する場合があります。

---

## リスクと導入計画
- まずはホストを狭い範囲（アセット＋アカウント）で配線し、焦点を絞ったテストを追加する。
- ホスト側のセマンティクスが成熟するまで、ネイティブ ISI 実行を権威ある経路として維持し、両経路の結果やイベントが同一であることをテストの「シャドウモード」で検証する。
- 整合性が検証できたら、IVM トリガに対して IVM ホストを本番で有効化し、将来的には通常トランザクションも IVM 経由へルーティングすることを検討する。

---

## TODO トラッキング（2026-03-14 更新）

| 項目 | 担当 | 現状 | 次ステップ / 期日 | 完了基準 |
|------|------|------|--------------------|-----------|
| `iroha_core::smartcontracts::ivm::host::CoreHost` 実装 | Core Host チーム (`@core-host`) | `docs/source/ivm_core_host_design.md` の設計に基づき、`iroha_core/src/smartcontracts/ivm/host.rs` に StateTransaction ラッパーを追加したドラフトを `feature/ivm-core-host` ブランチで維持中。`Execute` トレイトに必要な引数変換は未実装。 | 2026-03-20 までに PR #13245 を再開し、`cargo test -p ivm host_roundtrip` と `cargo test -p iroha_core ivm_host_shadow_execute` を通過させる。 | `CoreHost::execute` が `Execute` と同じイベント/エラーを返し、`STATUS.md` の IVM 行を「shadow-ready」に更新。 |
| Kotodama Norito 引数ヘルパー | Kotodama WG (`@kotodama-wg`) | ✅ `encode_pointer_arg`/`decode_pointer_arg` をコード生成へ組み込み、ポインタ既定値と NoritoBytes 復元が IVM syscalls に直結。 | — | `cargo test -p ivm kotodama_pointer_args` を追加し、`docs/source/kotodama_examples.md` に使用例を追記済み。 |
| NFT 系 syscall の正式名称整備 | Streaming & NFT チーム (`@nft-streaming`) | ✅ `SYSCALL_NFT_MINT_ASSET` / `SYSCALL_NFT_SET_METADATA` などへリネーム済み。 | — | `cargo test -p ivm abi_syscall_list_golden` 更新済み。`docs/source/ivm_syscalls*.md` と本ドキュメントに新名称を反映。 |
| syscall ガステーブル／enforcement | IVM Core (`@ivm-core`) | `gas_table.toml` 案が issue #13108 に添付。コード側 (`iroha_core::smartcontracts::limits`) は placeholder のまま、IVM 実行時も検証なし。 | 2026-03-29 までに `iroha_core/src/smartcontracts/limits.rs` へ定数を移植し、`ivm/src/limits.rs` で読み込み。`cargo test -p iroha_core limits_enforcement` を追加。 | `docs/source/gas_model.md` に表を追加し、`cargo clippy --workspace -- -D warnings` および `cargo test --workspace` が通過。 |
| ポインタ ABI Norito 往復テスト | QA (`@qa-ivm`) | ✅ `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` を追加し、Manifest/NFT ポインタの往復検証を CI で実行中。 | — | `cargo test -p iroha_data_model norito_pointer_abi_roundtrip` を追加し、`norito.md` にテスト参照を追記済み。 |
