---
lang: ja
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T10:01:27.059307+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ 転送ガジェットの設計

# 概要

現在の FASTPQ プランナーは、`TransferAsset` 命令に含まれるすべての基本操作を記録します。これは、各転送がバランス演算、ハッシュ ラウンド、および SMT 更新の費用を個別に支払うことを意味します。転送ごとのトレース行を減らすために、ホストが正規の状態遷移を実行し続けている間、最小限の算術/コミットメント チェックのみを検証する専用のガジェットを導入します。

- **範囲**: 既存の Kotodama/IVM `TransferAsset` システムコール サーフェス経由で発行される単一の転送と小さなバッチ。
- **目標**: ルックアップ テーブルを共有し、転送ごとの演算をコンパクトな制約ブロックにまとめることで、大量転送の FFT/LDE カラム フットプリントを削減します。

# 建築

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## トランスクリプトの形式

ホストは、syscall 呼び出しごとに `TransferTranscript` を発行します。

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` は、リプレイ保護のためにトランスクリプトをトランザクション エントリポイント ハッシュに結び付けます。
- `authority_digest` は、ソートされた署名者/クォーラム データに対するホストのハッシュです。ガジェットは等価性をチェックしますが、署名の検証はやり直しません。具体的には、ホスト Norito は `AccountId` (正規のマルチシグ コントローラが既に埋め込まれている) をエンコードし、Blake2b-256 で `b"iroha:fastpq:v1:authority|" || encoded_account` をハッシュし、結果の `Hash` を保存します。
- `poseidon_preimage_digest` = ポセイドン(アカウントから || アカウントへ || 資産 || 金額 || バッチハッシュ);ガジェットがホストと同じダイジェストを再計算するようにします。プリイメージ バイトは、共有 Poseidon2 ヘルパーに渡す前に、裸の Norito エンコーディングを使用して `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` として構築されます。このダイジェストはシングルデルタ転写物に存在し、マルチデルタバッチでは省略されます。

すべてのフィールドは Norito 経由でシリアル化されるため、既存の決定論が保持されることが保証されます。
`from_path` と `to_path` は両方とも、
`TransferMerkleProofV1` スキーマ: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`。
将来のバージョンでは、証明者がバージョン タグを適用しながらスキーマを拡張できます
デコードする前に。 `TransitionBatch` メタデータには、Norito でエンコードされたトランスクリプトが埋め込まれています
証明者が証人を解読できるように、`transfer_transcripts` キーの下のベクトル
帯域外クエリを実行する必要はありません。パブリック入力 (`dsid`、`slot`、ルート、
`perm_root`、`tx_set_hash`) は `FastpqTransitionBatch.public_inputs` に含まれ、
エントリのハッシュ/トランスクリプト数の簿記用のメタデータを残します。ホスト配管まで
着地すると、証明者はキーとバランスのペアから証明を総合的に導き出すため、行が
トランスクリプトでオプションのフィールドが省略されている場合でも、常に決定論的な SMT パスを含めます。

## ガジェットのレイアウト

1. **バランス演算ブロック**
   - 入力: `from_balance_before`、`amount`、`to_balance_before`。
   - チェック:
     - `from_balance_before >= amount` (共有 RNS 分解を備えた範囲ガジェット)。
     - `from_balance_after = from_balance_before - amount`。
     - `to_balance_after = to_balance_before + amount`。
   - カスタム ゲートにパックされるため、3 つの式すべてが 1 つの行グループを消費します。2. **ポセイドンコミットメントブロック**
   - 他のガジェットですでに使用されている共有ポセイドン ルックアップ テーブルを使用して `poseidon_preimage_digest` を再計算します。トレースには転送ごとのポセイドン弾はありません。

3. **マークル パス ブロック**
   - 既存の Kaigi SMT ガジェットを「ペア更新」モードで拡張します。 2 つのリーフ (送信者、受信者) は兄弟ハッシュの同じ列を共有し、重複する行を減らします。

4. **典拠ダイジェストチェック**
   - ホスト提供のダイジェストと監視値の間の単純な等価制約。署名は専用ガジェットに残ります。

5. **バッチ ループ**
   - プログラムは、`transfer_asset` ビルダーのループの前に `transfer_v1_batch_begin()` を呼び出し、その後で `transfer_v1_batch_end()` を呼び出します。スコープがアクティブである間、ホストは各転送をバッファリングし、単一の `TransferAssetBatch` として再生し、バッチごとに 1 回 Poseidon/SMT コンテキストを再利用します。デルタを追加するたびに、算術演算と 2 つのリーフ チェックのみが追加されます。トランスクリプト デコーダはマルチデルタ バッチを受け入れ、それらを `TransferGadgetInput::deltas` として表示するようになりました。これにより、プランナーは Norito を再読み取りせずに証人をフォールドできるようになります。すでに Norito ペイロードを持っているコントラクト (CLI/SDK など) は、`transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` を呼び出すことでスコープを完全にスキップでき、1 つの syscall で完全にエンコードされたバッチをホストに渡します。

# 主催者と証明者の変更|レイヤー |変更点 |
|------|-----------|
| `ivm::syscalls` | `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) を追加して、プログラムが中間 ISI を発行せずに複数の `transfer_v1` システムコールを一括処理できるようにするとともに、`transfer_v1_batch_apply` (`0x2B`)事前にエンコードされたバッチの場合。 |
| `ivm::host` とテスト |コア/デフォルト ホストはスコープがアクティブな間 `transfer_v1` をバッチ追加として扱い、`SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` を表面化し、モック WSV ホストはコミット前にエントリをバッファするため、回帰テストで決定的なバランスを確認できます。更新情報。【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713]【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` |状態遷移後に `TransferTranscript` を発行し、`StateBlock::capture_exec_witness` 中に明示的な `public_inputs` を使用して `FastpqTransitionBatch` レコードを構築し、FASTPQ 証明者レーンを実行して、Torii/CLI ツールと Stage6 バックエンドの両方が正規データを受信できるようにします。 `TransitionBatch` 入力。 `TransferAssetBatch` は、連続転送を単一のトランスクリプトにグループ化し、マルチデルタ バッチのポセイドン ダイジェストを省略して、ガジェットがエントリ間で決定的に反復できるようにします。 |
| `fastpq_prover` | `gadgets::transfer` は、プランナー (`crates/fastpq_prover/src/gadgets/transfer.rs`) のマルチデルタ トランスクリプト (バランス算術 + ポセイドン ダイジェスト) と表面構造化証人 (プレースホルダー ペアの SMT BLOB を含む) を検証するようになりました。 `trace::build_trace` はバッチ メタデータからこれらのトランスクリプトをデコードし、`transfer_transcripts` ペイロードが欠落している転送バッチを拒否し、検証された証人を `Trace::transfer_witnesses` に添付し、`TracePolynomialData::transfer_plan()` はプランナーがガジェット (`crates/fastpq_prover/src/trace.rs`) を消費するまで、集約されたプランを存続させます。行数回帰ハーネスは `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) 経由で出荷されるようになり、最大 65536 のパディング行までのシナリオをカバーしますが、ペアの SMT 配線は TF-3 バッチ ヘルパー マイルストーンの背後に残ります (スワップが完了するまで、プレースホルダーによりトレース レイアウトが安定します)。 |
| Kotodama | `transfer_batch((from,to,asset,amount), …)` ヘルパーを `transfer_v1_batch_begin`、連続する `transfer_asset` 呼び出し、および `transfer_v1_batch_end` に下げます。各タプル引数は、`(AccountId, AccountId, AssetDefinitionId, int)` の形式に従う必要があります。単一転送では既存のビルダーが保持されます。 |

Kotodama の使用例:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` は、個々の `Transfer::asset_numeric` 呼び出しと同じ権限チェックと算術チェックを実行しますが、単一の `TransferTranscript` 内にすべての差分を記録します。マルチデルタ転写物では、デルタごとのコミットメントがフォローアップされるまで、ポセイドン ダイジェストが省略されます。 Kotodama ビルダーは、begin/end syscall を自動的に発行するようになり、コントラクトは Norito ペイロードを手動でエンコードしなくてもバッチ転送を展開できるようになりました。

## 行数回帰ハーネス

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) は、設定可能なセレクター数を使用して FASTPQ 遷移バッチを合成し、その結果の `row_usage` 概要 (`total_rows`、セレクターごとの数、比率) をパディング長/log₂ とともにレポートします。次のコマンドを使用して、65536 行の天井のベンチマークを取得します。

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```発行された JSON は、`iroha_cli audit witness` がデフォルトで発行するようになった FASTPQ バッチ アーティファクトをミラーリングします (抑制するには `--no-fastpq-batches` を渡します)。そのため、プランナーの変更を検証するときに、`scripts/fastpq/check_row_usage.py` と CI ゲートは合成実行を以前のスナップショットと比較できます。

# 展開計画

1. **TF-1 (トランスクリプト プラミング)**: ✅ `StateTransaction::record_transfer_transcripts` は、`TransferAsset`/バッチごとに Norito トランスクリプトを出力し、`sumeragi::witness::record_fastpq_transcript` はそれらをグローバル監視内に保存し、`StateBlock::capture_exec_witness` をビルドします`fastpq_batches` とオペレーターおよび証明者レーン用の明示的な `public_inputs` (よりスリムなレーンが必要な場合は `--no-fastpq-batches` を使用してください)出力).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (ガジェット実装)**: ✅ `gadgets::transfer` は、マルチデルタ トランスクリプト (バランス算術 + ポセイドン ダイジェスト) を検証し、ホストが省略した場合にペアの SMT プルーフを合成し、`TransferGadgetPlan` 経由で構造化証人を公開し、`trace::build_trace` がそれらの証人をスレッドします。プルーフから SMT 列を設定中に `Trace::transfer_witnesses`。 `fastpq_row_bench` は 65536 行の回帰ハーネスをキャプチャするため、プランナは Norito を再生せずに行の使用状況を追跡します。ペイロード.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (バッチ ヘルパー)**: ホスト レベルのシーケンシャル アプリケーションとガジェット ループを含む、バッチ syscall + Kotodama ビルダーを有効にします。
4. **TF-4 (テレメトリとドキュメント)**: `fastpq_plan.md`、`fastpq_migration_guide.md`、およびダッシュボード スキーマを更新して、他のガジェットに対する転送行の割り当てを明らかにします。

# 未解決の質問

- **ドメイン制限**: 現在の FFT プランナーは、2¹⁴ 行を超えるトレースに対してパニックを起こします。 TF-2 では、ドメイン サイズを増やすか、ベンチマーク目標の縮小を文書化する必要があります。
- **複数アセット バッチ**: 初期ガジェットはデルタごとに同じアセット ID を想定します。異種バッチが必要な場合は、アセット間のリプレイを防ぐために、毎回 Poseidon Witness にアセットが含まれていることを確認する必要があります。
- **権限ダイジェストの再利用**: 長期的には、システムコールごとの署名者リストの再計算を避けるために、他の許可された操作に同じダイジェストを再利用できます。


この文書は設計上の決定を追跡します。マイルストーンが達成されたときに、ロードマップ エントリとの同期を保ちます。