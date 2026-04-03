<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Kura / WSV セキュリティおよびパフォーマンス監査 (2026-02-19)

## 範囲

この監査では以下が対象となりました。

- Kura 永続性と予算パス: `crates/iroha_core/src/kura.rs`
- 運用 WSV/状態コミット/クエリ パス: `crates/iroha_core/src/state.rs`
- IVM WSV モック ホスト サーフェス (テスト/開発スコープ): `crates/ivm/src/mock_wsv.rs`

範囲外: 無関係なクレートとフルシステムのベンチマークの再実行。

## リスクの概要

- クリティカル: 0
- 高: 4
- 中: 6
- 低: 2

## 調査結果 (重大度順)

### 高

1. **Kura ライターが I/O 障害でパニックを起こす (ノード可用性リスク)**
- コンポーネント: クラ
- タイプ: セキュリティ (DoS)、信頼性
- 詳細: ライター ループは、回復可能なエラーを返すのではなく、追加/インデックス/fsync エラーでパニックを起こすため、一時的なディスク障害によりノード プロセスが終了する可能性があります。
- 証拠:
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- 影響: リモート負荷 + ローカルディスク圧力により、クラッシュ/再起動ループが発生する可能性があります。2. **Kura エビクションは、`block_store` ミューテックスの下で完全なデータ/インデックスの書き換えを実行します**
- コンポーネント: クラ
- タイプ: パフォーマンス、可用性
- 詳細: `evict_block_bodies` は、`block_store` ロックを保持したまま、一時ファイル経由で `blocks.data` と `blocks.index` を書き換えます。
- 証拠:
  - ロック取得：`crates/iroha_core/src/kura.rs:834`
  - 完全な書き換えループ: `crates/iroha_core/src/kura.rs:921`、`crates/iroha_core/src/kura.rs:942`
  - アトミック置換/同期: `crates/iroha_core/src/kura.rs:956`、`crates/iroha_core/src/kura.rs:960`
- 影響: エビクション イベントにより、大規模な履歴に対する書き込み/読み取りが長期間停止する可能性があります。

3. **ステートコミットは、負荷の高いコミット作業全体にわたって粗い `view_lock` を保持します**
- コンポーネント: プロダクション WSV
- タイプ: パフォーマンス、可用性
- 詳細: ブロック コミットは、トランザクション、ブロック ハッシュ、ワールド ステートのコミット中に排他的な `view_lock` を保持し、重いブロックの下でリーダーの飢餓を引き起こします。
- 証拠:
  - ロック保持の開始: `crates/iroha_core/src/state.rs:17456`
  - ロック内部の作業: `crates/iroha_core/src/state.rs:17466`、`crates/iroha_core/src/state.rs:17476`、`crates/iroha_core/src/state.rs:17483`
- 影響: 大量のコミットが継続すると、クエリ/コンセンサスの応答性が低下する可能性があります。4. **IVM JSON 管理エイリアスにより、呼び出し元のチェックなしで特権付きの変更が可能になります (テスト/開発ホスト)**
- コンポーネント: IVM WSV モック ホスト
- タイプ: セキュリティ (テスト/開発環境での権限昇格)
- 詳細: JSON エイリアス ハンドラーは、呼び出し元スコープのアクセス許可トークンを必要としないロール/アクセス許可/ピア変更メソッドに直接ルーティングされます。
- 証拠:
  - 管理者エイリアス: `crates/ivm/src/mock_wsv.rs:4274`、`crates/ivm/src/mock_wsv.rs:4371`、`crates/ivm/src/mock_wsv.rs:4448`
  - ゲートなしミューテーター: `crates/ivm/src/mock_wsv.rs:1035`、`crates/ivm/src/mock_wsv.rs:1055`、`crates/ivm/src/mock_wsv.rs:855`
  - ファイルドキュメント内のスコープメモ (テスト/開発意図): `crates/ivm/src/mock_wsv.rs:295`
- 影響: テスト コントラクト/ツールが統合ハーネスのセキュリティ前提を自己強化し、無効にする可能性があります。

### 中

5. **Kura バジェット チェックは、エンキューごとに保留中のブロックを再エンコードします (書き込みごとに O(n))**
- コンポーネント: クラ
- タイプ: パフォーマンス
- 詳細: 各エンキューは、保留中のブロックを反復し、正規のワイヤ サイズ パスを介してそれぞれをシリアル化することにより、保留中のキューのバイトを再計算します。
- 証拠:
  - キュースキャン: `crates/iroha_core/src/kura.rs:2509`
  - ブロックごとのエンコード パス: `crates/iroha_core/src/kura.rs:2194`、`crates/iroha_core/src/kura.rs:2525`
  - エンキュー時の予算チェックで呼び出される: `crates/iroha_core/src/kura.rs:2580`、`crates/iroha_core/src/kura.rs:2050`
- 影響: バックログ時の書き込みスループットの低下。6. **Kura バジェット チェックは、エンキューごとにブロック ストア メタデータの読み取りを繰り返し実行します**
- コンポーネント: クラ
- タイプ: パフォーマンス
- 詳細: 各チェックでは、`block_store` をロックしながら永続インデックス数とファイル長を読み取ります。
- 証拠:
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- 影響: ホット エンキュー パスで回避可能な I/O/ロック オーバーヘッド。

7. **Kura のエビクションは、エンキュー バジェット パスからインラインでトリガーされます**
- コンポーネント: クラ
- タイプ: パフォーマンス、可用性
- 詳細: エンキュー パスは、新しいブロックを受け入れる前にエビクションを同期的に呼び出すことができます。
- 証拠:
  - エンキュー呼び出しチェーン: `crates/iroha_core/src/kura.rs:2050`
  - インラインエビクション呼び出し: `crates/iroha_core/src/kura.rs:2603`
- 影響: 予算に近い場合、トランザクション/ブロック取り込みのテール レイテンシーが急増します。

8. **`State::view` は競合下で粗いロックを取得せずに返される可能性があります**
- コンポーネント: プロダクション WSV
- タイプ: 一貫性/パフォーマンスのトレードオフ
- 詳細: 書き込みロック競合の場合、`try_read` フォールバックは、設計によりコース ガードなしでビューを返します。
- 証拠:
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- 影響: 生存性が向上しましたが、呼び出し元は競合下で弱いコンポーネント間のアトミック性を許容する必要があります。9. **`apply_without_execution` は DA カーソルの進行にハード `expect` を使用します**
- コンポーネント: プロダクション WSV
- タイプ: セキュリティ (パニックオンインバリアントブレイクによる DoS)、信頼性
- 詳細: DA カーソル進行の不変条件が失敗すると、コミットされたブロック適用パスがパニックになります。
- 証拠:
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
- 影響: 潜在的な検証/インデックス作成のバグがノードを破壊する障害になる可能性があります。

10. **IVM TLV パブリッシュ システムコールに、割り当て前の明示的なエンベロープ サイズ制限がありません (テスト/開発ホスト)**
- コンポーネント: IVM WSV モック ホスト
- タイプ: セキュリティ (メモリ DoS)、パフォーマンス
- 詳細: ヘッダー長を読み取り、このパスでホストレベルの上限なしで完全な TLV ペイロードを割り当て/コピーします。
- 証拠:
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- 影響: 悪意のあるテスト ペイロードにより、大規模な割り当てが強制される可能性があります。

### 低い

11. **Kura 通知チャネルは無制限です (`std::sync::mpsc::channel`)**
- コンポーネント: クラ
- タイプ: パフォーマンス/メモリの健全性
- 詳細: 通知チャネルは、持続的なプロデューサーの圧力中に冗長なウェイク イベントを蓄積する可能性があります。
- 証拠:
  - `crates/iroha_core/src/kura.rs:552`
- 影響: イベント サイズあたりのメモリ増加リスクは低いですが、回避可能です。12. **パイプラインのサイドカー キューは、ライターが排出されるまでメモリ内で制限されません**
- コンポーネント: クラ
- タイプ: パフォーマンス/メモリの健全性
- 詳細: サイドカー キュー `push_back` には明示的な上限/バックプレッシャーがありません。
- 証拠:
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- 影響: ライターの遅延が長期化すると、メモリが増加する可能性があります。

## 既存のテスト範囲とギャップ

### 蔵

- 既存の補償範囲:
  - ストレージ バジェットの動作: `store_block_rejects_when_budget_exceeded`、`store_block_rejects_when_pending_blocks_exceed_budget`、`store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`、`crates/iroha_core/src/kura.rs:6949`、`crates/iroha_core/src/kura.rs:6984`)
  - エビクションの正確さとリハイドレーション: `evict_block_bodies_does_not_truncate_unpersisted`、`evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`、`crates/iroha_core/src/kura.rs:8126`)
- ギャップ:
  - パニックを起こさずに追加/インデックス/fsync 障害を処理するためのフォールトインジェクションのカバレッジがない
  - 大きな保留キューとエンキューの予算チェック コストに対するパフォーマンス回帰テストはありません
  - ロック競合下での長い歴史を持つエビクション レイテンシ テストはありません

### プロダクション WSV

- 既存の補償範囲:
  - 競合フォールバック動作: `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - 階層型バックエンドに関するロック順序の安全性: `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- ギャップ:
  - 大量のワールドコミットの下で許容可能な最大コミット保持時間を主張する定量的競合テストはありません
  - DA カーソル進行不変条件が予期せず壊れた場合にパニックを起こさずに処理するための回帰テストはありません

### IVM WSV モックホスト- 既存の補償範囲:
  - 許可 JSON パーサー セマンティクスとピア解析 (`crates/ivm/src/mock_wsv.rs:5234`、`crates/ivm/src/mock_wsv.rs:5332`)
  - TLV デコードおよび JSON デコードに関する syscall スモーク テスト (`crates/ivm/src/mock_wsv.rs:5962`、`crates/ivm/src/mock_wsv.rs:6078`)
- ギャップ:
  - 無許可の管理者エイリアスの拒否テストはありません
  - `INPUT_PUBLISH_TLV` ではオーバーサイズの TLV エンベロープ拒否テストはありません
  - チェックポイント/復元クローンのコストに関するベンチマーク/ガードレール テストはありません

## 優先順位の高い修復計画

### フェーズ 1 (高衝撃硬化)

1. Kura ライター `panic!` ブランチを回復可能なエラー伝播 + 健全性低下シグナルで置き換えます。
- 対象ファイル：`crates/iroha_core/src/kura.rs`
- 受け入れ:
  - 挿入された追加/インデックス/fsync エラーがパニックにならない
  - エラーはテレメトリ/ロギングを通じて明らかになり、ライターは制御可能な状態を維持します

2. IVM モックホスト TLV パブリッシュおよび JSON エンベロープ パスの境界付きエンベロープ チェックを追加します。
- 対象ファイル：`crates/ivm/src/mock_wsv.rs`
- 受け入れ:
  - サイズ超過のペイロードは、割り当ての多い処理の前に拒否されます。
  - 新しいテストは、TLV と JSON の両方のサイズが大きすぎるケースをカバーします

3. JSON 管理者エイリアス (または厳密なテスト専用機能フラグと文書の明確な背後にあるゲート エイリアス) に対する明示的な呼び出し元のアクセス許可チェックを強制します。
- 対象ファイル：`crates/ivm/src/mock_wsv.rs`
- 受け入れ:
  - 権限のない呼び出し元は、エイリアスを通じてロール/権限/ピアの状態を変更できません

### フェーズ 2 (ホットパスのパフォーマンス)4. Kura の予算会計を段階的に行う。
- エンキューごとの完全な保留キューの再計算を、エンキュー/永続化/ドロップ時に更新される維持されたカウンターに置き換えます。
- 受け入れ:
  - 保留中のバイト計算のためのエンキュー コストは O(1) に近い
  - 回帰ベンチマークは、保留の深さが増加するにつれて安定したレイテンシを示します

5. エビクションロックの保持時間を短縮します。
- オプション: セグメント化された圧縮、ロック解放境界を使用したチャンク コピー、または境界付きフォアグラウンド ブロックを使用したバックグラウンド メンテナンス モード。
- 受け入れ:
  - 大きな履歴のエビクションのレイテンシーが減少し、フォアグラウンド操作の応答性が維持されます

6. 可能な場合は、粗い `view_lock` クリティカル セクションを短くします。
- コミット フェーズの分割またはステージングされたデルタのスナップショットを評価して、排他的ホールド ウィンドウを最小限に抑えます。
- 受け入れ:
  - 競合メトリクスは、重いブロックコミット下での 99p ホールド時間の短縮を示しています

### フェーズ 3 (ガードレールの運用)

7. Kura ライターとサイドカー キューのバックプレッシャー/キャップに制限付き/合体ウェイク シグナリングを導入します。
8. 以下のテレメトリ ダッシュボードを展開します。
- `view_lock` 待機/保留分布
- エビクション期間と実行ごとの再利用バイト数
- バジェットチェックのエンキューレイテンシ

## 推奨されるテストの追加1. `kura_writer_io_failures_do_not_panic` (ユニット、フォールト挿入)
2. `kura_budget_check_scales_with_pending_depth` (パフォーマンスの低下)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (統合/パフォーマンス)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (競合回帰)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (回復力)
6. `mock_wsv_admin_alias_requires_permissions` (セキュリティの後退)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (DoS ガード)
8. `mock_wsv_checkpoint_restore_cost_regression` (パフォーマンスベンチマーク)

## 範囲と信頼性に関する注意事項

- `crates/iroha_core/src/kura.rs` および `crates/iroha_core/src/state.rs` の検出結果は、本番パスの検出結果です。
- `crates/ivm/src/mock_wsv.rs` の検出結果は、ファイル レベルのドキュメントごとに、明示的にテスト/開発ホストを対象としています。
- この監査自体では、ABI バージョン管理の変更は必要ありません。