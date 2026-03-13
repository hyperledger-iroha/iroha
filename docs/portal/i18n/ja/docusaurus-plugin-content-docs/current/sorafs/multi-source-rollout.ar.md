---
lang: ar
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aad86e56a181c5ca2f75b52118f3f6259006e4340dc0809f5403f27d0d1c23b0
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: multi-source-rollout
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
このページは `docs/source/sorafs/runbooks/multi_source_rollout.md` を反映しています。レガシーのドキュメントセットが退役するまで、両方のコピーを同期したままにしてください。
:::

## 目的

このランブックは、SRE と当番エンジニアが以下の重要なワークフローを進めるためのものです。

1. マルチソース・オーケストレーターを制御された波でロールアウトする。
2. 既存セッションを不安定化させずに、問題のあるプロバイダーをブラックリスト化または優先度を下げる。

SF-6 で提供されたオーケストレーション・スタックが既に展開されていることを前提とします (`sorafs_orchestrator`, gateway の chunk-range API, テレメトリエクスポータ)。

> **参照:** [オーケストレーター運用ランブック](./orchestrator-ops.md) は、各実行の手順 (scoreboard の取得、段階的ロールアウトの切り替え、ロールバック) を詳述しています。ライブ変更時は両方の参照を併用してください。

## 1. 事前検証

1. **ガバナンス入力を確認する。**
   - すべての候補プロバイダーは、レンジ能力 payload とストリーム予算を含む `ProviderAdvertV1` エンベロープを公開する必要があります。`/v2/sorafs/providers` で検証し、期待される capability フィールドと照合してください。
   - レイテンシ/失敗率を供給するテレメトリ・スナップショットは、各カナリア実行前に 15 分未満である必要があります。
2. **設定をステージングする。**
   - オーケストレーターの JSON 設定をレイヤードな `iroha_config` ツリーに保存します:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     ロールアウト固有の制限 (`max_providers`, リトライ予算) を JSON に反映します。同じファイルを staging/production に投入して差分を最小化してください。
3. **正規フィクスチャを実行する。**
   - マニフェスト/トークンの環境変数を設定し、決定的 fetch を実行します:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     環境変数には、マニフェスト payload の digest (hex) と、カナリアに参加する各プロバイダーの base64 エンコード済み stream トークンを含める必要があります。
   - `artifacts/canary.scoreboard.json` を前回のリリースと差分比較してください。新規の非適格プロバイダーや 10% 超の weight 変化はレビューが必要です。
4. **テレメトリの配線を確認する。**
   - `docs/examples/sorafs_fetch_dashboard.json` の Grafana エクスポートを開きます。`sorafs_orchestrator_*` メトリクスが staging に出ていることを確認してから進めてください。

## 2. 緊急プロバイダー・ブラックリスト

プロバイダーが破損チャンクを返したり、継続的にタイムアウトしたり、コンプライアンスチェックに失敗した場合は、この手順に従ってください。

1. **証跡を収集する。**
   - 最新の fetch サマリー (`--json-out` の出力) をエクスポートします。失敗したチャンクのインデックス、プロバイダーのエイリアス、digest の不一致を記録してください。
   - `telemetry::sorafs.fetch.*` ターゲットの関連ログ抜粋を保存します。
2. **即時の override を適用する。**
   - オーケストレーターに配布するテレメトリ・スナップショットでプロバイダーを penalized に設定します (`penalty=true` または `token_health` を `0` にクランプ)。次回の scoreboard 生成で自動的に除外されます。
   - アドホックなスモークテストには、`sorafs_cli fetch` に `--deny-provider gw-alpha` を渡して、テレメトリ伝播を待たずに失敗経路を検証します。
   - 更新したテレメトリ/設定バンドルを影響環境に再デプロイします (staging → canary → production)。変更内容はインシデントログに記録してください。
3. **override を検証する。**
   - 正規フィクスチャの fetch を再実行し、scoreboard が `policy_denied` の理由でプロバイダーを非適格にしていることを確認します。
   - `sorafs_orchestrator_provider_failures_total` を確認し、拒否したプロバイダーのカウンタが増え続けていないことを確認します。
4. **長期ブロックをエスカレーションする。**
   - 24 時間超のブロックが必要な場合、advert をローテーション/停止するためのガバナンスチケットを起票します。投票が通るまでは deny リストを維持し、テレメトリ・スナップショットを更新して scoreboard に再入力されないようにしてください。
5. **ロールバック手順。**
   - プロバイダーを復帰させる場合は deny リストから削除し、再デプロイして新しい scoreboard スナップショットを取得します。変更内容をインシデントのポストモーテムに添付します。

## 3. 段階的ロールアウト計画

| フェーズ | スコープ | 必須シグナル | Go/No-Go 基準 |
|---------|---------|--------------|---------------|
| **Lab** | 専用の統合クラスタ | CLI による手動 fetch (fixture payload) | 全チャンク成功、プロバイダー失敗カウンタ 0、リトライ率 < 5%。 |
| **Staging** | フルの control-plane staging | Grafana ダッシュボード接続済み; アラートルールは warning-only | `sorafs_orchestrator_active_fetches` が各テスト後に 0 に戻る; `warn/critical` アラート発火なし。 |
| **Canary** | 本番トラフィックの ≤10% | Pager はミュートだがテレメトリはリアルタイム監視 | リトライ率 < 10%、プロバイダー失敗は既知のノイズピーアに限定、レイテンシヒストグラムが staging baseline ±20%。 |
| **General Availability** | 100% ロールアウト | Pager ルール有効 | 24 時間 `NoHealthyProviders` エラー 0、リトライ率安定、ダッシュボードの SLA パネルがグリーン。 |

各フェーズで行うこと:

1. 想定する `max_providers` とリトライ予算をオーケストレーター JSON に反映します。
2. `sorafs_cli fetch` もしくは SDK の統合テストスイートを、正規フィクスチャと環境に対応するマニフェストで実行します。
3. scoreboard と summary のアーティファクトを取得し、リリース記録に添付します。
4. 次のフェーズに進む前に、当番エンジニアとテレメトリダッシュボードをレビューします。

## 4. 可観測性とインシデント連携

- **メトリクス:** Alertmanager が `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` と `sorafs_orchestrator_retries_total` を監視していることを確認してください。急増は通常、プロバイダーが負荷下で劣化している兆候です。
- **ログ:** `telemetry::sorafs.fetch.*` ターゲットを共有ログアグリゲータへ送ります。`event=complete status=failed` の保存検索を作成してトリアージを迅速化します。
- **Scoreboards:** 各 scoreboard アーティファクトを長期保存します。JSON はコンプライアンスレビューや段階的ロールバックの証跡にもなります。
- **ダッシュボード:** 正規の Grafana ボード (`docs/examples/sorafs_fetch_dashboard.json`) を本番フォルダへ複製し、`docs/examples/sorafs_fetch_alerts.yaml` のアラートルールを適用します。

## 5. 連絡とドキュメント

- deny/boost の変更はすべて、タイムスタンプ、オペレーター、理由、関連インシデントとともに運用チェンジログへ記録します。
- プロバイダーの weight やリトライ予算が変わった場合は SDK チームに通知し、クライアント側の期待値を合わせます。
- GA 完了後は `status.md` を更新し、ロールアウトの要約とともにこのランブック参照をリリースノートへアーカイブします。
