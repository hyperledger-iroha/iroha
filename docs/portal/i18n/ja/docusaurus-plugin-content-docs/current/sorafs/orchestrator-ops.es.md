---
lang: es
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 47c14fd3d07096458be822db4ed68956adacb5d579b16fdca9c29fde7fee0c8e
source_last_modified: "2025-11-07T10:33:21.924371+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正規ソース
このページは `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md` を反映しています。レガシーの Sphinx ドキュメントセットが完全に移行されるまで、両方のコピーを同期したままにしてください。
:::

このランブックは、マルチソースの fetch オーケストレーターの準備、展開、運用を SRE が進めるための手順をまとめたものです。段階的な有効化やピアのブラックリストを含む、プロダクション向けの手順で開発ガイドを補完します。

> **参照:** [マルチソース・ロールアウト・ランブック](./multi-source-rollout.md) は、フリート全体のロールアウト波と緊急時のプロバイダー拒否に焦点を当てています。ガバナンス / ステージングの調整にはそちらを参照し、本書は日々のオーケストレーター運用に使用してください。

## 1. 事前チェックリスト

1. **プロバイダー入力の収集**
   - 対象フリート向けの最新プロバイダー広告 (`ProviderAdvertV1`) とテレメトリのスナップショット。
   - テスト対象のマニフェストから生成したペイロード計画 (`plan.json`)。
2. **決定的なスコアボードの生成**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - `artifacts/scoreboard.json` に本番プロバイダーがすべて `eligible` として掲載されていることを確認します。
   - スコアボードと並べてサマリー JSON をアーカイブしてください。監査では変更申請の認証時にチャンク再試行カウンタが参照されます。
3. **フィクスチャでのドライラン** — `docs/examples/sorafs_ci_sample/` の公開フィクスチャに対して同じコマンドを実行し、オーケストレーターのバイナリが期待されるバージョンと一致することを確認してから本番ペイロードに触れてください。

## 2. 段階的ロールアウト手順

1. **カナリア段階 (≤2 プロバイダー)**
   - スコアボードを再生成し、`--max-peers=2` で実行してオーケストレーターを小さなサブセットに制限します。
   - 監視:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - マニフェストの完全な fetch に対する再試行率が 1% 未満で、どのプロバイダーも失敗を蓄積していないことを確認してから進めます。
2. **ランプ段階 (50% プロバイダー)**
   - `--max-peers` を増やし、最新のテレメトリ・スナップショットで再実行します。
   - 各実行で `--provider-metrics-out` と `--chunk-receipts-out` を保存します。アーティファクトは ≥7 日保持してください。
3. **フルロールアウト**
   - `--max-peers` を削除するか、適格数の上限に設定します。
   - クライアントのデプロイでオーケストレーター・モードを有効化し、永続化したスコアボードと設定 JSON を構成管理システム経由で配布します。
   - ダッシュボードを更新し、`sorafs_orchestrator_fetch_duration_ms` の p95/p99 と地域別の再試行ヒストグラムを表示します。

## 3. ピアのブラックリストとブースト

ガバナンスの更新を待たずに問題のあるプロバイダーを切り分けるため、CLI のスコアリングポリシー上書きを使用します。

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` は指定したエイリアスを現在のセッションの対象から除外します。
- `--boost-provider=<alias>=<weight>` はプロバイダーのスケジューラ重みを引き上げます。値は正規化されたスコアボード重みに加算され、ローカル実行にのみ適用されます。
- インシデントチケットに上書きを記録し、JSON 出力を添付して、根本原因が解消された後に担当チームが状態を突き合わせられるようにします。

恒久的な変更の場合は、元のテレメトリを修正して違反者を penalised としてマークするか、更新済みのストリーム予算で広告を更新してから CLI の上書きを解除します。

## 4. 障害切り分け

fetch が失敗した場合:

1. 再実行前に次のアーティファクトを取得します。
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. `session.summary.json` を確認し、人間が読めるエラー文字列を確認します。
   - `no providers were supplied` → プロバイダーのパスと広告を確認します。
   - `retry budget exhausted ...` → `--retry-budget` を増やすか、不安定なピアを除外します。
   - `no compatible providers available ...` → 問題のあるプロバイダーのレンジ能力メタデータを監査します。
3. `sorafs_orchestrator_provider_failures_total` とプロバイダー名を突き合わせ、メトリクスが急増した場合はフォローアップチケットを作成します。
4. `--scoreboard-json` と取得したテレメトリでオフライン再現し、失敗を決定的に再現します。

## 5. ロールバック

オーケストレーターのロールアウトを戻すには:

1. `--max-peers=1` を設定した構成を配布してマルチソースのスケジューリングを実質的に無効化するか、クライアントを従来の単一ソース fetch パスに戻します。
2. `--boost-provider` の上書きをすべて削除し、スコアボードの重みを中立に戻します。
3. 少なくとも 1 日はオーケストレーターのメトリクス収集を継続し、残存する fetch がないことを確認します。

アーティファクトの厳格な取得と段階的ロールアウトを維持することで、異種のプロバイダーフリートに対してもマルチソース・オーケストレーターを安全に運用でき、観測性と監査要件を満たせます。
