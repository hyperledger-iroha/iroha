<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi 正式モデル (TLA+ / Apalache)

このディレクトリには、Sumeragi の安全性と活性性のための限定された正式なモデルが含まれています。

## 範囲

`Sumeragi.tla` はコミット パスをキャプチャします。
- 位相進行 (`Propose`、`Prepare`、`CommitVote`、`NewView`、`Committed`)、
- 投票および定足数のしきい値 (`CommitQuorum`、`ViewQuorum`)、
- NPoS スタイルのコミット ガードの加重ステーク クォーラム (`StakeQuorum`)、
- ヘッダー/ダイジェスト証拠を含む RBC 因果関係 (`Init -> Chunk -> Ready -> Deliver`)、
- 正直な進捗状況に対する GST と弱い公平性の仮定。

`SumeragiFrontierRecovery.tla`は1周あたりの集中した平ハングクラスをキャプチャします。
保留中の隣接フロンティアブロック:
- 定足数以下または定足数に達したコミット投票の証拠、
- 投票キューのバックログとローカル ドレイン、
- 欠落とローカル ペイロードの状態、
- 新しいフロンティアと古いフロンティアの回復の所有権、
- クォーラム - マーカー/ウィンドウ ペーシングの再スケジュール、
- 地域のフロンティアを再固定できる将来のフロンティア/新しい視点の証拠、
- 決定的な GST 後のコミット、再送信、限定されたビューの回転、および
  証拠ゼロの結果が失われる。

どちらのモデルも、ワイヤー フォーマット、ECDSA/署名を意図的に抽象化しています。
検証と完全なネットワークの詳細。

## ファイル- `Sumeragi.tla`: プロトコル モデルとプロパティ。
- `Sumeragi_fast.cfg`: CI に適した小さいパラメーター セット。
- `Sumeragi_deep.cfg`: より大きな応力パラメータ セット。
- `SumeragiFrontierRecovery.tla`: 重点を置いたフロンティア回復モデル。
- `SumeragiFrontierRecovery_fast.cfg`: CI 対応の小さいフロンティア パラメーター セット。
- `SumeragiFrontierRecovery_deep.cfg`: より大きなフロンティア バックログ/ウィンドウ/ビュー境界セット。
- `SumeragiFrontierRecovery_wide.cfg`: 手動のより広いフロンティア境界セット。
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: 予期された失敗による古い所有者の突然変異。
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: 予期された失敗の投票キューの突然変異。

## プロパティ

不変条件:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

時間的性質:
- `EventuallyCommit` (`[] (gst => <> committed)`)、GST 後の公平性エンコード付き
  `Next` で動作可能 (タイムアウト/障害プリエンプション ガードが有効になっています)
  進行中のアクション)。これにより、Apalache 0.52.x でモデルをチェックできるようになります。
  は、チェックされた一時プロパティ内の `WF_` 公平性演算子をサポートしません。

フロンティア回復の不変条件:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`、端末を除外します
  `pending /\ voteBacked /\ ~committed` が回復できない GST 後の状態、
  コミット、再送信、ローテーション、またはバウンドドロップ遷移。フロンティア回復の時間的特性:
- `PostGstVoteBackedFrontierEventuallyResolves`: GST 後、すべての未解決
  投票に裏付けられた保留中のフロンティア状態が最終的にコミット、ペイロードに達する
  リカバリ、クォーラム再送信、将来のフロンティア リーアンカー、または限定されたビュー
  回転。
- `RecoveredPayloadEventuallyAdvances`: 投票で支持された辺境州。
  回復されたペイロードは、コミットせずに永久に保留状態にしておくことはできません。
  再送信、リアアンカー、またはローテーション。
- `QuorumRetransmitEventuallyLeavesPending`: クォーラム再送信が開始されると
  票に裏付けられた辺境州の場合、保留中のラッパーは最終的にはクリアされなければなりません。
- `FutureFrontierEvidenceEventuallyReanchors`: 後のフロンティア/新しい視点の証拠
  保留中のラッパーをクリアするか、フロンティア リーアンカーとして消費される必要があります。

## 仮定マップ

フロンティアモデルは意図的に有限です。これらは実装です
抽象化された表面:|モデルコンセプト |実装面 |
| --- | --- |
| `pending`、`contiguous`、`payloadState` | `PendingBlock` の処理と `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` のローカル ペイロード チェック、および `proposal_handlers.rs` の BlockCreated/フロンティア所有権の具体化。 |
| `commitVotes`、`queuedVotes` | `crates/iroha_core/src/sumeragi/main_loop/tests.rs` の `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` および `reschedule_ignores_quorum_timeout_vote_queue_backlog` によって実行されるコミット投票カウントと投票イングレス ゲーティング。 |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)` ではアクティブ/古いフロンティア オーナーの状態、`maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` では古いオーナーの収量、`drop_superseded_contiguous_frontier_owner_state(...)` ではクリーンアップの置き換え。 |
| `quorumRescheduleArmed`、`quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` での投票に基づくクォーラムのペーシングの再スケジュール。回帰カバレッジには `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` が含まれます。 |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`、`handle_frontier_body_gap_with_topology(...)`、および `stale_frontier_rbc_repair_is_actionable(...)` における正確なフロンティアボディ修復および古い RBC 修復の入院。 |
| `quorumRetransmitted`、`rotated` |クォーラム再送信ターゲット選択、`rebroadcast_pending_block_updates(...)`、および決定論的ビュー変更呼び出し (`reschedule_stale_pending_blocks_with_now(...)`)。 |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)` の将来の新しいビュー/ハイフロンティア クォーラムの証拠。`pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` でカバーされます。 |

## ランニング

リポジトリのルートから:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

ランナーは、モードごとに明示的な Apalache `--length` を設定します。|モード |長さ |使用目的 |
| --- | ---: | --- |
| `fast` | 10 | CI コミットパスのチェック |
| `deep` | 10 |より大規模なコミットパスチェック |
| `frontier-fast` | 10 | CIフロンティアチェック |
| `frontier-deep` | 12 |より大きなフロンティアチェック |
| `frontier-wide` | 14 |手動/毎晩のフロンティアストレスチェック |

`APALACHE_LENGTH=<n>` は、ローカルで探索するときにモードごとのデフォルトをオーバーライドします。
反例または有界証明の拡大。

### 再現可能なローカル設定 (Docker は必要ありません)

このリポジトリで使用される固定されたローカル Apalache ツールチェーンをインストールします。

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

ランナーは次の場所でこのインストールを自動検出します。
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`。
インストール後、`ci/check_sumeragi_formal.sh` は追加の環境変数なしで動作するはずです。

```bash
bash ci/check_sumeragi_formal.sh
```

予想される失敗変異は、意図的に通常の CI の範囲外にあります。彼らはそうすべきです
Apalache では失敗しますが、モデルを変更するときに役立ちます。

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Apalache が `PATH` にない場合は、次のことができます。

- `APALACHE_BIN` を実行可能パスに設定する、または
- Docker フォールバックを使用します (`docker` が使用可能な場合はデフォルトで有効になります)。
  - 画像: `APALACHE_DOCKER_IMAGE` (デフォルト `ghcr.io/apalache-mc/apalache:0.52.2`)
  - Docker デーモンの実行が必要です
  - `APALACHE_ALLOW_DOCKER=0` でフォールバックを無効にします。

例:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## 注意事項- このモデルは、実行可能な Rust モデル テストを補完します (置き換えるものではありません)。
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  そして
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`。
- チェックは、`.cfg` ファイル内の定数値によって制限されます。
- PR CI は、`.github/workflows/pr.yml` でこれらのチェックを実行します。
  `ci/check_sumeragi_formal.sh`。
