<!-- Japanese translation of docs/source/merge_ledger.md -->

---
lang: ja
direction: ltr
source: docs/source/merge_ledger.md
status: complete
translator: manual
---

# マージレジャー設計 — レーン確定性とグローバル集約

本ノートはマイルストン 5 に向けたマージレジャー設計を確定します。非空ブロック方針、レーン横断の QC マージセマンティクス、およびレーン実行をグローバル状態コミットへ結び付ける最終確定ワークフローについて説明します。

設計は `nexus.md` で記載された Nexus アーキテクチャを前提とします。「レーンブロック」「レーン QC」「マージヒント」「マージレジャー」といった用語は同文書の定義を継承します。本ノートではランタイム・ストレージ・WSV 層が遵守すべき振る舞い規則と実装ガイダンスに焦点を当てます。

## 1. 非空ブロック方針

**ルール（MUST）:** レーン提案者は、ブロックに少なくとも 1 件の実行済みトランザクションフラグメント、時間トリガー、または決定的アーティファクト更新（例: データ可用性アーティファクトのロールアップ）が含まれる場合にのみブロックを発行します。空ブロックは禁止です。

**含意:**

- スロットの維持: 決定的コミットウィンドウを満たすトランザクションがない場合、レーンはブロックを発行せず次スロットへ遷移します。マージレジャーは該当レーンの直前ティップを保持します。
- トリガーのバッチ化: 状態遷移を伴わないバックグラウンドトリガー（例: 不変条件を再確認する cron）は空ブロックと見なされ、スキップするか他の処理と束ねてからブロック生成する必要があります。
- テレメトリ: `pipeline_detached_merged` 等のメトリクスはスキップしたスロットを明示的に扱い、オペレーターが「仕事なし」と「パイプライン停滞」を区別可能にします。
- リプレイ: ブロックストレージは合成された空プレースホルダを挿入しません。ブロックが出なければ、Kura のリプレイループは連続するスロットで同じ親ハッシュを観測するだけです。

**正規チェック:** ブロック提案と検証の際、`ValidBlock::commit` は関連する `StateBlock` に少なくとも 1 つのコミット済みオーバーレイ（デルタ、アーティファクト、トリガー）があることを検証します。これは `StateBlock::is_empty` ガード（ノーオペ書き込みを抑止）と一致します。署名要求前に強制することで、委員会が空ペイロードへ投票する状況を防ぎます。

## 2. レーン横断 QC マージセマンティクス

各レーンブロック `B_i` が委員会で可決されると以下が生成されます:

- `lane_state_root_i`: ブロックで触れた DS ごとの状態ルートに対する Poseidon2-SMT コミットメント。
- `merge_hint_root_i`: マージレジャー向けロール候補（ドメインタグ `"iroha:merge:candidate:v1\0"`）。
- `lane_qc_i`: 実行投票のプレイメージ（ブロックハッシュ、`parent_state_root`、`post_state_root`、height/view/epoch、chain_id、モードタグ）に対するレーン委員会の集約署名。

マージノードは全レーン `i ∈ [0, K)` の最新ティップ `{(B_i, lane_qc_i, merge_hint_root_i)}` を収集します。

**マージエントリ（MUST）:**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```

- `lane_tips[i]` はマージエントリが封じるレーン `i` のブロックハッシュです。前回エントリ以降にブロックが無ければ値を引き継ぎます。
- `merge_hint_root[i]` は対応するレーンブロックの `merge_hint_root`。ブロックが繰り返される場合も同値を再利用します。
- `global_state_root` は `ReduceMergeHints(merge_hint_root[0..K-1])` に等しく、ドメインタグ `"iroha:merge:reduce:v1\0"` を用いた Poseidon2 の fold です。縮約は決定的でピア間で同じ値に再構築されなければなりません。
- `merge_qc` はマージ委員会によるエントリ直列化への BFT クォーラム署名。

**マージ QC 署名ペイロード（MUST）:**

マージ委員会は次の決定的ダイジェストに署名します:

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` はレーン tip から導出するマージ委員会のビュー（エントリに含まれるレーンヘッダの `view_change_index` の最大値）。
- `chain_id` は設定済みチェーン識別子文字列（UTF-8 バイト列）。
- ペイロードは上記のフィールド順で Norito エンコードされます。

このダイジェストは `merge_qc.message_digest` に格納され、BLS 署名が検証する対象となります。

**マージ QC の構築（MUST）:**

- マージ委員会ロスターは現在の commit-topology バリデータ集合。
- 必要クォーラム = `commit_quorum_from_len(roster_len)`。
- `merge_qc.signers_bitmap` は commit-topology の順序で署名者インデックスを LSB-first で符号化。
- `merge_qc.aggregate_signature` は上記ダイジェストに対する BLS-normal 集約署名。

**検証手順（MUST）:**

1. 各 `lane_qc_i` を `lane_tips[i]` に対して検証し、ブロックヘッダーが一致する `merge_hint_root_i` を含むことを確認。
2. `lane_qc_i` が `Invalid` または未実行ブロックを指さないかチェック。前述の非空方針によりヘッダーには状態オーバーレイが含まれます。
3. `ReduceMergeHints` を再計算し、`global_state_root` と比較。
4. マージ QC のダイジェストを再計算し、署名者ビットマップ、クォーラム閾値、集約署名を commit-topology ロスターに照らして検証。

**可観測性:** マージノードは `merge_entry_lane_repeats_total{i}` を Prometheus に発行し、スロットをスキップしたレーンを可視化します。

## 3. 確定ワークフロー

### 3.1 レーンレベルの最終化

1. トランザクションはレーンごとに決定的スロットへスケジュールされます。
2. エグゼキュータが `StateBlock` にオーバーレイを適用し、デルタとアーティファクトを生成。
3. 検証後、レーン委員会が実行投票のプレイメージ（ブロックハッシュ、`parent_state_root`、`post_state_root`、height/view/epoch、chain_id、モードタグ）に署名。タプル `(block_hash, lane_qc_i, merge_hint_root_i)` はレーン最終化済みとみなされます。
4. ライトクライアントは DS 限定証明のためにレーンティップを最終確定として扱えますが、後でマージレジャーと照合するため `merge_hint_root` を記録する必要があります。

### 3.2 マージレジャーの最終化

1. マージ委員会が最新レーンティップを収集し、各 `lane_qc_i` を検証して `MergeLedgerEntry` を構築。
2. 決定的縮約を検証後、委員会がエントリへ署名（`merge_qc`）。
3. ノードはエントリをマージレジャーログに追加し、レーンブロック参照と共に永続化。
4. `global_state_root` が該当エポック／スロットの正規世界状態コミットメントとなります。フルノードは WSV チェックポイントメタデータを更新し、決定的リプレイで同値を再構築できなければなりません。

### 3.3 WSV とストレージ統合

- `State::commit_merge_entry` はレーン状態ルートおよび `global_state_root` を記録し、レーン実行をグローバル整合ハッシュと結び付けます。
- Kura は `MergeLedgerEntry` をレーンブロックアーティファクトと並列に保存し、リプレイ時にレーンレベルとグローバルの最終化系列を再構築可能にします。
- レーンがスロットをスキップした場合でもストレージは前回ティップを保持するだけで、新規ブロックが出るまでマージエントリは更新されません。
- API（Torii、テレメトリ）はレーンティップと最新マージエントリの両方を公開し、オペレーターやクライアントがレーン・グローバル双方の状態を突き合わせることができます。

## 4. 実装ノート

- `crates/iroha_core/src/state.rs`: `State::commit_merge_entry` が縮約を検証し、レーン／グローバルメタデータを世界状態へ連携。クエリや観測者がマージヒントと正規グローバルハッシュへアクセス可能。
- `crates/iroha_core/src/block.rs`: 検証フェーズで空ブロックを `BlockValidationError::EmptyBlock`
  として拒否し、署名要求より前に非空方針を強制したうえでマージレジャーへ
  参照を渡します。
- 決定的縮約ヘルパーはマージサービスに実装済み。Poseidon2 fold による `reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) が正規縮約を提供し、ハードウェアアクセラレータ導入は今後の課題ながら
  スカラーフォールバックで決定性を担保。
- テレメトリ統合: レーン単位のマージ繰り返し回数と `global_state_root` ゲージを公開し、マージエントリの停滞をダッシュボードで検知。
- クロスコンポーネントテスト: リプレイ後の縮約で記録済み `global_state_root` が再生成されることを確認するゴールデンテストを追加。

## 5. 互換性と移行

- 既存の単一レーン環境では `K=1` とみなし、マージレジャーは依然としてエントリを記録しますが `ReduceMergeHints` は恒等写像に退化します。非空方針は従来の挙動（空ブロックなし）と一致します。
- レーンを追加する場合、マージをサポートしない古いノードは委員会パスから外す必要があります。そうしないと `MergeLedgerEntry` を検証できません。
- 永続化フォーマットは前方互換。将来のエントリは Norito ヘッダー以降に補助フィールドを追加しても構いませんが、ABI バージョンが更新されるまで未知フィールドは拒否されます。

この設計でマイルストン 5 の「Merge-Ledger Design Notes」成果物が完了します。実装担当者は上記の実装メモを踏まえ、ランタイムとストレージ層を計画通りに更新してください。
