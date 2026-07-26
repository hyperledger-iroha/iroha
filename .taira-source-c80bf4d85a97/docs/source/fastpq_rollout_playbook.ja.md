---
lang: ja
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FASTPQ ロールアウト ハンドブック (ステージ 7-3)

このプレイブックは、Stage7-3 ロードマップ要件、つまりすべてのフリートのアップグレードを実装します。
FASTPQ GPU 実行を有効にする場合は、再現可能なベンチマーク マニフェストを添付する必要があります。
Grafana の証拠と文書化されたロールバック ドリルのペア。補完する
`docs/source/fastpq_plan.md` (ターゲット/アーキテクチャ) および
`docs/source/fastpq_migration_guide.md` (ノードレベルのアップグレード手順) (フォーカスによる)
オペレータ向けのロールアウト チェックリストに記載されています。

## 範囲と役割

- **リリース エンジニアリング / SRE:** 独自のベンチマーク キャプチャ、マニフェスト署名、
  ロールアウトの承認前にダッシュボードをエクスポートします。
- **Ops Guild:** 段階的なロールアウトを実行し、ロールバック リハーサルを記録し、保存します
  `artifacts/fastpq_rollouts/<timestamp>/` の下のアーティファクト バンドル。
- **ガバナンス/コンプライアンス:** あらゆる変更に証拠が伴うことを検証します
  フリートに対して FASTPQ のデフォルトが切り替わる前にリクエストを実行します。

## 証拠バンドルの要件

すべてのロールアウト送信には、次のアーティファクトが含まれている必要があります。すべてのファイルを添付する
リリース/アップグレード チケットに追加し、バンドルを保管しておいてください
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`。|アーティファクト |目的 |作り方 |
|----------|-----------|-----|
| `fastpq_bench_manifest.json` |正規の 20000 行ワークロードが `<1 s` LDE 上限を下回っており、ラップされたすべてのベンチマークのハッシュを記録していることを証明します。 Capture Metal/CUDA を実行し、ラップして次を実行します。`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
|ラップされたベンチマーク (`fastpq_metal_bench_*.json`、`fastpq_cuda_bench_*.json`) |ホストのメタデータ、行使用量の証拠、ゼロフィル ホットスポット、ポセイドン マイクロベンチの概要、ダッシュボード/アラートで使用されるカーネル統計をキャプチャします。 `fastpq_metal_bench` / `fastpq_cuda_bench` を実行し、生の JSON をラップします。`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`CUDA キャプチャに対して繰り返します (ポイント関連する監視/スクレイピング ファイルの `--row-usage` および `--poseidon-metrics`)。ヘルパーはフィルタリングされた `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` サンプルを埋め込むため、WP2-E.6 の証拠は Metal と CUDA で同一になります。スタンドアロンの Poseidon マイクロベンチの概要 (ラップされた入力または生の入力がサポートされている) が必要な場合は、`scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` を使用します。 |
|  |  | **ステージ 7 ラベル要件:** 結果の `metadata.labels` セクションに `device_class` と `gpu_kind` の両方が含まれていない限り、`wrap_benchmark.py` は失敗するようになりました。自動検出でそれらを推測できない場合 (たとえば、切り離された CI ノードをラップする場合)、`--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete` などの明示的なオーバーライドを渡します。 |
|  |  | **加速テレメトリ:** ラッパーはデフォルトで `cargo xtask acceleration-state --format json` もキャプチャし、ラップされたベンチマークの隣に `<bundle>.accel.json` および `<bundle>.accel.prom` を書き込みます (`--accel-*` フラグまたは `--skip-acceleration-state` でオーバーライドします)。キャプチャ マトリックスはこれらのファイルを使用して、フリート ダッシュボード用の `acceleration_matrix.{json,md}` を構築します。 |
| Grafana エクスポート |導入テレメトリとロールアウト ウィンドウのアラート アノテーションを証明します。| `fastpq-acceleration` ダッシュボードをエクスポートします。`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`エクスポートする前に、ボードにロールアウトの開始/停止時間の注釈を付けます。リリース パイプラインは、`scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (`GRAFANA_TOKEN` 経由で提供されるトークン) 経由でこれを自動的に実行できます。 |
|アラートのスナップショット |ロールアウトを保護したアラート ルールをキャプチャします。| `dashboards/alerts/fastpq_acceleration_rules.yml` (および `tests/` フィクスチャ) をバンドルにコピーして、レビュー担当者が `promtool test rules …` を再実行できるようにします。 |
|ロールバックドリルログ |オペレーターが強制的な CPU フォールバックとテレメトリ確認応答をリハーサルしたことを示します。 [ロールバック ドリル](#rollback-drills) の手順を使用し、コンソール ログ (`rollback_drill.log`) と結果の Prometheus スクレイピング (`metrics_rollback.prom`) を保存します。 || `row_usage/fastpq_row_usage_<date>.json` | TF-5 が CI およびダッシュボードで追跡する ExecWitness FASTPQ 行割り当てを記録します。 Torii から新しい監視をダウンロードし、`iroha_cli audit witness --decode exec.witness` 経由でデコードし (オプションで `--fastpq-parameter fastpq-lane-balanced` を追加して、予期されるパラメーター セットをアサートします。FASTPQ バッチはデフォルトで出力します)、`row_usage` JSON を `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/` にコピーします。レビュー担当者がファイル名をロールアウト チケットと関連付けることができるようにファイル名にタイムスタンプを付けたままにし、`python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (または `make check-fastpq-rollout`) を実行して、Stage7-3 ゲートが証拠を添付する前にすべてのバッチがセレクター カウントをアドバタイズし、`transfer_ratio = transfer_rows / total_rows` が不変であることを検証します。 |

> **ヒント:** `artifacts/fastpq_rollouts/README.md` には推奨される名前が記載されています
> スキーム (`<stamp>/<fleet>/<lane>`) と必要な証拠ファイル。の
> `<stamp>` フォルダーは、アーティファクトを並べ替え可能に保つために `YYYYMMDDThhmmZ` をエンコードする必要があります
> チケットを相談せずに。

## 証拠作成チェックリスト1. **GPU ベンチマークをキャプチャします。**
   - 正規のワークロード (20000 論理行、32768 個のパディング行) を実行します。
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`。
   - `--row-usage <decoded witness>` を使用して結果を `scripts/fastpq/wrap_benchmark.py` でラップし、バンドルに GPU テレメトリとともにガジェットの証拠が含まれるようにします。 `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` を渡すと、アクセラレータがターゲットを超えた場合、または Poseidon キュー/プロファイル テレメトリが欠落している場合にラッパーが高速で失敗し、分離された署名が生成されます。
   - マニフェストに両方の GPU ファミリが含まれるように、CUDA ホスト上で繰り返します。
   - `benchmarks.metal_dispatch_queue` またはを削除しないでください**
     `benchmarks.zero_fill_hotspots` はラップされた JSON からブロックします。 CIゲート
     (`ci/check_fastpq_rollout.sh`) はこれらのフィールドを読み取り、キューに入れると失敗するようになりました。
     ヘッドルームが 1 スロットを下回るか、LDE ホットスポットが `mean_ms > を報告すると低下する
     0.40ms`、Stage7 テレメトリ ガードが自動的に適用されます。
2. **マニフェストを生成します。** `cargo xtask fastpq-bench-manifest …` を次のように使用します。
   表に示します。 `fastpq_bench_manifest.json` をロールアウト バンドルに保存します。
3. **Grafana をエクスポートします。**
   - ロールアウト ウィンドウで `FASTPQ Acceleration Overview` ボードに注釈を付けます。
     関連する Grafana パネル ID にリンクします。
   - Grafana API (上記のコマンド) を介してダッシュボード JSON をエクスポートし、次の内容を含めます
     `annotations` セクションを参照すると、レビュー担当者は採用曲線を
     段階的な展開。
4. **スナップショット アラート。** 使用されている正確なアラート ルール (`dashboards/alerts/…`) をコピーします。
   バンドルへのロールアウトによって。 Prometheus ルールがオーバーライドされた場合は、次の内容を含めます
   オーバーライドの差分。
5. **Prometheus/OTEL スクレイピング** それぞれから `fastpq_execution_mode_total{device_class="<matrix>"}` をキャプチャします
   ホスト（ステージ前後）＋OTELカウンター
   `fastpq.execution_mode_resolutions_total` とペアになっている
   `telemetry::fastpq.execution_mode` ログ エントリ。これらの工芸品はそれを証明しています
   GPU の採用は安定しており、強制的な CPU フォールバックは引き続きテレメトリを生成します。
6. **行使用量テレメトリをアーカイブします。** ExecWitness をデコードした後、
   ロールアウトして、結果の JSON をバンドルの `row_usage/` の下にドロップします。 CI
   ヘルパー (`ci/check_fastpq_row_usage.sh`) は、これらのスナップショットを
   正規のベースライン、および `ci/check_fastpq_rollout.sh` にはすべてのものが必要になりました。
   TF-5 証拠を添付しておくために、少なくとも 1 つの `row_usage` ファイルを同梱するバンドル
   リリースチケットへ。

## 段階的なロールアウト フロー

すべてのフリートに対して 3 つの決定論的フェーズを使用します。出口後にのみ前進
各フェーズの基準が満たされており、証拠バンドルに文書化されています。|フェーズ |範囲 |終了基準 |添付ファイル |
|------|------|------|-----------|
|パイロット (P1) |リージョンごとに 1 つのコントロール プレーン + 1 つのデータ プレーン ノード | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` 48 時間で 90% 以上、Alertmanager インシデントはゼロ、ロールバック ドリルは合格。 |両方のホストからのバンドル (ベンチ JSON、パイロット アノテーション付きの Grafana エクスポート、ロールバック ログ)。 |
|ランプ(P2) |バリデーターの 50% 以上に加え、クラスターごとに少なくとも 1 つのアーカイブ レーン | GPU の実行は 5 日間持続し、10 分を超えるダウングレード スパイクは 1 回以下で、Prometheus カウンターは 60 秒以内にフォールバック アラートを示すことを証明しました。 |ランプ注釈、Prometheus スクレープ差分、Alertmanager スクリーンショット/ログを示す Grafana エクスポートを更新しました。 |
|デフォルト (P3) |残りのノード。 FASTPQ は `iroha_config` でデフォルトとしてマークされています。最終的な導入曲線を参照する署名付きベンチ マニフェスト + Grafana エクスポート、および設定の切り替えを示す文書化されたロールバック ドリル。 |最終マニフェスト、Grafana JSON、ロールバック ログ、構成変更レビューへのチケット参照。 |

ロールアウト チケットの各プロモーション ステップを文書化し、
`grafana_fastpq_acceleration.json` 注釈により、レビュー担当者は
証拠のあるタイムライン。

## ロールバック ドリル

すべてのロールアウト ステージには、ロールバック リハーサルが含まれている必要があります。

1. クラスターごとに 1 つのノードを選択し、現在のメトリックを記録します。
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2.いずれかの設定ノブを使用して、CPU モードを 10 分間強制します。
   (`zk.fastpq.execution_mode = "cpu"`) または環境オーバーライド:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. ダウングレードログを確認する
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) とスクレイピング
   Prometheus エンドポイントを再度実行して、カウンターの増分を表示します。
4. GPU モードを復元し、`telemetry::fastpq.execution_mode` がレポートすることを確認します。
   `resolved="metal"` (または非金属レーンの場合は `resolved="cuda"/"opencl"`)、
   Prometheus スクレープに CPU サンプルと GPU サンプルの両方が含まれていることを確認します。
   `fastpq_execution_mode_total{backend=…}` に経過時間を記録します。
   検出/クリーンアップ。
5. シェルのトランスクリプト、メトリクス、およびオペレータの確認応答を次のように保存します。
   ロールアウト バンドルの `rollback_drill.log` および `metrics_rollback.prom`。これら
   ファイルには完全なダウングレードと復元のサイクルを示す必要があります。
   ログに GPU が不足している場合、`ci/check_fastpq_rollout.sh` が失敗するようになりました。
   リカバリラインまたはメトリクススナップショットでは、CPU または GPU カウンターのいずれかが省略されます。

これらのログは、すべてのクラスターが正常に機能を低下させることができ、SRE チームが
GPU ドライバーまたはカーネルが後退した場合に決定的にフォールバックする方法を知っています。

## 混合モードのフォールバック証拠 (WP2-E.6)

ホストが GPU FFT/LDE を必要とするが、CPU ポセイドン ハッシュが必要な場合 (Stage7 <900ms ごと)
要件）、標準のロールバック ログと一緒に次のアーティファクトをバンドルします。1. **構成差分** を設定するホストローカルオーバーライドをチェックイン (またはアタッチ) します。
   退出中 `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`)
   `zk.fastpq.execution_mode` は手付かずです。パッチに名前を付けます
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`。
2. **ポセイドンカウンタースクレイピング**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   キャプチャでは、`path="cpu_forced"` がロックステップで増加していることを示す必要があります。
   そのデバイスクラスの GPU FFT/LDE カウンター。元に戻してから 2 回目のスクレイピングを行う
   GPU モードに戻ると、レビュー担当者は `path="gpu"` 行の再開を確認できるようになります。

   結果のファイルを `wrap_benchmark.py --poseidon-metrics …` に渡すと、ラップされたベンチマークが `poseidon_metrics` セクション内に同じカウンターを記録します。これにより、Metal と CUDA のロールアウトが同一のワークフローに保たれ、別個のスクレイプ ファイルを開かなくてもフォールバック証拠が監査可能になります。
3. **ログの抜粋** を証明する `telemetry::fastpq.poseidon` エントリをコピーします。
   リゾルバが CPU (`cpu_forced`) に切り替えられました
   `poseidon_fallback.log`、アラートマネージャーのタイムラインを確認できるようにタイムスタンプを保持します。
   設定変更と相関があります。

CI は現在、キュー/ゼロフィル チェックを強制します。混合モードのゲートが到着すると、
`ci/check_fastpq_rollout.sh` は、次の内容を含むバンドルも要求します。
`poseidon_fallback.patch` には、対応する `metrics_poseidon.prom` スナップショットが同梱されています。
このワークフローに従って、WP2-E.6 フォールバック ポリシーを監査可能に保ち、
デフォルトオンのロールアウト中に使用されたのと同じ証拠コレクター。

## レポート作成と自動化

- `artifacts/fastpq_rollouts/<stamp>/` ディレクトリ全体を
  チケットをリリースし、ロールアウトが終了したら `status.md` から参照します。
- `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` を実行します (経由)
  `promtool`) CI 内で、ロールアウトにバンドルされているアラート バンドルが確実に維持されるようにする
  コンパイルします。
- `ci/check_fastpq_rollout.sh` (または
  `make check-fastpq-rollout`) を指定し、次の場合に `FASTPQ_ROLLOUT_BUNDLE=<path>` を渡します。
  単一のロールアウトをターゲットにしたい。 CI は同じスクリプトを経由して呼び出します。
  `.github/workflows/fastpq-rollout.yml` なので、不足しているアーティファクトは、
  リリースチケットはクローズされる可能性があります。リリース パイプラインは検証済みのバンドルをアーカイブできます
  署名されたマニフェストと一緒に渡します
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>`から
  `scripts/run_release_pipeline.py`;ヘルパーが再実行する
  `ci/check_fastpq_rollout.sh` (`--skip-fastpq-rollout-check` が設定されていない場合) および
  ディレクトリ ツリーを `artifacts/releases/<version>/fastpq_rollouts/…` にコピーします。
  このゲートの一部として、スクリプトは Stage7 のキュー深度とゼロフィルを強制します。
  `benchmarks.metal_dispatch_queue` を読み取ることで予算を取得し、
  各 `metal` ベンチ JSON のうちの `benchmarks.zero_fill_hotspots`。

このプレイブックに従うことで、決定的な採用を実証し、
ロールアウトごとに単一の証拠バンドルを作成し、ロールバック訓練も並行して監査し続ける
署名されたベンチマークマニフェスト。