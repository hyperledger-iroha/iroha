<!-- Japanese translation of docs/source/query_benchmarks.md -->

---
lang: ja
direction: ltr
source: docs/source/query_benchmarks.md
status: complete
translator: manual
---

# クエリベンチマークスイート

本ページでは Iroha のクエリ向けマイクロベンチマークの構成と読み取り方を説明します。ネットワークを切り離し、イテレーターベースの実行とクライアント側ビルダーのオーバーヘッドに焦点を当てています。

## ベンチ配置

- コアクエリ用ベンチ: `crates/iroha_core/benches/queries.rs`
- クライアント経路ビルダー用ベンチ（モック実行器）: `crates/iroha_core/benches/queries_client.rs`

どちらもワークスペースの lint 要件に従い、`fn main` を持つ Criterion ベンチです。

## 手法

- ベンチはメモリ上に `State` と `World` を直接構築し（ネットワークなし）、`ValidQuery::execute` を使って項目のイテレータを取得します。
- 副作用が最適化で消えないよう `count()` やコレクション化＋ソートで結果を消費し、`criterion::black_box` でデッドコード除去を防ぎます。
- クライアント経路オーバーヘッドでは、`MockExec` が `QueryExecutor` を実装し、ネットワーク I/O を伴わずに `QueryBuilder`/`QueryBuilderExt` へバッチを供給することで、ビルダーコストと型ダウンキャスト経路を分離します。

## データセット

- アカウント: `build_state_with_accounts(n)` が単一ドメインと `n` アカウント（権限のみの所有者）を生成。
- アセット: `build_state_with_assets(n_accounts, assets_per_account)` が上記に加えて各アカウントに 1〜2 件のアセットを追加。
- ドメイン: `build_state_with_domains(n)` が独立したドメインを `n` 件生成。
- アセット定義: `build_state_with_asset_definitions(n)` がドメイン・所有アカウント・`n` 件のアセット定義を生成。

## 測定対象

コアベンチ（`queries.rs`）:
- FindAccounts
  - イテレートして件数をカウント（1k, 10k）
  - ID でソート（10k）
  - フル結果ベクタに対するページネーションのシミュレーション（ページ幅 100）
- FindAssets
  - イテレートして件数をカウント（約 10k）
  - 特定アカウントでフィルタ
  - 量基準でフィルタ
- FindDomains
  - イテレートして件数をカウント（5k）
  - ID でソート（10k）
- FindAssetDefinitions
  - イテレートして件数をカウント（10k）

クライアント経路ベンチ（`queries_client.rs`）:
- `QueryBuilder` + `execute_all()`（モック `QueryExecutor`）
  - 総数 1k、`fetch_size = 100`
  - 総数 10k、`fetch_size = 500`

いずれもネットワークの影響を除外し、クライアント SDK が辿る実際の経路に近づけています。

## 実行方法

ビルド:

```
cargo build -p iroha_core --benches
```

個別実行:

```
# iroha_core の全クエリベンチ
cargo bench -p iroha_core --bench queries

# クライアント経路ベンチ（モック実行器）
cargo bench -p iroha_core --bench queries_client
```

注: Criterion は結果を `target/criterion` 以下に配置します。安定した比較のため、CPU ガバナーやバックグラウンド処理のノイズを抑え、複数回の実行で分散を確認してください。

## 結果の読み方

- イテレータ処理コストは走査する件数に比例します。ベンチは線形な振る舞いを可視化します。ソートは `O(n log n)` のコストを追加し、キーサイズや比較対象に敏感です。
- イテレータ側でのフィルタはサーバ側フィルタの上限を示します。実際には前段でのプリフィルタリングを優先し、下流負荷を減らすべきです。
- フルベクタ上でのページネーションシミュレーションはクライアント側チャンク分割の近似です。実運用ではサーバ側のページネーション（`fetch_size`）に頼ることで、全件読み込みを避けます。
- クライアント経路ベンチは `QueryOutputBatchBoxTuple` の構築と型ダウンキャストに伴うオーバーヘッドを切り出して評価します。

## 次のステップ

- サーバ側ハンドラ用ベンチ（ホットな述語、ソートキー）を追加。
- サーバ側射影が復活した際にはセレクション（projection）ベンチも追加。
- 合成ステートビルダーを用いたトリガー／ブロック向けベンチを拡充。
