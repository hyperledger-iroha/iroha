<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
id: orchestrator-tuning
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
`docs/source/sorafs/developer/orchestrator_tuning.md` を反映しています。レガシーのドキュメントが廃止されるまで、両方のコピーを同期してください。
:::

# オーケストレーターのロールアウトとチューニング ガイド

このガイドは [設定リファレンス](orchestrator-config.md) と
[マルチソース・ロールアウト・ランブック](multi-source-rollout.md) を前提にしています。
各ロールアウト段階での調整方法、スコアボードのアーティファクトの読み方、
トラフィック拡大前に必要なテレメトリ信号を説明します。CLI・SDK・自動化の
すべてで一貫して適用し、各ノードが同じ決定的な fetch ポリシーに従うように
してください。

## 1. ベースラインのパラメータセット

共有の設定テンプレートから開始し、ロールアウトの進行に合わせて
少数のノブだけを調整します。以下の表は一般的なフェーズの推奨値です。
表にない値は `OrchestratorConfig::default()` と `FetchOptions::default()` の
既定値にフォールバックします。

| フェーズ | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | 備考 |
|---------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|------|
| **Lab / CI** | `3` | `2` | `2` | `2500` | `300` | 厳しめのレイテンシ上限と短いグレース期間で、ノイジーなテレメトリを早期に露出。リトライを抑えて無効なマニフェストを早期に検出。 |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | 本番デフォルトに近づけつつ、探索的な peer に余裕を持たせる。 |
| **Canary** | `6` | `3` | `3` | `5000` | `900` | デフォルトと同等。`telemetry_region` を設定してカナリアトラフィックを分割。 |
| **一般提供 (GA)** | `None`（すべての eligible を使用） | `4` | `4` | `5000` | `900` | 一時的な障害を吸収するために retry/失敗しきい値を上げつつ、監査で決定性を維持。 |

- `scoreboard.weight_scale` は、下流が別の整数分解能を要求しない限り既定の `10_000` を維持します。スケールを上げてもプロバイダー順序は変わらず、クレジット分布が細かくなるだけです。
- フェーズ移行時は JSON バンドルを保存し、`--scoreboard-out` を使用して監査トレイルに正確なパラメータセットを残します。

## 2. スコアボードの衛生管理

スコアボードはマニフェスト要件、プロバイダー広告、テレメトリを統合します。
次へ進む前に:

1. **テレメトリの鮮度を検証。** `--telemetry-json` が参照するスナップショットが
   グレース期間内に取得されているか確認します。`telemetry_grace_secs` を超える
   エントリは `TelemetryStale { last_updated }` で失敗します。これはハードストップとし、
   テレメトリエクスポートを更新してから再試行してください。
2. **eligibility の理由を確認。** `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`
   でアーティファクトを保存します。各エントリには失敗原因を示す `eligibility` ブロックが
   含まれます。能力不一致や期限切れの広告は上書きせず、上流の payload を修正してください。
3. **重みの変化をレビュー。** `normalised_weight` を前回リリースと比較します。10 % 超の
   変動は広告・テレメトリの意図的変更に対応している必要があり、ロールアウトログに記録
   されるべきです。
4. **アーティファクトをアーカイブ。** `scoreboard.persist_path` を設定し、各実行で
   最終スナップショットを保存します。マニフェストとテレメトリバンドルと一緒に
   リリース記録へ添付します。
5. **プロバイダーミックスの証跡を記録。** `scoreboard.json` のメタデータと
   対応する `summary.json` は `provider_count`, `gateway_provider_count`,
   `provider_mix` ラベルを公開し、`direct-only` / `gateway-only` / `mixed` の判定が
   できる必要があります。Gateway のキャプチャは `provider_count=0` と
   `provider_mix="gateway-only"` を示し、混在時は両者が非ゼロであることが必要です。
   `cargo xtask sorafs-adoption-check` がこれらのフィールドを検証（不一致なら失敗）するため、
   `ci/check_sorafs_orchestrator_adoption.sh` か独自スクリプトと併用して
   `adoption_report.json` を生成してください。Torii gateway を利用する場合は、
   `gateway_manifest_id`/`gateway_manifest_cid` をメタデータに残し、アドプションゲートが
   マニフェスト封筒とキャプチャ済みのプロバイダーミックスを相関できるようにします。

詳細なフィールド定義は
`crates/sorafs_car/src/scoreboard.rs` と `sorafs_cli fetch --json-out` が出力する
CLI サマリー構造を参照してください。

## CLI / SDK フラグ リファレンス

`sorafs_cli fetch`（`crates/sorafs_car/src/bin/sorafs_cli.rs`）と
`iroha_cli app sorafs fetch`（`crates/iroha_cli/src/commands/sorafs.rs`）は
同じオーケストレーター設定面を共有しています。ロールアウト証跡の
取得やカノニカル fixtures の再現には次のフラグを使用してください:

共有マルチソース・フラグ一覧（CLI help と docs の同期はこのファイルの編集のみで行う）:

- `--max-peers=<count>` は scoreboard フィルタを通過する eligible プロバイダー数を制限します。未設定なら全 eligible を使用し、意図的に single-source fallback を試す場合のみ `1` を指定します。SDK の `maxPeers`（`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`）に対応。
- `--retry-budget=<count>` は `FetchOptions` のチャンクごとの retry 上限に連動します。推奨値は本ガイドのロールアウト表を使用し、証跡取得の CLI 実行は SDK のデフォルトと一致させてパリティを確保します。
- `--telemetry-region=<label>` は Prometheus の `sorafs_orchestrator_*` 系（および OTLP リレー）に地域/環境ラベルを付け、lab/staging/canary/GA をダッシュボードで分離できます。
- `--telemetry-json=<path>` は scoreboard が参照するスナップショットを注入します。JSON を scoreboard と並べて保存し、監査で再現可能にする（`cargo xtask sorafs-adoption-check --require-telemetry` がどの OTLP ストリームを使ったか検証できるように）。
- `--local-proxy-*`（`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`）は bridge 監視フックを有効化。設定すると、オーケストレーターはローカル Norito/Kaigi proxy 経由で chunk を配信し、ブラウザクライアントや guard cache、Kaigi room が Rust と同じ receipts を受け取ります。
- `--scoreboard-out=<path>`（必要に応じて `--scoreboard-now=<unix_secs>` を併用）で eligibility スナップショットを保存します。保存した JSON は、リリースチケットに記載したテレメトリ/マニフェストのアーティファクトと必ず紐付けてください。
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` は広告メタデータ上での決定的な調整です。リハーサル用途に限定し、本番のダウングレードは governance アーティファクトを経由して全ノードに同じポリシーが適用されるようにします。
- `--provider-metrics-out` / `--chunk-receipts-out` は provider 健康指標と chunk receipts を保持します。ロールアウト証跡提出時に必ず両方を添付してください。

例（公開 fixture を使用）:

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK 側は Rust クライアント（`crates/iroha/src/client.rs`）、JS バインディング
（`javascript/iroha_js/src/sorafs.js`）、Swift SDK
（`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`）の `SorafsGatewayFetchOptions`
から同じ設定を消費します。CLI の既定値と同期させ、運用者が自動化へ
そのまま移植できるようにします。

## 3. Fetch ポリシーのチューニング

`FetchOptions` は retry、並行度、検証を制御します。調整時は:

- **Retries:** `per_chunk_retry_limit` を `4` より上げると復旧時間は伸びますが、
  プロバイダー障害を隠す恐れがあります。上限は `4` を維持し、プロバイダーの
  ローテーションで低品質を露出させるのが推奨です。
- **失敗しきい値:** `provider_failure_threshold` はセッション中にプロバイダーを
  無効化するタイミングを決定します。retry 予算より低いしきい値だと、
  retry を使い切る前に peer が排除されます。
- **並行度:** `global_parallel_limit` は原則 `None` のままにします。特定環境で
  レンジを飽和できない場合のみ設定し、値はプロバイダーのストリーム予算合計
  以下にして starvation を避けます。
- **検証トグル:** `verify_lengths` と `verify_digests` は本番で必須です。混在
  プロバイダーフリートでも決定性を保証します。無効化は隔離された fuzzing
  環境に限定してください。

## 4. トランスポートと匿名性のステージング

`rollout_phase`, `anonymity_policy`, `transport_policy` でプライバシー姿勢を表現します:

- `rollout_phase="snnet-5"` を優先し、匿名性ポリシーは SNNet-5 のマイルストーンに
  追従させます。`anonymity_policy_override` はガバナンスの署名指示がある場合のみ。
- SNNet-4/5/5a/5b/6a/7/8/12/13 が 🈺 の間は `transport_policy="soranet-first"` を維持
  （`roadmap.md` 参照）。`direct-only` は文書化されたダウングレードや
  コンプライアンス訓練のみで使用し、PQ カバレッジのレビュー後に
  `soranet-strict` へ昇格させます（クラシカル relay のみだと即失敗）。
- `write_mode="pq-only"` は、SDK・オーケストレーター・ガバナンスツールすべてが
  PQ を満たせる場合のみ強制。ロールアウト中は `write_mode="allow-downgrade"` を
  維持し、緊急対応が direct ルートに依存できるようにしつつ、テレメトリで
  ダウングレードを可視化します。
- guard 選定と circuit staging は SoraNet ディレクトリに依存します。署名済み
  `relay_directory` スナップショットと `guard_set` cache を保存し、guard の churn を
  合意した保持期間内に抑えます。`sorafs_cli fetch` が記録する cache フィンガープリントは
  ロールアウト証跡の一部です。

## 5. ダウングレードとコンプライアンス フック

オーケストレーターには手動介入なしでポリシーを強制する 2 つのサブシステムがあります:

- **ダウングレード是正**（`downgrade_remediation`）: `handshake_downgrade_total` を監視し、
  `window_secs` 内で `threshold` を超えた場合にローカル proxy を `target_mode`
  （デフォルトは metadata-only）へ強制します。`threshold=3`, `window=300`, `cooldown=900`
  の既定値は、インシデントレビューで別のパターンが示されない限り維持します。
  変更はロールアウトログに記録し、`sorafs_proxy_downgrade_state` をダッシュボードで確認します。
- **コンプライアンスポリシー**（`compliance`）: 司法管轄とマニフェストの carve‑out は
  governance 管理の opt‑out リストで運用します。設定バンドルに ad‑hoc 例外は入れず、
  `governance/compliance/soranet_opt_outs.json` の署名更新を依頼して再デプロイしてください。

両システムとも、最終的な設定バンドルを保存し、リリース証跡に含めて、
監査でダウンシフトのトリガーを追跡できるようにします。

## 6. テレメトリとダッシュボード

ロールアウトを拡大する前に、次の信号が対象環境で稼働していることを確認します:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  canary 完了後にゼロであること。
- `sorafs_orchestrator_retries_total` と
  `sorafs_orchestrator_retry_ratio` — canary 中は 10 % 未満、GA 後は 5 % 未満に安定。
- `sorafs_orchestrator_policy_events_total` — 想定ステージ（`stage` ラベル）を検証し、
  `outcome` でブラウンアウトを記録。
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — PQ relay の供給量をポリシー期待と比較。
- `telemetry::sorafs.fetch.*` のログターゲット — 共有ログ集約基盤に流し、`status=failed`
  の保存検索を用意。

`dashboards/grafana/sorafs_fetch_observability.json` の Grafana ダッシュボードを読み込み
（ポータルの **SoraFS → Fetch Observability** にもエクスポート済み）、
地域/マニフェストセレクタ、プロバイダー retry ヒートマップ、chunk レイテンシ
ヒストグラム、stall カウンタが SRE のレビューと一致するようにします。
`dashboards/alerts/sorafs_fetch_rules.yml` の Alertmanager ルールを配線し、
`scripts/telemetry/test_sorafs_fetch_alerts.sh` で Prometheus 構文を検証します
（helper は `promtool test rules` をローカルまたは Docker で実行）。アラートの
ハンドオフにはスクリプトが出力する routing ブロックが必要で、これを
ロールアウトチケットの証跡として添付します。

### テレメトリ burn-in のワークフロー

ロードマップ **SF-6e** は、マルチソース・オーケストレーターを GA 既定値に
切り替える前に 30 日間の burn-in を要求します。リポジトリのスクリプトで、
ウィンドウ内の各日に再現可能なアーティファクトを取得してください:

1. burn-in 変数を設定して `ci/check_sorafs_orchestrator_adoption.sh` を実行します。
   例:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   helper は `fixtures/sorafs_orchestrator/multi_peer_parity_v1` を再生し、
   `scoreboard.json`, `summary.json`, `provider_metrics.json`, `chunk_receipts.json`,
   `adoption_report.json` を `artifacts/sorafs_orchestrator/<timestamp>/` に書き込み、
   `cargo xtask sorafs-adoption-check` で最小 eligible 数を検証します。
2. burn-in 変数がある場合、`burn_in_note.json` を出力し、ラベル、日次 index、
   マニフェスト ID、テレメトリソース、アーティファクト digest を記録します。
   これをロールアウトログに添付し、30 日間のどの日を満たしたかを明確にします。
3. 更新済み Grafana ボード（`dashboards/grafana/sorafs_fetch_observability.json`）を
   staging/production ワークスペースへインポートし、burn-in ラベルを付与して、
   テスト対象のマニフェスト/リージョンのサンプルが各パネルに表示されることを確認します。
4. `dashboards/alerts/sorafs_fetch_rules.yml` の変更時は、`scripts/telemetry/test_sorafs_fetch_alerts.sh`
   （または `promtool test rules …`）を実行し、burn-in 期間のメトリクスに対する
   アラートルーティングを文書化します。
5. ダッシュボードのスナップショット、アラートテスト出力、`telemetry::sorafs.fetch.*`
   のログテールをオーケストレーターアーティファクトと一緒に保管し、
   ガバナンスがライブシステムからメトリクスを引かずに証跡を再現できるようにします。

## 7. ロールアウト チェックリスト

1. CI で候補設定の scoreboards を再生成し、アーティファクトをバージョン管理下に保存。
2. 各環境（lab, staging, canary, production）で決定的な fixture fetch を実行し、
   `--scoreboard-out` と `--json-out` のアーティファクトをロールアウト記録に添付。
3. on-call エンジニアとテレメトリダッシュボードを確認し、上記メトリクスにライブサンプルがあることを確認。
4. 最終的な設定パス（通常は `iroha_config`）と、広告/コンプライアンスに使用した
   ガバナンスレジストリの git commit を記録。
5. ロールアウトトラッカーを更新し、SDK チームへ新しいデフォルトを通知して
   クライアント統合の整合性を保つ。

このガイドに従うことで、オーケストレーターの展開は決定的かつ監査可能となり、
retry 予算、プロバイダー容量、プライバシー姿勢の調整に向けた明確なフィードバック
ループが得られます。
