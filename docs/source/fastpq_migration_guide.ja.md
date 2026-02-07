---
lang: ja
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ 本番移行ガイド

この Runbook では、Stage6 実稼働 FASTPQ 証明者を検証する方法について説明します。
決定論的なプレースホルダー バックエンドは、この移行計画の一部として削除されました。
これは、`docs/source/fastpq_plan.md` の段階的計画を補完するものであり、すでに追跡していることを前提としています。
`status.md` のワークスペースのステータス。

## 対象読者と範囲
- バリデーターオペレーターは、ステージング環境またはメインネット環境で実稼働証明器をロールアウトします。
- リリース エンジニアは、運用バックエンドに同梱されるバイナリまたはコンテナを作成します。
- SRE/可観測性チームが新しいテレメトリ信号を配線し、アラートを送信します。

範囲外: Kotodama コントラクト オーサリングおよび IVM ABI の変更 (詳細については、`docs/source/nexus.md` を参照)
実行モデル）。

## 機能マトリックス
|パス |有効にする貨物機能 |結果 |いつ使用するか |
| ---- | ----------------------- | ------ | ----------- |
|実稼働証明者 (デフォルト) | _なし_ | FFT/LDE プランナーと DEEP-FRI パイプラインを備えた Stage6 FASTPQ バックエンド。【crates/fastpq_prover/src/backend.rs:1144】 |すべての実稼働バイナリのデフォルト。 |
|オプションの GPU アクセラレーション | `fastpq_prover/fastpq-gpu` |自動 CPU フォールバックで CUDA/Metal カーネルを有効にします。【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 |サポートされているアクセラレータを備えたホスト。 |

## ビルド手順
1. **CPU のみのビルド**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   運用バックエンドはデフォルトでコンパイルされます。追加の機能は必要ありません。

2. **GPU 対応ビルド (オプション)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU のサポートには、ビルド中に利用可能な `nvcc` を備えた SM80+ CUDA ツールキットが必要です。【crates/fastpq_prover/Cargo.toml:11】

3. **セルフテスト**
   ```bash
   cargo test -p fastpq_prover
   ```
   これをリリース ビルドごとに 1 回実行して、パッケージ化する前に Stage6 パスを確認します。

### メタルツールチェーンの準備 (macOS)
1. ビルドする前に Metal コマンド ライン ツールをインストールします。`xcode-select --install` (CLI ツールがない場合) および `xcodebuild -downloadComponent MetalToolchain` をインストールして、GPU ツールチェーンをフェッチします。ビルド スクリプトは `xcrun metal`/`xcrun metallib` を直接呼び出し、バイナリが存在しない場合はすぐに失敗します。【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. CI の前にパイプラインを検証するには、ビルド スクリプトをローカルにミラーリングします。
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   これが成功すると、ビルドは `FASTPQ_METAL_LIB=<path>` を発行します。ランタイムはその値を読み取り、metallib を決定的にロードします。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Metal ツールチェーンを使用せずにクロスコンパイルする場合は、`FASTPQ_SKIP_GPU_BUILD=1` を設定します。ビルドは警告を出力し、プランナーは CPU パス上に残ります。【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Metal が利用できない場合 (フレームワークがない、サポートされていない GPU、または空の `FASTPQ_METAL_LIB`)、ノードは自動的に CPU にフォールバックします。ビルド スクリプトは環境変数をクリアし、プランナーはダウングレードをログに記録します。【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### リリースチェックリスト (ステージ 6)
以下のすべての項目が完了して添付されるまで、FASTPQ リリース チケットはブロックされたままにしておきます。

1. **1 秒未満の証明メトリクス** — 新しくキャプチャされた `fastpq_metal_bench_*.json` を検査し、
   `benchmarks.operations` エントリを確認します。ここで、`operation = "lde"` (およびミラーリングされた
   `report.operations` サンプル) は、20000 行のワークロードに対して `gpu_mean_ms ≤ 950` をレポートします (32768 が埋め込まれています)
   行）。天井の外側のキャプチャでは、チェックリストに署名する前に再実行が必要です。
2. **署名されたマニフェスト** — 実行
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   したがって、リリースチケットにはマニフェストとその分離された署名の両方が含まれます
   (`artifacts/fastpq_bench_manifest.sig`)。レビュー担当者は、事前にダイジェストと署名のペアを検証します。
   リリースの促進。【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 マトリックス マニフェスト (ビルド済み)
   `scripts/fastpq/capture_matrix.sh` 経由) はすでに 20k 行のフロアをエンコードしており、
   回帰のデバッグ。
3. **証拠の添付ファイル** — Metal ベンチマーク JSON、stdout ログ (または Instruments トレース) をアップロードします。
   CUDA/Metal マニフェスト出力、およびリリース チケットへの切り離された署名。チェックリストのエントリ
   すべてのアーティファクトと署名に使用される公開鍵のフィンガープリントにリンクする必要があるため、下流の監査が可能になります。
   検証ステップを再実行できます。【artifacts/fastpq_benchmarks/README.md:65】### 金属検証ワークフロー
1. GPU 対応ビルド後、`FASTPQ_METAL_LIB` が `.metallib` (`echo $FASTPQ_METAL_LIB`) を指していることを確認し、ランタイムが確定的にロードできるようにします。【crates/fastpq_prover/build.rs:188】
2. GPU レーンを強制的にオンにしてパリティ スイートを実行します:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`。バックエンドは Metal カーネルを実行し、検出が失敗した場合は確定的な CPU フォールバックをログに記録します。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. ダッシュボードのベンチマーク サンプルをキャプチャします:\
   コンパイルされた Metal ライブラリ (`fd -g 'fastpq.metallib' target/release/build | head -n1`) を見つけます。
   `FASTPQ_METAL_LIB` 経由でエクスポートし、\ を実行します。
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`。
  正規の `fastpq-lane-balanced` プロファイルでは、すべてのキャプチャが 32,768 行 (2¹⁵) にパディングされるため、JSON は `rows` と `padded_rows` の両方を Metal LD​​E レイテンシーとともに伝えます。 `zero_fill` またはキュー設定により、AppleM シリーズ ホストで GPU LDE が 950 ミリ秒 (<1 秒) ターゲットを超えた場合は、キャプチャを再実行します。結果として得られる JSON/ログを他のリリース証拠と一緒にアーカイブします。夜間の macOS ワークフローは同じ実行を実行し、比較のためにアーティファクトをアップロードします。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  ポセイドンのみのテレメトリが必要な場合 (たとえば、機器のトレースを記録するため)、上記のコマンドに `--operation poseidon_hash_columns` を追加します。ベンチは引き続き `FASTPQ_GPU=gpu` を尊重し、`metal_dispatch_queue.poseidon` を発行し、新しい `poseidon_profiles` ブロックを含めるため、リリース バンドルには Poseidon のボトルネックが明示的に文書化されています。
  証拠には `zero_fill.{bytes,ms,queue_delta}` と `kernel_profiles` (カーネルごと) が含まれるようになりました。
  占有率、推定 GB/秒、および継続時間の統計）により、GPU 効率をグラフ化できます。
  生のトレースを再処理し、`twiddle_cache` ブロック (ヒット/ミス + `before_ms`/`after_ms`)
  キャッシュされた twiddle アップロードが有効であることを証明します。 `--trace-dir` はハーネスを再起動します。
  `xcrun xctrace record` および
  タイムスタンプ付きの `.trace` ファイルを JSON と一緒に保存します。まだオーダーメイドを提供できます
  `--trace-output` (オプションの `--trace-template` / `--trace-seconds`)
  カスタムの場所/テンプレート。 JSON は監査用に `metal_trace_{template,seconds,output}` を記録します。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】各キャプチャ後に `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` を実行すると、パブリケーションには Grafana ボード/アラート バンドル (`dashboards/grafana/fastpq_acceleration.json`、`dashboards/alerts/fastpq_acceleration_rules.yml`) のホスト メタデータ (現在は `metadata.metal_trace` が含まれます) が含まれます。レポートには操作ごとに `speedup` オブジェクト (`speedup.ratio`、`speedup.delta_ms`) が含まれるようになり、ラッパーは `zero_fill_hotspots` (バイト、レイテンシ、派生 GB/秒、およびメタル キュー デルタ カウンタ) をホイストし、`kernel_profiles` をフラット化します。 `benchmarks.kernel_summary` は、`twiddle_cache` ブロックをそのまま保持し、新しい `post_tile_dispatches` ブロック/サマリーをコピーして、レビュー担当者がキャプチャ中にマルチパス カーネルが実行されたことを証明できるようにします。また、ポセイドン マイクロベンチの証拠を `benchmarks.poseidon_microbench` に要約して、ダッシュボードがスカラーとデフォルトのレイテンシーを引用できるようにします。生のレポートを再解析します。マニフェスト ゲートは同じブロックを読み取り、それを省略する GPU 証拠バンドルを拒否するため、ポストタイリング パスがスキップされるたびにオペレータがキャプチャを更新する必要があるか、設定が間違っています。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  Poseidon2 Metal カーネルは同じノブを共有しています。`FASTPQ_METAL_POSEIDON_LANES` (32 ～ 256、2 の累乗) および `FASTPQ_METAL_POSEIDON_BATCH` (レーンごとに 1 ～ 32 状態) を使用すると、再構築せずに起動幅とレーンごとの動作を固定できます。ホストは、すべてのディスパッチの前に、これらの値を `PoseidonArgs` にスレッドします。デフォルトでは、ランタイムは `MTLDevice::{is_low_power,is_headless,location}` を検査して、低電力 SoC が `256×8` に留まりながら、個別 GPU を VRAM 階層起動に向けてバイアスします (48GiB 以上が報告された場合は `256×24`、32GiB では `256×20`、それ以外の場合は `256×16`)。 (古い 128/64 レーン パーツはレーンごとに 8/6 状態に固執します) そのため、ほとんどのオペレーターは手動で環境変数を設定する必要がありません。そして、両方の起動プロファイルとスカラー レーンに対する測定された速度向上を記録する `poseidon_microbench` ブロックを発行するため、リリース バンドルは新しいカーネルが実際に `poseidon_hash_columns` 縮小することを証明できます。また、これには `poseidon_pipeline` ブロックが含まれているため、Stage7 の証拠は新しい占有層と並んでチャンクの深さ/オーバーラップ ノブをキャプチャします。通常の実行では、env を未設定のままにしておきます。ハーネスは再実行を自動的に管理し、子キャプチャが実行できない場合は失敗をログに記録し、`FASTPQ_GPU=gpu` が設定されていても GPU バックエンドが利用できない場合はすぐに終了するため、サイレント CPU フォールバックがパフォーマンスに忍び込むことはありません。アーティファクト。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】ラッパーは、`metal_dispatch_queue.poseidon` デルタ、共有 `column_staging` カウンター、または `poseidon_profiles`/`poseidon_microbench` 証拠ブロックが欠落している Poseidon キャプチャを拒否するため、オペレーターは、重複ステージングまたはスカラー対デフォルトを証明できないキャプチャを更新する必要があります。 Speedup.【scripts/fastpq/wrap_benchmark.py:732】 ダッシュボードまたは CI デルタ用のスタンドアロン JSON が必要な場合は、`python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` を実行します。ヘルパーはラップされたアーティファクトと生の `fastpq_metal_bench*.json` キャプチャの両方を受け入れ、デフォルト/スカラー タイミングで `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` を出力し、メタデータを調整し、記録された高速化を行います。【scripts/fastpq/export_poseidon_microbench.py:1】
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` を実行して実行を終了すると、Stage6 リリース チェックリストは `<1 s` LDE 上限を強制し、リリース チケットに同梱される署名付きマニフェスト/ダイジェスト バンドルを発行します。【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. ロールアウト前にテレメトリを確認します。Prometheus エンドポイント (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) をカールし、予期しない `resolved="cpu"` がないか `telemetry::fastpq.execution_mode` ログを検査します。エントリ.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. CPU フォールバック パスを意図的に強制して (`FASTPQ_GPU=cpu` または `zk.fastpq.execution_mode = "cpu"`)、SRE プレイブックが決定論的な動作と一致するように文書化します。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. オプションの調整: デフォルトでは、ホストは短いトレースでは 16 レーン、中程度では 32 レーンを選択し、`log_len ≥ 10/14` の場合は 64/128、`log_len ≥ 18` の場合は 256 に到達します。また、共有メモリ タイルを小規模なトレースでは 5 段階、`log_len ≥ 12` では 4 段階に維持します。タイル化後のカーネルへの作業を開始する前の、`log_len ≥ 18/20/22` の 12/14/16 ステージ。上記の手順を実行してこれらのヒューリスティックをオーバーライドする前に、`FASTPQ_METAL_FFT_LANES` (8 から 256 までの 2 のべき乗) および/または `FASTPQ_METAL_FFT_TILE_STAGES` (1 ～ 16) をエクスポートします。 FFT/IFFT および LDE 列のバッチ サイズは両方とも、解決されたスレッドグループ幅 (ディスパッチごとに約 2048 論理スレッド、32 列が上限で、ドメインの拡大に応じて 32→16→8→4→2→1 と段階的に減少) から派生しますが、LDE パスではドメインの上限が適用されます。ホスト間でビットごとの比較が必要な場合、確定的な FFT バッチ サイズを固定するには `FASTPQ_METAL_FFT_COLUMNS` (1 ～ 32) を設定し、同じオーバーライドを LDE ディスパッチャーに適用するには `FASTPQ_METAL_LDE_COLUMNS` (1 ～ 32) を設定します。 LDE タイルの深さは FFT ヒューリスティックも反映します。`log₂ ≥ 18/20/22` によるトレースは、ワイド バタフライをポストタイリング カーネルに渡す前に 12/10/8 共有メモリ ステージのみを実行します。`FASTPQ_METAL_LDE_TILE_STAGES` (1 ～ 32) を介してその制限をオーバーライドできます。ランタイムは、Metal カーネル引数を介してすべての値をスレッド化し、サポートされていないオーバーライドをクランプし、解決された値をログに記録するため、metallib を再構築せずに実験を再現できるようになります。ベンチマーク JSON は、解決されたチューニングと LDE 統計経由でキャプチャされたホスト ゼロフィル バジェット (`zero_fill.{bytes,ms,queue_delta}`) の両方を明らかにするため、キュー デルタが各キャプチャに直接関連付けられます。また、`column_staging` ブロック (フラット化されたバッチ、 flatten_ms 、 wait_ms 、 wait_ratio ) が追加されるため、レビュー担当者はダブル バッファー パイプラインによって導入されたホスト/デバイスのオーバーラップを検証できます。 GPU がゼロフィル テレメトリの報告を拒否した場合、ハーネスはホスト側のバッファ クリアから決定論的なタイミングを合成し、それを `zero_fill` ブロックに挿入するようになりました。そのため、リリース証拠は、フィールド.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575]【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. 個別の Mac ではマルチキューのディスパッチは自動です。`Device::is_low_power()` が false を返すか、メタル デバイスがスロット/外部の場所を報告すると、ホストは 2 つの `MTLCommandQueue` をインスタンス化し、ワークロードが 16 列以上 (ファンアウトによってスケールされる) を運ぶ場合にのみファンアウトし、列バッチをキュー間でラウンドロビンするため、長いトレースは両方の GPU レーンをビジー状態に保ちます。決定論を損なう。マシン間で再現可能なキャプチャが必要な場合は、ポリシーを `FASTPQ_METAL_QUEUE_FANOUT` (1 ～ 4 キュー) および `FASTPQ_METAL_COLUMN_THRESHOLD` (ファンアウト前の最小合計列) でオーバーライドします。パリティ テストではこれらのオーバーライドが強制されるため、マルチ GPU Mac はカバーされたままとなり、解決されたファンアウト/しきい値はキュー深度テレメトリの隣に記録されます。### アーカイブする証拠
|アーティファクト |キャプチャ |メモ |
|----------|-----------|----------|
| `.metallib` バンドル | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` と `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` に、`xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` と `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib` が続きます。 | Metal CLI/ツールチェーンがインストールされ、このコミットの決定論的ライブラリが生成されたことを証明します。【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
|環境スナップショット |ビルド後は `echo $FASTPQ_METAL_LIB`。リリースチケットでは絶対パスを保持してください。 |出力が空の場合は、Metal が無効になっていることを意味します。値を記録すると、GPU レーンが出荷アーティファクトで引き続き利用可能であることが文書化されます。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
| GPUパリティログ | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` を作成し、`backend="metal"` またはダウングレード警告を含むスニペットをアーカイブします。 |ビルドをプロモートする前にカーネルが実行 (または決定論的にフォールバック) することを示します。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
|ベンチマーク出力 | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` 経由でラップして署名します。 |ラップされた JSON レコード `speedup.ratio`、`speedup.delta_ms`、FFT チューニング、パディング行 (32,768)、エンリッチされた `zero_fill`/`kernel_profiles`、フラット化された `kernel_summary`、検証済み`metal_dispatch_queue.poseidon`/`poseidon_profiles` ブロック (`--operation poseidon_hash_columns` が使用されている場合)、および GPU LDE 平均が 950 ミリ秒以下に留まり、ポセイドンが 1 秒未満に留まるためのトレース メタデータ。バンドルと生成された `.json.asc` 署名の両方をリリース チケットとともに保持し、ダッシュボードと監査人が再実行せずにアーティファクトを検証できるようにします。ワークロード。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
|ベンチマニフェスト | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`。 |両方の GPU アーティファクトを検証し、LDE 平均が `<1 s` の上限を超える場合は失敗し、BLAKE3/SHA-256 ダイジェストを記録し、検証可能なメトリクスなしではリリース チェックリストを進めることができないように署名されたマニフェストを発行します。【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA バンドル | SM80 ラボ ホストで `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` を実行し、JSON を `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` にラップ/署名し (ダッシュボードが正しいクラスを選択するように `--label device_class=xeon-rtx-sm80` を使用します)、パスを `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` に追加し、`.json`/`.asc` を保持します。マニフェストを再生成する前に、Metal アーティファクトとペアリングします。チェックインされた `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` は、監査人が期待する正確なバンドル形式を示しています。【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
|テレメトリーの証明 | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` と起動時に出力される `telemetry::fastpq.execution_mode` ログ。 |トラフィックを有効にする前に、Prometheus/OTEL が `device_class="<matrix>", backend="metal"` (またはダウングレード ログ) を公開することを確認します。【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 ||強制CPUドリル | `FASTPQ_GPU=cpu` または `zk.fastpq.execution_mode = "cpu"` で短いバッチを実行し、ダウングレード ログをキャプチャします。 |リリース中にロールバックが必要な場合に備えて、SRE Runbook を決定論的なフォールバック パスに合わせて維持します。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
|トレースキャプチャ (オプション) | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` でパリティ テストを繰り返し、発行されたディスパッチ トレースを保存します。 |ベンチマークを再実行することなく、後のプロファイリング レビューのために占有/スレッドグループの証拠を保存します。

多言語 `fastpq_plan.*` ファイルはこのチェックリストを参照しているため、ステージングおよび運用オペレーターは同じ証拠追跡をたどります。【docs/source/fastpq_plan.md:1】

## 再現可能なビルド
ピン留めされたコンテナーのワークフローを使用して、再現可能な Stage6 アーティファクトを生成します。

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

ヘルパー スクリプトは、`rust:1.88.0-slim-bookworm` ツールチェーン イメージ (および GPU 用の `nvidia/cuda:12.2.2-devel-ubuntu22.04`) をビルドし、コンテナー内でビルドを実行し、`manifest.json`、`sha256s.txt`、およびコンパイルされたバイナリをターゲット出力に書き込みます。ディレクトリ.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

環境の上書き:
- `FASTPQ_RUST_IMAGE`、`FASTPQ_RUST_TOOLCHAIN` – 明示的な Rust ベース/タグを固定します。
- `FASTPQ_CUDA_IMAGE` – GPU アーティファクトを生成するときに CUDA ベースを交換します。
- `FASTPQ_CONTAINER_RUNTIME` – 特定のランタイムを強制します。デフォルトの `auto` は `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` を試行します。
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – ランタイム自動検出のカンマ区切りの優先順位 (デフォルトは `docker,podman,nerdctl`)。

## 構成の更新
1. TOML でランタイム実行モードを設定します。
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   値は `FastpqExecutionMode` を通じて解析され、起動時にバックエンドにスレッドされます。【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. 必要に応じて起動時にオーバーライドします。
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI はノードが起動する前に解決された設定をオーバーライドします。【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. 開発者はエクスポートすることで、構成に触れることなく一時的に検出を強制できます。
   バイナリを起動する前は `FASTPQ_GPU={auto,cpu,gpu}`。オーバーライドがログに記録され、パイプラインが
   まだ解決済みモードが表示されています。【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## 検証チェックリスト
1. **起動ログ**
   - ターゲット `telemetry::fastpq.execution_mode` から `FASTPQ execution mode resolved` を予期します。
     `requested`、`resolved`、および `backend` ラベル。【crates/fastpq_prover/src/backend.rs:208】
   - 自動 GPU 検出では、`fastpq::planner` からのセカンダリ ログが最終レーンを報告します。
   - metallib が正常にロードされると、メタル ホストは `backend="metal"` を表示します。コンパイルまたはロードが失敗すると、ビルド スクリプトは警告を発し、`FASTPQ_METAL_LIB` をクリアし、プランナーは続行する前に `GPU acceleration unavailable` を記録します。 CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Prometheus メトリクス**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   カウンタは `record_fastpq_execution_mode` を介してインクリメントされます (現在は というラベルが付けられています)
   `{device_class,chip_family,gpu_kind}`) ノードが実行を解決するたびに
   モード.【crates/iroha_telemetry/src/metrics.rs:8887】
   - 金属カバレッジについては確認してください
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     導入ダッシュボードと一緒に増分します。【crates/iroha_telemetry/src/metrics.rs:5397】
   - `irohad --features fastpq-gpu` でコンパイルされた macOS ノードは追加で公開します
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     そして
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` つまり、Stage7 ダッシュボード
     ライブ Prometheus スクレイピングからデューティ サイクルとキューのヘッドルームを追跡できます。【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **テレメトリのエクスポート**
   - OTEL ビルドは、同じラベルを持つ `fastpq.execution_mode_resolutions_total` を発行します。あなたの
     ダッシュボードまたはアラートは、GPU がアクティブである必要があるときに予期しない `resolved="cpu"` を監視します。

4. **正気性の証明/検証**
   - `iroha_cli` または統合ハーネスを介して小さなバッチを実行し、プルーフが検証されていることを確認します。
     同じパラメータでコンパイルされたピア。

## トラブルシューティング
- **解決モードは GPU ホスト上の CPU に留まります** — バイナリが次の方法でビルドされたことを確認してください
  `fastpq_prover/fastpq-gpu`、CUDA ライブラリがローダー パス上にあり、`FASTPQ_GPU` は強制されていません
  `cpu`。
- **Apple Silicon では Metal を使用できません** - CLI ツールがインストールされていることを確認し (`xcode-select --install`)、`xcodebuild -downloadComponent MetalToolchain` を再実行し、ビルドで空ではない `FASTPQ_METAL_LIB` パスが生成されたことを確認します。値が空または欠落している場合、仕様によりバックエンドが無効になります。【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` エラー** - 証明者と検証者の両方が同じ正規カタログを使用していることを確認します
  `fastpq_isi` によって発行されます。サーフェスが `Error::UnknownParameter` と一致しません。【crates/fastpq_prover/src/proof.rs:133】
- **予期しない CPU フォールバック** — `cargo tree -p fastpq_prover --features` を検査し、
  `fastpq_prover/fastpq-gpu` が GPU ビルドに存在することを確認します。 `nvcc`/CUDA ライブラリが検索パス上にあることを確認します。
- **テレメトリ カウンタがありません** — ノードが `--features telemetry` (デフォルト) で起動されたことを確認します。
  また、OTEL エクスポート (有効な場合) にはメトリック パイプラインが含まれます。【crates/iroha_telemetry/src/metrics.rs:8887】

## フォールバック手順
決定的プレースホルダー バックエンドは削除されました。回帰によりロールバックが必要な場合は、
Stage6 を再発行する前に、以前に正常に動作していたリリース アーティファクトを再デプロイし、調査します。
バイナリ。変更管理の決定を文書化し、フォワード ロールが完了した後でのみ完了するようにします。
退化は理解されています。

3. テレメトリを監視して、`fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` が予想される値を反映していることを確認します。
   プレースホルダーの実行。

## ハードウェア ベースライン
|プロフィール | CPU | GPU |メモ |
| ------- | --- | --- | ----- |
|リファレンス (Stage6) | AMD EPYC7B12 (32 コア)、256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000 行の合成バッチは 1000 ミリ秒以内に完了する必要があります。【docs/source/fastpq_plan.md:131】 |
| CPUのみ |物理コア 32 以上、AVX2 | – | 20000 行の場合は約 0.9 ～ 1.2 秒かかると予想されます。決定論のために `execution_mode = "cpu"` を保持します。 |## 回帰テスト
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU ホスト上)
- オプションのゴールデンフィクスチャチェック:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

このチェックリストからの逸脱を運用ランブックに文書化し、完了後に `status.md` を更新します。
移行ウィンドウが完了します。