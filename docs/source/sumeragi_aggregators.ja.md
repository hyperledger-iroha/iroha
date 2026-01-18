<!-- Japanese translation of docs/source/sumeragi_aggregators.md -->

---
lang: ja
direction: ltr
source: docs/source/sumeragi_aggregators.md
status: complete
translator: manual
---

# Sumeragi 集約ノードルーティング

## 概要

本ノートでは Phase 3 のフェアネス更新以降、Sumeragi が採用している決定論的コレクタ（「アグリゲータ」）ルーティング戦略をまとめます。すべてのバリデータは同じブロック高さとビューに対して同一のコレクタ順序を計算し、即興的なランダム性への依存を排除します。指定コレクタが全て応答しない場合も、再試行が上限付きの「ゴシップ」ファンアウトにエスカレーションされることが保証されます。

## 決定論的選出

- 新しい `sumeragi::collectors` モジュールは `deterministic_collectors(topology, mode, k, seed, height, view)` を公開し、任意の `(height, view)` ペアに対して再現可能な `Vec<PeerId>` を返します。
- パーミッションドモードではエポックの PRF/VRF シードに基づく PRF でコレクタを選出し、カノニカルロスターから `(height, view)` ごとに決定論的な順序を導出してリーダーを除外します。PRF シードがない場合は連続するテイル集合にフォールバックします。
- NPoS モードでは従来通りエポックごとの PRF を用いますが、ヘルパーが計算を一元化することで呼び出し側すべてが同一順序を取得します。シードは `EpochManager` が提供するエポック乱数から導出されます。
- `CollectorPlan` は並び順の消費状況とゴシップフォールバックが発火したかどうかを記録します。`collect_aggregator_ms`, `sumeragi_redundant_sends_*`, `sumeragi_gossip_fallback_total` といったテレメトリはフォールバック頻度と冗長ファンアウトの所要時間を可視化します。

## フェアネス目標

1. **再現性:** 同一のバリデータトポロジ、コンセンサスモード、PRF シード、`(height, view)` の組み合わせであれば、すべてのピアが同じ一次／二次コレクタに到達しなければならない。ヘルパーがプロキシテイルやオブザーバといったトポロジ固有の要素を吸収し、コンポーネントやテスト間で移植性のある順序を提供します。
2. **ローテーション:** PRF 選出が両モードで高さ/ビューごとに一次コレクタを交代させ、特定のオブザーバが常時集約を担当し続けることを防ぎます。連続テイルへのフォールバックは PRF シードがない場合のみです。
3. **可観測性:** テレメトリはコレクタ割り当てを報告し続け、ゴシップフォールバックが作動した際には警告を発します。これによりオペレーターは挙動不審なコレクタを検出できます。

## リトライとゴシップバックオフ

- バリデータは進行中の `VotingBlock` と並行して `CollectorPlan` を保持し、どのコレクタに接触したか、およびゴシップにエスカレーションしたかを記録します。
- 冗長送信（`r`）はプランを順に進める形で決定論的に適用されます。追加コレクタが存在しない、またはすべての試行が応答なしで終了した場合、プランはゴシップフォールバックが発火したことをマークします。`collect_aggregator_gossip_total`（`/v1/sumeragi/phases`）は Prometheus カウンタと同値で、オペレーターが反復的なエスカレーションを監視できます。
- ゴシップフォールバックは署名済みブロックと prevote をトポロジ内の全ピア（自ノードを除く）へ送信します。指定コレクタが全滅した場合でもライブネスを保証し、通常ケースでは対象を限定したまま、従来の「全てにブロードキャスト」フェイルセーフを再現します。
- フォールバックはブロックごとに 1 回のみ発火し、ネットワークストームを避けます。ロック済み commit certificate による提案ドロップは `block_created_dropped_by_lock_total` をインクリメントし、ヘッダー検証失敗は `block_created_hint_mismatch_total` や `block_created_proposal_mismatch_total` を上げます。`/v1/sumeragi/status` には最新の Highest/Locked QC ハッシュが含まれ、ダッシュボードで特定ブロックとドロップ急増の相関を取れます。

## 実装概要

- 新たな公開モジュール `sumeragi::collectors` に `CollectorPlan` と `deterministic_collectors` を配置し、クレートレベル／統合テストの双方がコンセンサスアクターを起動せずにフェアネスを検証できるようにしました。
- `VotingBlock` は `CollectorPlan` を保持し、プランの消耗状況を確認しつつゴシップを 1 度だけ発火させるヘルパーを提供します。
- `Sumeragi` は `collector_targets_for_round()` でプランを構築し、`gossip_vote_to_all_collectors()` でフォールバックブロードキャストを実装します。
- ユニットテストと統合テストは PRF の決定性、フォールバック選出、バックオフ状態遷移を検証します。

## レビュー署名

- Reviewed-by: Consensus WG
- Reviewed-by: Platform Reliability WG
