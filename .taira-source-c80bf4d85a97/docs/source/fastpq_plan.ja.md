<!-- Japanese translation of docs/source/fastpq_plan.md -->

---
lang: ja
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
translator: manual
---

# FASTPQ プルーバー作業計画

この文書は、プロダクション品質の FASTPQ-ISI プルーバーを提供し、データスペースのスケジューリング・パイプラインへ統合するための段階的な計画をまとめたものです。特記がない限り記述は規範的であり、Cairo 系 DEEP-FRI 境界を用いた健全性推定を前提とします。CI で実行される自動リジェクションサンプリング・テストは、健全性推定が 128 ビットを下回った場合に失敗します。

この計画は `roadmap.md` に記載された「FASTPQ Production Track」と常に同期しており、ステージ名や成果物、ステータスの変更は両ファイル同時に更新する必要があります。

## 完了済みの基盤

### プレースホルダーバックエンド *(完了)*
- Norito の決定的エンコードと BLAKE2b コミットメント。
- `fastpq-placeholder` フィーチャー配下で決定的アーティファクトを提供するプレースホルダープルーバーにより、上位レイヤーが先行統合できるようにする。
- 正準パラメータテーブルを `fastpq_isi` で提供。

### トレースビルダー試作 *(2025-11-09 完了)*
> **ステータス:** `fastpq_prover` が正準パッキングヘルパー（`pack_bytes`, `PackedBytes`）と、Goldilocks 上の決定的 Poseidon2 順序コミットメントを公開。定数は `ark-poseidon2` コミット `3f2b7fe` に固定済み。ゴールデンフィクスチャ（`tests/fixtures/packing_roundtrip.json`, `tests/fixtures/ordering_hash.json`）がリグレッションスイートの基準として追加された。

- 各トレース行には以下を格納:
  - `key_limbs[i]`: 標準化されたキー（base-256、7byte LE）を Goldilocks 要素として格納。
  - `value_old_limbs[i]`, `value_new_limbs[i]`: 旧値／新値の同形式パッキング。
  - セレクタ列: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`。
  - 補助列: `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`。
  - 資産列: `asset_id_limbs[i]`（7byte LE）。
  - SMT 列（階層ごと）: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, 非会員証明用 `neighbour_leaf`。
  - メタデータ列: `dsid`, `slot`。
- **決定的順序付け:** `(key_bytes, op_rank, original_index)` で安定ソートし、Poseidon2 ドメインタグ `fastpq:v1:ordering` でコミット。ハッシュ前像は `[domain_len, domain_limbs…, payload_len, payload_limbs…]`（長さは u64 フィールド要素）形式にエンコード。
- ルックアップ証人: `s_perm = 1` の場合、`perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` を生成（role/permission は 32byte LE、epoch は 8byte LE）。
- セレクタの排他性、資産保存、`dsid`/`slot` の固定値を生成時と AIR 内で検証。
- `N_trace = 2^k`（行数の 2 冪切り上げ）、`N_eval = N_trace * 2^b`（blowup 指数 b）。
- フィクスチャ／テスト:
  - パッキング往復テスト（`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`）。
  - 順序ハッシュの安定性検証（`tests/fixtures/ordering_hash.json`）。
  - バッチフィクスチャ（`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`）。

#### AIR 列スキーマ
| 列グループ | 名称 | 説明 |
|------------|------|------|
| Activity | `s_active` | 実データ行で 1、パディング行で 0。 |
| Main | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | Goldilocks 要素（LE、7byte）としてのキー／値。 |
| Asset | `asset_id_limbs[i]` | 標準化資産 ID（LE、7byte）。 |
| Selectors | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | 0/1。合計が `s_active` と一致し、`s_perm` は役割付与／剥奪行を反映。 |
| Auxiliary | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | 制約・監査用の補助列。 |
| SMT | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | Poseidon2 入出力と非会員証明。 |
| Lookup | `perm_hash` | 権限ルックアップ用 Poseidon2 ハッシュ。 |

各階層の更新ルール:
```
if path_bit_ℓ == 0:
    node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
else:
    node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
```
挿入行は `(node_in_0 = 0, node_out_0 = value_new)`、削除行は `(node_in_0 = value_old, node_out_0 = 0)`。非会員証明では `neighbour_leaf` で空領域を示す。最終行で `old_root`／`new_root` と一致する境界制約を課す。

## 今後のステージ

### ステージ 0 — パラメータ再構成と計画 *(進行中)*
- `fastpq_isi::StarkParameterSet` に以下を追加: `trace_log_size`, `trace_root`, `lde_log_size`, `lde_root`, `permutation_size`, `lookup_log_size`, `omega_coset`。
- Poseidon シードを使った生成手順:
  - ドメインタグ `fastpq:v1:domain_roots` をスポンジへ吸収。
  - 各パラメータで `(trace_root, lde_root, omega_coset)` を導出し、`2^trace_log_size`／`2^lde_log_size` の原始元であることを検証。
  - プロセスを付録 B に記載し、`fastpq_isi/src/params.rs` のテーブルを更新。
- `fastpq_prover::fft` のハードコード定数（例: `GOLDILOCKS_PRIMITIVE_ROOT`）を導出結果に置き換え。  
  再生成コマンド:  
  `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots`
- `find_by_name` テストを更新し、新フィールドを検証するリグレッションテストを追加。
- `fastpq-prover-preview` フィーチャーで新 FFT プランナーを段階導入し、プレースホルダーを併用。
- スキーマ付録、`status.md`, `roadmap.md` を同時更新。

正準メタデータ（Goldilocks, p = 2^64 - 2^32 + 1）:

| パラメータ | trace_log_size | trace_root | lde_log_size | lde_root | permutation_size | lookup_log_size | omega_coset |
|------------|----------------|------------|--------------|----------|------------------|-----------------|-------------|
| `fastpq-lane-balanced` | 14 | `0x79c0_926e_32a5_e647` | 17 | `0x3bdd_b7b6_4404_fc19` | 16,384 | 17 | `0x916f_c4aa_51c6_2011` |
| `fastpq-lane-latency`  | 14 | `0x2853_ec35_498f_58f5` | 18 | `0x74e8_2886_9ab7_331b` | 16,384 | 18 | `0x483d_36dc_097c_9ec2` |

`fastpq-prover-preview` フィーチャーを利用すると、新プランナーを opt-in で利用しつつ既存のプレースホルダーバックエンドを維持できる。

### ステージ 1 — FFT / LDE 基盤 *(進行中)*
- ✅ `Planner::new(&StarkParameterSet)` がステージ 0 で導出したトレース／LDE ドメイン表をキャッシュし、原始根および 2-アディシティを事前検証。
- ✅ `fft_columns` / `ifft_columns` は Rayon 並列の反復 DIT 実装で列順序を決定的に保持。
- ✅ `lde_columns(&self, coeffs: &[Vec<u64>]) -> Vec<Vec<u64>>` は正準コセットの冪で係数を前処理し、キャッシュ済み LDE ドメイン上で評価。
- ✅ Criterion ベンチマークは正準トレース長の上限 (現在は 2^16。日次の 32k バッチを含む) を計測し、将来 `trace_log_size` が増えた際にも自動でスケール。目標は 32 コア CPU で <150 ms/列。
- ✅ テストおよび `fastpq::planner` ログが FFT/IFFT round-trip、ドメイン決定性、LDE 評価の再現性をカバー。

### ステージ 2 — 列／ルックアップ変換 *(進行中)*
- ✅ `trace_commitment` を係数ベースにリファクタリング。
- ✅ 列コミットメントをストリーミング Poseidon（`commit(coeffs) = Poseidon(tag || coeffs)`）に変更。
- ✅ 行コミットメント:
  - LDE ドメインで一度列を評価し、行ハッシュ組立てに再利用。
  - `hash_trace_rows` を `[row_index, column_count, value_0, …]` 形式の Poseidon ストリームへ置換。
- ✅ ルックアップ証人:
  - 係数空間で証人ベクトルを構築し、LDE ドメインで評価してグランドプロダクトに利用。
  - `perm_root` コミットメントも同パイプラインで生成。
- 互換性テスト:
  - 小規模トレースで新コミットメントが従来プレースホルダーと一致するか検証。
- `fastpq-placeholder` フィーチャーによる旧コードはステージ 6 で撤去済み。
- ✅ ドキュメント／フィクスチャ更新:
  - `lookup_grand_product.md`, `smt_update.md` の例を刷新（ルックアップ積のハッシュ化とレベル別 SMT 列を反映）。
  - 付録 C にトランスクリプトのドメインタグや列順序を記録。

### ステージ 3 — DEEP-FRI 実装 *(未着手)*
- 折りたたみループ:
  - コセット上の評価値を用い、アリティ 8/16 の多項式折り畳みを実行。
  - 各レイヤーのルートを Poseidon ストリームでコミットし、トランスクリプトへ追加。
  - `U(X) = f(X) + β * f(γX^arity + …)` を仕様通り計算。
- トランスクリプト更新:
  - `Transcript::append_fr_layer(round, root)` と `challenge_beta(round)` を実装。
  - β チャレンジをプルーバー・ベリファイア双方で参照。
- 葉ハッシュ:
  - ステージ 2 のストリーミングを利用し、クエリ用のメルクルパスをバッチ生成。
- テスト:
  - Winterfell 互換のリファレンス実装と比較するトイ多項式テスト。
  - β／ルート／クエリ値の改変による否定テストでベリファイア拒否を確認。

### ステージ 4 — クエリサンプリングと検証 *(進行中)*
- ✅ クエリサンプラー:
  - Fiat–Shamir チャレンジで評価インデックスを導出し、重複を排除してソートし、ドメイン長に合わせて上限をかける実装が着地済み。
- ✅ 検証:
  - プランナー出力から多項式コミットメントを再構築し、改ざんされた γ・グランドプロダクト・クエリオープニングを拒否するテストを追加。
- ✅ クロステスト:
  - `crates/fastpq_prover/tests/transcript_replay.rs` がプレビュー用フィクスチャ `tests/fixtures/stage4_balanced_preview.bin` を再生し、Fiat–Shamir トランスクリプトが決定的に再現されることを検証。
- ドキュメント: `nexus.md` のフローチャートを更新。

### ステージ 5 — GPU / SIMD 最適化 *(進行中)*
- 対象カーネル: LDE(NTT), Poseidon2 ハッシュ, メルクル構築, FRI 折り畳み。
- 決定性: fast-math 無効化、CPU/CUDA/Metal でビット一致を保証し、CI で証明ルートを比較。
- Metal バックエンド (Apple Silicon):
  - ビルドスクリプトが `metal/kernels/ntt_stage.metal` と `metal/kernels/poseidon2.metal` を `xcrun metal`/`xcrun metallib` でコンパイルし、`fastpq.metallib` を生成する。開発ホストには Metal CLI（`xcode-select --install` と必要に応じて `xcodebuild -downloadComponent MetalToolchain`）が入っていることを確認する。【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - `build.rs` と同じ手順を手動で実行し、CI キャッシュのウォームアップや決定的なパッケージングに利用できる:
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    完了後は `FASTPQ_METAL_LIB=<path>` が設定され、ランタイムで決定的に読み込める。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - Metal ツールチェーンを取得できないクロスコンパイルでは `FASTPQ_SKIP_GPU_BUILD=1` を指定し、警告を記録した上で CPU 経路を維持する。【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
  - プロファイル用途でスレッドグループ幅を切り替える場合は `FASTPQ_METAL_THREADGROUP=<width>` を指定できる。デバイス制限でクランプされ、ログに記録されるため探索結果と突き合わせられる。【crates/fastpq_prover/src/metal.rs:321】
  - FFT タイルのチューニングは `FASTPQ_METAL_FFT_LANES`（8〜256 の 2 の累乗）と `FASTPQ_METAL_FFT_TILE_STAGES`（1〜16）で行える。デフォルトはログサイズに応じて 16 → 32（`log_len ≥ 6`）→ 64（`log_len ≥ 10`）→ 128（`log_len ≥ 14`）→ 256（`log_len ≥ 18`）レーンを選び、ステージ数も 5 → 4（`log_len ≥ 12`）→ 12/14/16（`log_len ≥ 18/20/22`）へと引き上げたうえでポストタイルカーネルに渡す。ランタイムが値を `FftArgs` に通し、サポート範囲外はクランプ＋ログ出力するため、metallib を作り直さずに決定的なプロファイルスイープが可能。【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT と LDE の列バッチ数は解決済みの threadgroup 幅から導出され、1 コマンドあたり約 4,096 論理スレッドを目標に 64 列までを一度にまとめる “circular buffer” タイルを採用した。`log₂(len)` が 2¹⁶/2¹⁸/2²⁰/2²² を超えるまでは 64 列を維持し、それ以降で 64→32→16→8→4→2→1 列へ段階的に縮小するため、20k 行キャプチャでも 64 列単位の dispatch が保証される。アダプティブスケジューラはおおよそ 2 ms のターゲットに近づくまで列幅を倍増させ続け、観測されたディスパッチ時間が目標の 130% を超えた場合には列数を自動的に半減して lane / tile 変更後でも即座に安全なレンジへ戻す。Poseidon の permutation も同じスケジューラを共有し、`fastpq_metal_bench` の `metal_heuristics.batch_columns.poseidon` ブロックに解決された状態数・上限・直近の計測値・override 状態が記録されるため、Poseidon のチューニングとキュー深度テレメトリをワンステップで突き合わせられる。`FASTPQ_METAL_FFT_COLUMNS`（1〜64）で FFT 向けの固定値を指定でき、LDE 側は `FASTPQ_METAL_LDE_COLUMNS`（1〜64）で同様に強制できる。ベンチマークは `kernel_profiles.*.columns` に解決結果を出力するためチューニング比較が再現しやすい。【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1284】
- ディスクリート GPU（`is_low_power` が false もしくは `location` が Slot/External）ではマルチキュー dispatch が自動で働き、既定で 2 本の `MTLCommandQueue` を確保して総列数が 16×fan-out 以上のワークロードだけラウンドロビンで投げる。コマンドバッファセマフォは “各キュー最低 2 本 in-flight” を強制し、`metal_dispatch_queue` には集計ウィンドウ (`window_ms`) と正規化されたビジー率 (`busy_ratio`) がグローバル／各キュー両方で記録されるため、両キューが同じ時間幅で 50% 以上ビジーだったことをリリースアーティファクトで証明できる。`FASTPQ_METAL_QUEUE_FANOUT`（1〜4）と `FASTPQ_METAL_COLUMN_THRESHOLD` でファンアウト／しきい値を固定でき、Metal パリティテストは両環境変数を強制してマルチ GPU 環境でも決定性を継続的に検証する。【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- Metal 検出はまず `MTLCreateSystemDefaultDevice` / `MTLCopyAllDevices` を（headless シェルでは CoreGraphics セッションをウォームアップしたうえで）試し、それでも見つからない場合にのみ `system_profiler` にフォールバックするようになった。`FASTPQ_DEBUG_METAL_ENUM=1` を付けるとこの列挙結果をそのまま標準エラーへ吐き出し、`FASTPQ_GPU=gpu` を強制したのに GPU が見つからなかった場合は `fastpq_metal_bench` 自体がエラーで停止するため、“黙って CPU へ戻る” ケースを潰しつつトリアージログをそのままハーネスの出力へ入れられる。【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
- Poseidon GPU 計測もフォールバックを正しく区別するようになった。`hash_columns_gpu` は結果が GPU 由来かどうかを返し、`measure_poseidon_gpu` は CPU フォールバックが起きた瞬間にサンプルを破棄して警告を出し、Poseidon microbench の子プロセスも GPU が無い環境では即座に失敗する。これにより `gpu_recorded=false` が確実に立ち、キューのサマリも同じウィンドウを報告し続けるため、ダッシュボードがすぐに退行を検知できる。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】
- `fastpq_metal_bench` は `device_profile` ブロックを出力し、デバイス名・registry id・`low_power`/`headless` フラグ・location（built_in/slot/external）・ディスクリート GPU かどうか・`hw.model`・Apple SoC ラベル（例: “M3 Max”）を同じ JSON に含めるようになった。Stage 7 のダッシュボードはこの情報で M4/M3 と離散 GPU を簡単にバケットでき、キュー／ヒューリスティックの証跡と並べて保存することで各ベンチがどのマシンクラスで取得されたかを確実に示せる。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】
- FFT のホスト／デバイス重ね合わせはダブルバッファ方式に移行した。batch *n* が `fastpq_fft_post_tiling` で完了する間に batch *n + 1* を 2 本目のバッファへフラット化し、バッファ再利用が必要なときだけ待機する。バックエンドはフラット化したバッチ数とフラット化 vs GPU 待機時間を記録し、`fastpq_metal_bench` が `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` をレポートへ追加するので、ホストが各ディスパッチ間で遊んでいないことをエビデンスで示せる。Poseidon の `--operation poseidon_hash_columns` も同じダブルバッファ staging を共有するため、追加の計測なしで `column_staging` ブロックとキュー差分が埋まる。【crates/fastpq_prover/src/metal.rs:1006】【crates/fastpq_prover/src/metal.rs:1076】【crates/fastpq_prover/src/metal.rs:2028】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1778】
- Poseidon2 カーネルは高占有の Metal シェーダーへ刷新された。各 threadgroup がラウンド定数と MDS 行を threadgroup メモリにコピーし、全ラウンドをアンロールした状態で 1 レーンあたり複数状態を処理することで、ディスパッチごとに必ず 4,096 以上の論理スレッドを投げる。`FASTPQ_METAL_POSEIDON_LANES`（32〜256 の 2 の累乗）と `FASTPQ_METAL_POSEIDON_BATCH`（1〜32 状態／レーン）でランチ形状を固定でき、ホストは解決した値を `PoseidonArgs` に流すだけで済む。`MTLDevice::{is_low_power,is_headless,location}` から得たヒントを一度だけ記録し、ディスクリート GPU には VRAM 階層ごとに `256×24`（48 GiB 以上）、`256×20`（32 GiB）、それ以外は `256×16` を自動割り当てし、低電力 SoC には `256×8`（128/64 レーン世代には 8/6 状態）を維持するため、設定なしで 16 状態超のパイプライン深度を得られる。`fastpq_metal_bench` は `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` 付きで自分自身を再実行し、`poseidon_microbench` ブロックにスカラーレーンとのスピードアップを記録しつつ、`poseidon_pipeline` テレメトリ（`chunk_columns`/`pipe_depth`/`fallbacks`）もエクスポートして各 GPU キャプチャーで重ね合わせウィンドウを証明する。【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - LDE のタイル段数は FFT と同じヒューリスティックに合わせ、`log₂(len) ≥ 18` で 12 段、log₂20 で 10 段、log₂22 以上では 8 段に制限して残りのステージをポストタイルカーネルへ渡す。決定的な深さが必要な場合は `FASTPQ_METAL_LDE_TILE_STAGES`（1〜32）で上書きでき、ヒューリスティックが途中で止まったときだけ後段のディスパッチが発生するため、キュー深度とカーネルテレメトリは従来通り再現性を保つ。【crates/fastpq_prover/src/metal.rs:827】
  - ベンチマーク報告には `post_tile_dispatches` ブロックが追加され、ポストタイルカーネル（FFT/IFFT/LDE）のディスパッチ回数と開始ステージを記録する。`scripts/fastpq/wrap_benchmark.py` はこれを `benchmarks.post_tile_dispatches` / `benchmarks.post_tile_summary` に持ち上げ、`cargo xtask fastpq-bench-manifest` は GPU 走査にこの証跡が無いと失敗するため、20k 行キャプチャでマルチパス実行が必ず証明される。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - LDE カーネルはホスト側でゼロ初期化された評価バッファを前提とする。バッファを再利用する場合は実行前に 0 クリアすること。【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - coset の乗算は最終段の FFT 内に取り込まれ、追加パスが不要になった。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - 共有メモリ版 FFT/LDE カーネルはタイル深度で処理を終え、残りのステージ（および IFFT の正規化）は専用カーネル `fastpq_fft_post_tiling` が引き継ぐ。Rust ホストは同じカラムバッチを 2 本目のディスパッチに渡し、`log_len` がタイル上限を超えた場合だけ後段を投入するため、キュー深度や `kernel_profiles` のテレメトリを乱さずに GPU 上で広いステージを捌ける。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - ホスト側で段ごとの Goldilocks twiddle を事前計算（1 ステージあたり 8 バイト）し、`(log_len, inverse)` ごとにキャッシュした Metal バッファをリユースすることで毎回のアップロードを避ける。`fastpq_metal_bench` の JSON には `twiddle_cache` ブロック（hits/misses と `before_ms`/`after_ms`）が追加され、旧来のアップロードコストとの差を証跡に残せる。【crates/fastpq_prover/src/metal.rs:896】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:976】
  - カーネル参照（各エントリーポイントの役割、threadgroup 上限、タイル段数、`fastpq.metallib` 再生成手順）は `docs/source/fastpq_metal_kernels.md` にまとまりました。オペレーターやレビュアーはこの文書で Metal 実装を追跡できます。【docs/source/fastpq_metal_kernels.md:1】
  - `FASTPQ_METAL_TRACE=1` を指定すると各ディスパッチのデバッグログ（パイプライン名、スレッドグループ幅、グループ数、経過時間）が出力され、Metal System Trace と付き合わせられる。【crates/fastpq_prover/src/metal.rs:346】
  - Metal のディスパッチキューは `FASTPQ_METAL_MAX_IN_FLIGHT` で同時コマンドバッファ数を制限し、既定値は `system_profiler` から検出した GPU コア数（失敗時は CPU 並列度）を元にキューのファンアウト下限へクランプして導出される。`fastpq_metal_bench` の JSON には `metal_dispatch_queue`（limit/dispatch_count/max_in_flight/busy_ms/overlap_ms）に加えて新しい `metal_heuristics` ブロックが追加され、実際に適用されたコマンド上限と FFT/LDE のバッチ幅（Override が強制されたかどうかを含む）が記録される。Poseidon だけを対象にしたキャプチャ（`FASTPQ_METAL_TRACE=1 --operation poseidon_hash_columns`）では `metal_dispatch_queue.poseidon` サブブロックが併記され、`poseidon_profiles` がカーネルサンプルから帯域・占有率・スレッド構成をまとめる。プローブ後も GPU が zero-fill やキュー深度を報告しない場合はホスト側で決定的なゼロ埋め測定を合成して `zero_fill` ブロックに挿入するため、公開エビデンスが空欄になることはない。【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - ランタイム検出は `system_profiler` で Metal を確認し、フレームワーク・デバイス・metallib のいずれかが欠けている場合は `FASTPQ_METAL_LIB` を空にして CPU 経路へフォールバックする。【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - オペレーター向けチェックリスト (Metal):
    1. ツールチェーンが揃っていること、そして `FASTPQ_METAL_LIB` が生成済みの `.metallib` を指していることを確認する（`cargo build --features fastpq-gpu` 実行後に `echo $FASTPQ_METAL_LIB` が空でないこと）。【crates/fastpq_prover/build.rs:188】
    2. GPU を強制したパリティテストを実行: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`。Metal カーネルを叩き、検出に失敗した場合は自動的に CPU 経路へ戻る。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. ダッシュボード用のベンチ結果を取得: `fd -g 'fastpq.metallib' target/release/build | head -n1` で生成済みの `.metallib` を特定し、
       `FASTPQ_METAL_LIB` に設定した上で
       `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json`
      を実行する。`fastpq-lane-balanced` は 20k 行を 32,768 行にパディングして処理するため、JSON には要求行数とパディング後ドメインの両方が記録される。Metal の LDE は 950ms（Apple M-series での `<1s` 目標）を超えないことを SLO としているため、この値を超えた場合はゼロフィルやキュー深度を確認してから取り直す。生成された JSON / ログを証跡として保存する。macOS Nightly でも同じコマンドを実行し、成果物をアップロードしている。レポートには `fft_tuning.{threadgroup_lanes,tile_stage_limit}` と各操作の `speedup`、LDE セクションの `zero_fill.{bytes,ms,queue_delta}`（queue_delta は Metal コマンドバッファ上限・投入数・同時実行ピーク・busy/overlap ms をまとめた差分カウンタ）、そして新設された `kernel_profiles`（カーネル別のオキュパンシー比・推定帯域幅・継続時間レンジ）が含まれるため、追加処理なしで GPU/CPU の性能差、ホスト側のゼロ埋めコスト、GPU キュー／カーネル挙動を提示できる。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】 Instruments の証跡も必要な場合は `--trace-auto` を付ける（もしくは `--trace-output fastpq.trace` を指定し、必要に応じて `--trace-template` / `--trace-seconds` を上書きする）と、`xcrun xctrace record`（デフォルトは “Metal System Trace”）の下でベンチマークが再実行され、JSON に `metal_trace_{template,seconds,output}` が記録される。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      *(EN note: run `python3 scripts/fastpq/wrap_benchmark.py … --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json --sign-output` so the bundle enforces the <950 ms and <1 s targets, embeds the row-usage snapshot, surfaces `benchmarks.poseidon_microbench`, and emits a detached signature for Grafana/alert assets. When you need a standalone Poseidon microbench artefact, call `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json` to drop `benchmarks/poseidon/poseidon_microbench_<timestamp>.json`; the helper accepts both wrapped bundles and raw `fastpq_metal_bench*.json` captures, and you can rebuild the aggregated manifest with `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`.)*
      JSON には `speedup.ratio` / `speedup.delta_ms` が追加され、GPU と CPU の優位性を証跡内で直接確認できるようになった。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】
     さらにラッパーは `zero_fill_hotspots`（バイト数・ゼロ埋め時間・算出 GB/s・queue_delta 差分）と `kernel_profiles` を出力し、パディングのボトルネックや Metal キュー／カーネルの挙動（帯域幅・オキュパンシー）を raw JSON を読まずに把握でき、`--row-usage` を渡した場合は `metadata.row_usage_snapshot` に転送ガジェットのエビデンスが埋め込まれ、`--sign-output` により `.json.asc` 署名も生成される。【scripts/fastpq/wrap_benchmark.py:1】
    4. 実 ExecWitness から行使用率テレメトリを採取し、転送ガジェットと従来経路の行配分を証跡化する。Torii から witness を取得 (`iroha_cli audit witness --binary --out exec.witness`) した後、`iroha_cli audit witness --decode exec.witness`（必要なら `--fastpq-parameter fastpq-lane-balanced` で期待パラメータを検証）を実行すると、各 FASTPQ バッチに `row_usage`（`total_rows` / `transfer_rows` / `non_transfer_rows`、selector 別カウント、`transfer_ratio`）が付与される。FASTPQ バッチはデフォルトで出力されるため、通常は追加のフラグは不要で、出力を抑えたい場合だけ `--no-fastpq-batches` を付ければよい。この断片を Metal ベンチ JSON と同じディレクトリに保存しておけば、Grafana は追加のトランスクリプト処理なしにガジェット vs レガシーの比率を描画できる。【crates/iroha_cli/src/audit.rs:209】`scripts/fastpq/check_row_usage.py` を使うと、新旧スナップショットを比較して transfer_ratio / total_rows の回帰を即座に検出でき、CI で失敗させることができる。

       ```bash
       python3 scripts/fastpq/check_row_usage.py \
         --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
         --candidate fastpq_row_usage_2025-05-12.json \
         --max-transfer-ratio-increase 0.005 \
         --max-total-rows-increase 0
       ```

       `scripts/fastpq/examples/` にサンプル JSON があり、ローカルのスモークテストで利用できる。手元では
       `make check-fastpq-row-usage`（`ci/check_fastpq_row_usage.sh` を呼び出す）を実行し、CI では
        `ci/check_fastpq_row_usage.sh`（`.github/workflows/fastpq-row-usage.yml`）を実行し、
        `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` のスナップショット同士を比較して
        transfer_ratio / total_rows の回帰を即座に検出している。`--summary-out <path>` を渡すと機械可読な差分
        (`fastpq_row_usage_summary.json`) も生成され、CI アーティファクトとしてアップロードされる。
        GPU が zero-fill テレメトリを返さない場合でも、ホスト側で決定的なゼロ埋め測定を合成して `zero_fill` ブロックに挿入するため、公開 JSON から項目が欠落することはない。
        Stage 7-3 のロールアウトバンドルは `scripts/fastpq/validate_row_usage_snapshot.py` の検証にも合格する必要があり、各 `row_usage`
        エントリがセレクタ別カウントを持ち、`transfer_ratio = transfer_rows / total_rows` の不変条件を満たしていることを確認する。
        `ci/check_fastpq_rollout.sh` がヘルパーを自動実行し、条件を欠いた証跡は GPU 義務化前に拒否される。【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
    5. `curl http://localhost:8180/metrics | rg 'fastpq_execution_mode_total.*backend="metal"'` などでテレメトリを確認し、`telemetry::fastpq.execution_mode` ログに意図しない `resolved="cpu"` が出ていないか監視する。【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    6. メンテナンス時は `FASTPQ_GPU=cpu`（もしくは `zk.fastpq.execution_mode = "cpu"`）で明示的に CPU を強制し、フォールバックのログが出ることを Runbook に記録しておく。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Metal 対応エビデンス（各ロールアウトで下記の証跡を保存し、決定性・テレメトリ・フォールバック要件を満たすこと）:

    | ステップ | 目的 | コマンド／証跡 |
    | --- | --- | --- |
    | metallib ビルド | Metal CLI が導入され、このコミット向けの決定的 `.metallib` を生成できることを確認 | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` → `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` → `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` → `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`。【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | 環境変数確認 | `FASTPQ_METAL_LIB` が空でないことを証跡化し、Metal backend が有効であると示す | `echo $FASTPQ_METAL_LIB`（絶対パスが返るのが期待値。空なら backend 無効化）。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | GPU パリティ | カーネルが実行される／決定的なフォールバックログが出ることを確認 | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` を実行し、`backend="metal"` もしくはフォールバック警告をログとして保存する。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
| ベンチ取得 | `speedup.*` と FFT チューニングを含む JSON / ログを保存してダッシュボードへ供給 | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json` を実行し、JSON と標準出力をリリースノートと一緒に保管する（レポートには要求 20k 行とパディング後 32,768 行の両方が記録され、LDE が 950ms 未満に収まっているか確認できる）。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | テレメトリ確認 | Prometheus / ログに `backend="metal"` が出ていること、もしくはフォールバックログを残す | `curl -s http://<host>:8180/metrics | rg fastpq_execution_mode_total` と起動時の `telemetry::fastpq.execution_mode` ログを保存する。【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    | CPU フォールバック演習 | SRE 手順のために決定的な CPU ルートを練習し、ログを残す | `FASTPQ_GPU=cpu` または `zk.fastpq.execution_mode = "cpu"` で短いワークロードを実行し、ダウングレードのログを保存する。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
    | トレース取得（任意） | `FASTPQ_METAL_TRACE=1` でディスパッチログを収集し、後から lane/tile override を追跡可能にする | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` でパリティテストを 1 回実行し、生成されたトレースを成果物に添付する。【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    証跡はリリースチケットと `docs/source/fastpq_migration_guide.md` のチェックリストに添付し、ステージング／本番で同じ手順を踏む。【docs/source/fastpq_migration_guide.md:1】

### リリースチェックリストの強制

FASTPQ のリリースチケットは、以下のゲートを満たし証跡を添付するまでクローズできない。

1. **サブ秒プローフ指標** — 最新の `fastpq_metal_bench_*.json` を確認し、`operation = "lde"` の `benchmarks.operations`（および対応する `report.operations`）が 20,000 行（32,768 行にパディング）のワークロードで `gpu_mean_ms ≤ 950` を示していること。閾値を超えた場合はゼロフィル／キュー設定を調査し、再取得してから承認する。
2. **署名付きベンチマニフェスト** — Metal/CUDA のバンドルを揃えたら  
  `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   を実行し、生成された `fastpq_bench_manifest.json` と `fastpq_bench_manifest.sig`、使用した公開鍵フィンガープリントをリリースチケットへ添付する。【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】
3. **証跡添付** — Metal ベンチ JSON、stdout ログ（または Instruments トレース）、`fastpq_bench_manifest.{json,sig}` をチケットに並べて貼り付け、レビュアが `fastpq_bench_manifest.json` のダイジェストと照合できるようにする。【artifacts/fastpq_benchmarks/README.md:65】

- テレメトリとフォールバック:
  - 実行モードのログとメトリクス（`telemetry::fastpq.execution_mode`, `fastpq_execution_mode_total{backend="metal"|...}`）で要求値と解決値の差異を可視化し、サイレントフォールバックを検出する。【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - `FASTPQ_GPU={auto,cpu,gpu}` のオーバーライドは継続サポートされ、未知の値は警告を出しつつテレメトリへ露出する。【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - GPU テスト（`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`）は CUDA / Metal の双方で成功する必要があり、metallib が無い場合や検出が失敗した場合は CI が安全にスキップする。【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
- ✅ SM80 回帰ハーネス:
  - `fastpq-gpu` フィーチャー有効時に CUDA カーネルへ直接ワークロードを投げる FFT / IFFT / LDE のパリティテストを追加。デバイスが `GPU backend unavailable` を返した場合は安全にスキップするため、SM80 環境では GPU 出力を自動検証しつつ、CPU のみのホストでもテストはグリーンを維持します。【crates/fastpq_prover/src/fft.rs:858】【crates/fastpq_prover/src/fft.rs:919】【crates/fastpq_prover/src/fft.rs:1003】
- ✅ ベンチマーク: CPU と GPU を比較する Criterion スイート:
  - `crates/fastpq_prover/benches/planner.rs` が FFT / IFFT / LDE を対象に、正規の 2^13〜2^16 トレース（`fastpq-lane-balanced` と `fastpq-lane-latency`）で計測し、アクセラレータが無いホストは `gpu_fallback` としてラベル付けしてドキュメントへ取り込みます。
  - カーネル失敗や非対応時も決定的に CPU 経路へフォールバックする分割ロジックを再利用し、計測値は CPU/GPU 間で一致します。【crates/fastpq_prover/src/fft.rs:132】
- ランタイム検出とディスパッチ:
  - `fastpq_prover::config::ExecutionMode { Cpu, Gpu, Auto }` を導入。
  - GPU が混雑している場合のスケジューラ分割（任意）。
- ✅ Poseidon スポンジの GPU 対応: `PoseidonBackend` が CUDA 実装（`fastpq_cuda::fastpq_poseidon_permute`）を呼び出し、カーネル未対応やエラー時は警告を一度だけ発しつつ決定的な CPU 経路へ自動フォールバック。
- Poseidon バッチハッシュの GPU 実装と `PoseidonBackend` トレイトによる切り替え。

### ステージ 6 — プロダクション切り替え *(進行中)*
- 本番切替:
  - ✅ 決定的モックを削除し、`Prover` の標準コンストラクタが常に本番バックエンドを初期化するよう統合（CPU/GPU 切替は `zk.fastpq.execution_mode` または `irohad --fastpq-execution-mode` で制御）。
  - ✅ 20,000 行トレースで <1,000 ms となるよう性能閾値を更新（ベースライン HW を明記）。ナイトリーの回帰テストは 20,000 件の合成行を 1,000 ms 以内で処理し、基準ハードウェア構成 (CPU: AMD EPYC 7B12 32 コア / 256 GiB RAM、GPU: NVIDIA A100 40 GB / CUDA 12.2) を記録します。【crates/fastpq_prover/tests/perf_production.rs:1】【.github/workflows/fastpq-production-perf.yml:19】
- CI:
  - ✅ `FASTPQ_PROOF_ROWS=20000` を使ったリリースビルド性能テスト（ナイトリー、`.github/workflows/fastpq-production-perf.yml`）。
  - メトリクスアーティファクトを収集し公開。
    - ✅ ナイトリーの性能ログアーティファクト（`fastpq-production-perf-log`）をアップロードし、ワークフローサマリに計測結果を追記。
    - ✅ `scripts/fastpq/src/bin/publish_perf.rs` で NDJSON テレメトリを生成し、ナイトリーのワークフローでログとともにアップロードしてパイプラインへ取り込む。【scripts/fastpq/src/bin/publish_perf.rs:1】【.github/workflows/fastpq-production-perf.yml:61】
- ドキュメント／運用:
  - ✅ `status.md`, `docs/source/nexus.md`, `docs/source/zk/prover_runbook.md`, `docs/source/references/configuration.md` を更新し、本番バックエンドと実行モードのテレメトリを案内。
  - ✅ `docs/source/fastpq_migration_guide.md` を公開し、ビルドフラグ／ハードウェア要件／トラブルシュート／フォールバック手順を整理。
- ハードニング:
  - ✅ ツールチェーン・コンテナを固定した再現ビルド (`scripts/fastpq/repro_build.sh` とコンテナ定義)。【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.cpu:1】【scripts/fastpq/docker/Dockerfile.gpu:1】
  - トレース／SMT／ルックアップ構造のファジング。
  - x86_64 と ARM64 でのクロスアーキテクチャ再実行テストを CI に組み込む。

### ステージ 7 — ハードニングとロールアウト *(未着手)*
- テスト拡充:
  - ランダム多項式を用いた FFT/FRI の性質テストで決定性を確認。
  - ドメインルート改変、トランスクリプト改ざん、β/γ 不整合を含む否定テストを実施。
- 統合:
  - 新プルーバーで IVM/Torii の E2E（ガバナンス投票、送金フロー）を実行。
  - 小規模トレースで旧プレースホルダーとの結果（コミットメント、トランスクリプト）一致を確認。
- 移行ガイド:
  - フィーチャーフラグ、設定、要件ハードウェア、トラブルシュート、フォールバック手順をまとめたドキュメントを提供。
  - フォールバック再有効化手順を掲載（暫定措置）。
- ロールアウト完了時に `status.md` へ要約と性能データを追記。
- Alertmanager に `FastpqCpuFallbackBurst` アラートを追加し、`auto|gpu` 要求のうち CPU バックエンドで解決された割合が 10 分以上にわたって 5% を超えた場合にページングする。発火したら `fastpq_execution_mode_total` を確認し、GPU 証跡を取り直してステージ 7 の証跡バンドルへ添付する。
- さらに `FastpqQueueDutyCycleDrop` を SLO セットに加え、`fastpq_metal_queue_ratio{metric="busy"}` の 15 分移動平均が 50% を下回っているにもかかわらず GPU ワークロードが投入され続けている場合に即座に警告するようにした。これで本番テレメトリとベンチマーク証跡の間で Metal キュー占有率のターゲットを常に整合させたまま、GPU をデフォルト化する前に劣化を検知できる。【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】

### ステージ7-1 デバイスラベルとアラート契約

`scripts/fastpq/wrap_benchmark.py` は macOS の `system_profiler` を呼び出し、各ラップ済み JSON にハードウェアラベルを埋め込むことで、フリートテレメトリとキャプチャマトリクスが同じ名前空間を共有できるようにしている。20 000 行の Metal キャプチャには次のような `labels` ブロックが含まれる。

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

Linux 環境でも同じスクリプトが `/proc/cpuinfo`、`nvidia-smi`/`rocm-smi`、`lspci` を読み取り、`cpu_model`、`gpu_model`、`device_class`（例: `xeon-rtx-sm80`, `neoverse-mi300`）を正規化して書き込む。`--label` による手動上書きは残しているが、ステージ 7 の証跡バンドルはこの自動検出結果に依存するため、ラベル欠落は CI のチェックで即座にブロックされる。

ランタイムでも `fastpq.{device_class,chip_family,gpu_kind}` もしくは `FASTPQ_DEVICE_CLASS/FASTPQ_CHIP_FAMILY/FASTPQ_GPU_KIND` 環境変数で同じラベルを設定し、Prometheus の `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` や `fastpq_metal_queue_*{...}` がキャプチャファイルと同じ識別子を使うようにする。これにより `dashboards/grafana/fastpq_acceleration.json` のテンプレート変数と `dashboards/alerts/fastpq_acceleration_rules.yml` のクエリが 1:1 で対応する。【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

以下の SLO / アラート表は各シグナルがステージ 7 のゲートにどう結び付くかを示す。

| 信号 | ソース | 目標 / トリガー | エンフォース |
|------|--------|-----------------|---------------|
| GPU 採用率 | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", backend="metal"}` | 各クラスで ≥95% を `resolved="gpu"` に保つ。15 分平均で 50% を割ったらページング | `FastpqMetalDowngrade`（ページ）【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| バックエンドギャップ | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…"}` | 全クラスで常に 0。10 分を超えるバーストで警告 | `FastpqBackendNoneBurst`（ワーニング）【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| CPU フォールバック率 | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…"}` | GPU リクエストの ≤5% だけが CPU に落ちること。10 分以上 5% を超えたらページング | `FastpqCpuFallbackBurst`（ページ）【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| Metal キューのデューティサイクル | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…"}` | GPU ワークロードが投入されている間は 15 分移動平均を常に ≥50% に保つ。稼働率が目標を割り込んだままリクエストが継続したら警告 | `FastpqQueueDutyCycleDrop`（ワーニング）【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| キュー深度と zero-fill 予算 | ラップ済み JSON の `metal_dispatch_queue` / `zero_fill_hotspots` ブロック | `max_in_flight` は常に `limit` の 1 スロット下、LDE zero-fill 平均は 0.40 ms 以下。逸脱したバンドルは却下 | `scripts/fastpq/wrap_benchmark.py` の出力と `docs/source/fastpq_rollout_playbook.md` のレビューチェックで確認 |
| ランタイムのキューヘッドルーム | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…"}` | `limit - max_in_flight ≥ 1` を維持。10 分連続で頭打ちなら警告 | `FastpqQueueHeadroomLow`（ワーニング）【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| ランタイムの zero-fill レイテンシ | Prometheus `fastpq_zero_fill_duration_ms{device_class="…"}` | 最新サンプルを ≤0.40 ms（ステージ 7 予算）に収める | `FastpqZeroFillRegression`（ページ）【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |

#### Stage7-1 アラート対応チェックリスト

これらのアラートはすべて、ロールアウト証跡と本番メトリクスを同期させるための
運用タスクに直結する。

1. **`FastpqQueueHeadroomLow`（Warning）。** 
   `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` を
   即座に実行し、Grafana `fastpq-acceleration` ボードの “Queue headroom” パネルを
   キャプチャする。結果は
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom` に保存し、
   どのアラート ID かを明記する。【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression`（Pager）。**
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` を確認し、値が安定しない場合は
   直近のベンチ JSON を `scripts/fastpq/wrap_benchmark.py` で再ラップして
   `zero_fill_hotspots` を更新する。promQL の出力、スクリーンショット、更新後の
   ファイルをロールアウトディレクトリに添付する。これらが
   `ci/check_fastpq_rollout.sh` が期待する証跡になる。【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst`（Pager）。**
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` が 5% を超えていることを
   確認し、`telemetry::fastpq.execution_mode resolved="cpu"` ログを抜き出して
   `metrics_cpu_fallback.prom`／`rollback_drill.log` にまとめる。
4. **証跡のまとめ。** アラートが消えたら
   `docs/source/fastpq_rollout_playbook.md` のステージ 7-3 手順（Grafana エクスポート、
   アラートスナップショット、ロールバックドリル）をやり直し、
   `ci/check_fastpq_rollout.sh` でバンドルを再検証する。【docs/source/fastpq_rollout_playbook.md:114】

手動作業を減らしたい場合は
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
を実行すると、Prometheus API からキューヘッドルーム／zero-fill／CPU フォールバックの
各メトリクスを取得し、`metrics_headroom.prom`・`metrics_zero_fill.prom`・
`metrics_cpu_fallback.prom` を指定フォルダにまとめて保存できる。

`ci/check_fastpq_rollout.sh` はこれらの条件を CI 上で再検証する。`fastpq_bench_manifest.json` に含まれる `metal` キャプチャごとに `benchmarks.metal_dispatch_queue.{limit,max_in_flight}` と `benchmarks.zero_fill_hotspots[]` を読み出し、ヘッドルームが 1 未満になったり `mean_ms > 0.40` を検出したりした時点でバンドルを失格にする。同じパスで `metadata.labels.device_class` と `metadata.labels.gpu_kind` の存在も必須とし、欠落していればその場で失敗させる。【ci/check_fastpq_rollout.sh#L1】

Grafana の “Latest Benchmark” パネルと `docs/source/fastpq_rollout_playbook.md` に記載されたエビデンス手順も `device_class`、zero-fill 予算、キュー深度スナップショットを引用し、オンコールが本番メトリクスと証跡バンドルを突き合わせられるようにしている。

### ステージ7-1 フリートテレメトリ運用手順

GPU レーンをデフォルト化する前に次の手順を踏み、リリース時の証跡と本番テレメトリを整合させる。

1. **キャプチャ／ランタイム双方でラベルを統一する。** `python3 scripts/fastpq/wrap_benchmark.py` は `metadata.labels.device_class/chip_family/gpu_kind` を自動で付与する。`iroha_config` 側では `fastpq.{device_class,chip_family,gpu_kind}` もしくは `FASTPQ_DEVICE_CLASS/FASTPQ_CHIP_FAMILY/FASTPQ_GPU_KIND=<matrix-label>` を同じ値に合わせ、`artifacts/fastpq_benchmarks/matrix/devices/*.txt` に並ぶクラスと一致させる。新しいクラスを追加したら `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices` でマニフェストを再生成し、CI とダッシュボードの両方に知らせる。
2. **キューゲージと採用率を確認する。** Metal ホスト上で `irohad --features fastpq-gpu` を起動し、メトリクスエンドポイントを確認する。

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```

   1 行目はセマフォサンプラーが `busy` / `overlap` / `limit` / `max_in_flight` を公開しているかを示し、2 行目は各クラスが `backend="metal"` で解決しているか、`backend="cpu"` へ落ちているかを示す。これらを Prometheus/OTel に登録してからダッシュボードをインポートすると、Grafana で即座にフリートビューを描画できる。
3. **ダッシュボードとアラートパックを配置する。** `dashboards/grafana/fastpq_acceleration.json` を Grafana に取り込み（デバイスクラス／チップファミリ／GPU 種別のテンプレート変数は保持）、`dashboards/alerts/fastpq_acceleration_rules.yml` を Alertmanager に読み込む。付属の `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` で `FastpqMetalDowngrade` と `FastpqBackendNoneBurst` が定義どおり発火することを確認する。
4. **リリースをエビデンスバンドルでガードする。** `docs/source/fastpq_rollout_playbook.md` を参照しつつ、ラップ済みベンチマーク、Grafana エクスポート、アラートパック、キュー証跡、ロールバックログを 1 つのバンドルにまとめる。`make check-fastpq-rollout`（または `ci/check_fastpq_rollout.sh --bundle <path>`）が自動で検証を走らせ、ヘッドルームや zero-fill 予算が退行した時点で承認を拒否する。
5. **アラートを是正措置へ結び付ける。** Alertmanager がページしたら Grafana パネルと手順 2 の生メトリクスを照らし合わせ、キュー飢餓か CPU フォールバックか `backend="none"` バーストかを切り分ける。本ドキュメントと `docs/source/fastpq_rollout_playbook.md` に記載されたランブックへ従い、該当する `fastpq_execution_mode_total`、`fastpq_metal_queue_ratio`、`fastpq_metal_queue_depth` の抜粋と Grafana／アラートのリンクをリリースチケットへ添付して SLO 侵害を明示する。

### ステージ7 FFT キュー ファンアウト

`crates/fastpq_prover/src/metal.rs` の Metal ホストは `QueuePolicy` を導入し、
デバイスがディスクリート GPU だと判定された場合に複数のコマンドキューを
自動生成するようになった。統合 GPU では従来どおり 1 本のキューを使い、
ディスクリート GPU ではデフォルトで 2 本を確保し、列数が 16 本を超えたとき
だけファンアウトを有効化する。`FASTPQ_METAL_QUEUE_FANOUT` と
`FASTPQ_METAL_COLUMN_THRESHOLD` で両ヒューリスティックを上書きでき、FFT/LDE
の各バッチはラウンドロビンでキューに割り当てられ、ポストタイルのディスパッチ
も同じキュー上で実行されるため依存関係が崩れない。【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】

ヘルパーテストはキューポリシーとパーサのバリデーションを網羅し、GPU を持たない
CI でもガードを実行できるようにした。また GPU 依存の回帰テストは上記の
オーバーライドを強制した状態で走らせることで、新しいデフォルトをラボ環境でも
再現できる。【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

## サウンドネス指標・SLO
| `N_trace` | blowup | FRI arity | 層数 | クエリ数 | 推定ビット | 証明サイズ (≤) | RAM (≤) | P95 レイテンシ (≤) |
|-----------|--------|-----------|------|----------|-------------|------------------|---------|--------------------|
| 2^15 | 8 | 8 | 5 | 52 | 約 190 | 300 KB | 1.5 GB | 0.40 s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | 約 132 | 420 KB | 2.5 GB | 0.75 s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | 約 142 | 550 KB | 3.5 GB | 1.20 s (A100) |

導出は付録 A を参照。CI は推定ビットが 128 未満になった場合に失敗します。

## Public IO スキーマ
| フィールド | バイト数 | エンコーディング | 備考 |
|------------|----------|--------------------|------|
| `dsid` | 16 | UUID (LE) | エントリが属するレーンのデータスペース ID（デフォルトレーンは GLOBAL）。ドメインタグ `fastpq:v1:dsid`。 |
| `slot` | 8 | u64 (LE) | エポック（ナノ秒）。 |
| `old_root` | 32 | Poseidon2 (LE) | バッチ前の SMT ルート。 |
| `new_root` | 32 | Poseidon2 (LE) | バッチ後の SMT ルート。 |
| `perm_root` | 32 | Poseidon2 (LE) | 権限テーブルのルート。 |
| `tx_set_hash` | 32 | BLAKE2b | ソート済みトランザクション ID。 |
| `parameter` | 可変 | UTF-8 | パラメータ名（例: `fastpq-lane-balanced`）。 |
| `protocol_version`, `params_version` | 各 2 | u16 (LE) | バージョン番号。 |
| `ordering_hash` | 32 | Poseidon2 (LE) | 並び順ハッシュ。 |

削除は値リムをゼロ化。未存在キーはゼロ葉 + `neighbour_leaf` 証明で表す。

`FastpqTransitionBatch.public_inputs` が `dsid` / `slot` / ルートコミットメントの正規キャリアであり、metadata は entry hash / transcript count の管理に限定される。

## ハッシュ定義
- 並び順: Poseidon2（タグ `fastpq:v1:ordering`）。
- アーティファクト: `PublicIO || proof.commitments` を BLAKE2b（タグ `fastpq:v1:artifact`）。

## ステージ別 DoD
- **ステージ 0 DoD**
  - 新フィールドが全正準パラメータに設定済み。
  - 導出スクリプトとコミットされた定数が一致する CI フィクスチャが存在。
  - ドキュメント（付録 B、ロードマップ、`status.md`）を同時更新。
- **ステージ 1 DoD**
  - 列対応プランナーとユニットテスト／ベンチマークが統合。
  - FFT/IFFT 往復テストで決定性を確認。ドメイン選択ログはトレース機能で制御。
  - `fastpq-prover-preview` フィーチャーで新プランナーを切り替え可能。
- **ステージ 2 DoD**
  - ストリーミング Poseidon コミットメントとプレースホルダー互換フィクスチャが揃う。
  - ルックアップ証人生成が LDE 値に基づいて機能し、作業例を更新。
  - トランスクリプトのドメインタグを付録 C に記録し、CI ゴールデンを更新。
- **ステージ 3 DoD**
  - DEEP-FRI 折り畳みとトランスクリプト操作がテスト付きで実装。
  - メルクルパスのバッチ生成と Winterfell との相互検証を完了。
  - 折り畳みラウンドのテレメトリスパンを SRE 用に追加。
- **ステージ 4 DoD**
  - クエリサンプラーがアーキテクチャ間で決定的一致。ベリファイアがプルーバー出力と一致。
  - オープニング改変や β/γ 不整合を検知する否定テストを追加。
  - `nexus.md` の図表を更新。
- **ステージ 5 DoD**
  - GPU カーネルが CPU と完全一致し、決定的フォールバックが確認済み。
  - 実行モード設定を CLI／設定で公開し、性能データを文書化。
  - ナイトリー CI で GPU ベンチマークを実行（ハードウェアが利用可能な場合）。
- **ステージ 6 DoD**
  - プレースホルダーをフィーチャーフラグの背後のみに縮退。
  - 性能ハーネスと移行ガイド、運用ランブックを公開。
  - フォールバック・リプレイを含むクロスアーキテクチャテストが 2 回連続で成功。
- **ステージ 7 DoD**
  - FFT/FRI・トランスクリプト・ルックアップの包括的テスト（正／負）が揃い決定性を確認。
  - 新プルーバーで IVM/Torii E2E が通過し、旧プレースホルダーとの比較結果を文書化。
  - 展開チェックリスト、フォールバック検証、`status.md`/`roadmap.md` の更新を完了。

さらに、SLO セットには `FastpqQueueDutyCycleDrop` ルールが追加され、`fastpq_metal_queue_ratio{metric="busy"}` の 15 分移動平均が 50% を下回った Metal キューを即座に警告することで、GPU テレメトリ契約をベンチマーク証跡と整合させたまま強制できます。

---

## 批評サマリーと未解決アクション

### 強み
- 段階的なステージ設計で部分的なロールアウトが容易。
- 決定的コミットメントと順序仕様が端から端まで文書化されている。
- DoD 基準が明確で、ロードマップ・計画・実装の整合性が保たれている。

### 優先アクション
1. ✅ ステージ 1 の列対応 FFT プランナーと Criterion ベンチマークを実装済み。性能回帰の監視を継続。
2. プレビュー プランナーをナイトリー CI (`fastpq-prover-preview`) に組み込み、ステージ 2 ロールアウト前に健全性を確認。
3. 付録 C のコミットメント パイプラインに合わせた互換性テストを整備し、プレースホルダー／プレビュー間の差分を洗い出す。

### 決着済みの設計判断
- P1 時点では ZK を無効（正当性保証のみ）。将来ステージで再検討。
－ 権限テーブルのルートはガバナンス状態から導出され、ルックアップ証明で参照。  
- 欠落キー証明はゼロ葉 + `neighbour_leaf` で同一符号化。  
- 削除は正規化キー空間内で値をゼロに設定する方式とする。

## 付録 A — サウンドネス導出

本付録では「サウンドネス・SLO」表の算出手順と、CI が用いるリジェクションサンプリングによる裏付け方法を説明する。

### 記法
- `N_trace = 2^{k}` — 並べ替えとゼロ詰め後のトレース長（2 の累乗）。
- `b` — ブローアップ係数（`N_eval = N_trace × b`）。
- `r` — FRI の折り畳み数（正準セットでは 8 または 16）。
- `ell` — FRI のリダクション段数（表の `layers` 列）。
- `q` — 1 証明あたり検証者が発行するクエリ数（表の `queries` 列）。
- `rho` — 各 FRI 層の開始時点における合成多項式の実効コードレート。プランナーが追跡している行／列制約について `max_i (degree_i / domain_i)` を取り、最悪ケースの次数とドメイン幅の比として算出する。

Goldilocks 体の位数は `|F| = 2^64 - 2^32 + 1` であり、Fiat–Shamir の衝突確率を上界化する際は `log2 |F| ≈ 64` と近似する。

### 解析的境界
定率の DEEP-FRI では統計的失敗確率は

```
p_fri  ≤  Σ_{j=0}^{ell-1} rho^{q} = ell * rho^{q}
```

に抑えられる。プランナーは各層で次数とドメインを同じ比率で縮小するため、`rho` は層をまたいで一定となる。Fiat–Shamir の衝突確率は高々 `q / 2^{64}`、グラインディングは独立に `2^{-g}`（正準セットでは `g ∈ {21, 23}`）を付加する。表の `est bits` には FRI 項 `-log2 p_fri` を記録し、Fiat–Shamir／グラインディング分は追加の安全余裕として扱う。

### プランナー出力と行単位の計算
Stage 1 の列プランナーを代表的なバッチで動作させると次の実効コードレートが得られる。次数は最初の FRI ラウンドの直前に観測した最大合成多項式次数である。

| パラメータセット | `N_trace` | `b` | `N_eval` | `rho`（プランナー） | 実効次数 (`rho * N_eval`) | `ell` | `q` | `-log2(ell * rho^{q})` |
|------------------|-----------|-----|----------|----------------------|---------------------------|-------|-----|------------------------|
| 20k バッチ（バランス） | `2^15` | 8 | 262144 | 0.077026 | 20192  | 5 | 52 | 190 ビット |
| 65k バッチ（スループット） | `2^16` | 8 | 524288 | 0.200208 | 104967 | 6 | 58 | 132 ビット |
| 131k バッチ（レイテンシ） | `2^17` | 16 | 2097152 | 0.209492 | 439337 | 5 | 64 | 142 ビット |

例（20k バッチ）:
1. `N_trace = 2^15` なので `N_eval = 2^15 · 8 = 2^18`。
2. プランナーは `rho = 0.077026` と報告するため `p_fri = 5 × rho^{52} ≈ 6.4e-58`。
3. `-log2 p_fri = 190` ビットとなり、表の値と一致する。
4. Fiat–Shamir の衝突は `≈ 2^{-58.3}`、グラインディング（`g = 23`）は確率をさらに `2^{-23}` だけ押し下げるため、総サウンドネスは公開値よりも 160 ビット超の余裕が残る。

### CI によるリジェクションサンプリング
すべての CI ランでは以下のモンテカルロハーネスを実行して境界を実測する。
1. 正準パラメータセットをランダムに選び、目標トレース長に合わせた `TransitionBatch` を合成する。
2. トレースを生成し、ランダムな制約（例: グランドプロダクト寄与の反転や SMT 兄弟ノードの不整合）を破壊したうえで証明生成を試みる。
3. 検証器を再実行し、Fiat–Shamir（グラインディングを含む）を再サンプリングして改ざんされた証明が検査を通過するかを記録する。
4. 各パラメータセットについて 16,384 シード分繰り返し、Clopper–Pearson の 99% 下限から観測拒否率をビット数に変換する。

実測値は表の解析値と ±0.6 ビットの範囲で一致する。CI ジョブは下限が 128 ビットを下回ると失敗するため、列プランナーや FRI 配線の退行はマージ前に検出される。

## 付録 B — ドメインルート導出

正準サブグループの生成子は Poseidon2 パラメータから決定的に導出され、すべての実装で同じ結果が得られる。

### 手順
1. **シード選択:** ドメインタグ `fastpq:v1:domain_roots` を FASTPQ と同じ Poseidon2 スポンジ（幅 3、レート 2、フルラウンド 4、パーシャル 57）に吸収。入力は `fastpq_prover::pack_bytes` と同じ `[len, limbs…]` 形式。得られたフィールド要素を基底生成子 `g_base` とする（現行シードでは `g_base = 7`）。
2. **トレース生成子:** `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` を計算し、`trace_root^{2^{trace_log_size}} = 1` かつ半分の次数で 1 にならないことを確認。
3. **LDE 生成子:** `lde_log_size` を用いて同様に `lde_root` を導出。
4. **コセット選択:** ステージ 0 は基礎サブグループを使用。将来的にコセットが必要な場合は `fastpq:v1:domain_roots:coset` のようなタグを追加吸収して派生させる。
5. **Permutation サイズ:** `permutation_size` を明示的に保持し、スケジューラが特定のパディング規約に依存しないようにする。

### 再現と検証
- ツール: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots`  
  オプションで `--format table` や `--seed`、`--filter` を指定可能。
- テスト: `fastpq_isi::roots_match_declared_domain` と `canonical_sets_meet_security_target` が常にテーブルとの一致を確認。

### 参照値
ステージ 0 表内の `trace_root`, `lde_root`, `omega_coset` が現在の正準値。追加パラメータを導入する際はこの表と `fastpq_isi/src/params.rs` を同時更新する。

## 付録 C — コミットメントパイプライン詳細

### ストリーミング Poseidon コミットメントフロー
ステージ 2 ではプルーバーとベリファイアが共有する決定的トレースコミットメントを次の手順で構築する。
1. **トランジションの正規化:** `trace::build_trace` がバッチをソートし、`N_trace = 2^{⌈log₂ rows⌉}` までパディングしたうえで後述の順序で列ベクトルを生成する。
2. **列ハッシュ:** `trace::column_hashes` が列を順次走査し、`fastpq:v1:trace:column:<name>` でシードされた専用 Poseidon2 スポンジに吸収する。`fastpq-prover-preview` フィーチャが有効な場合は、バックエンドが必要とする IFFT/LDE 係数を再利用し、追加の行列コピーを確保せずにストリーミングでハッシュを行う。
3. **Merkle 畳み込み:** `trace::merkle_root` が列ハッシュを Poseidon Merkle 木に持ち上げ、内部ノードで `fastpq:v1:trace:node` を使用する。各レベルの葉数が奇数の場合は最後の葉を複製してペアリングする。
4. **最終ダイジェスト:** `digest::trace_commitment` がドメイン（`fastpq:v1:trace_commitment`）、パラメータ名、パディング後の寸法、列ダイジェスト、Merkle ルートを長さ付きで連結し、`Hash::new`（SHA3-256）でハッシュした値を `Proof::trace_commitment` に埋め込む。

ベリファイアはクエリを開く前に同じダイジェストを再計算し、一致しなければ証明を即座に拒否する。

### 列の順序と名称
列はハッシュパイプラインが消費する順番で出力される。
1. セレクタフラグ: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`。
2. パック済みリム列（列ごとにゼロ埋め）:
   - `key_limb_{i}`,
   - `value_old_limb_{i}`,
   - `value_new_limb_{i}`,
   - `asset_id_limb_{i}`。
3. 補助スカラー: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `dsid`, `slot`。
4. 各レベル `ℓ ∈ [0, 31]` の疎な Merkle 証明列: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`。

`column_hashes` が計算するコミットメントは常にこの列順をハッシュするため、プレースホルダーバックエンドとプレビュー版 STARK 実装のあいだでトレース互換性が保たれる。

### トランスクリプトのドメインタグ
ステージ 2 では Fiat–Shamir トランスクリプトのタグを次のように固定し、検証側の決定性を保証する。

| タグ | 用途 |
|------|------|
| `fastpq:v1:init` | プロトコルバージョン、パラメータセット、`PublicIO` を初期吸収する。 |
| `fastpq:v1:roots` | トレースの Merkle ルートとルックアップ LDE ルートをコミットする。 |
| `fastpq:v1:gamma` | ルックアップのグランドプロダクトチャレンジをサンプリングする。 |
| `fastpq:v1:alpha:<i>` | 合成多項式用チャレンジ（`i = 0,1`）をサンプリングする。 |
| `fastpq:v1:lookup:product` | 算出済みグランドプロダクトの評価値を吸収する。 |
| `fastpq:v1:beta:<round>` | 各 FRI リダクションラウンドでの折り畳みチャレンジをサンプリングする。 |
| `fastpq:v1:fri_layer:<round>` | 各 FRI 層の Merkle ルートをコミットする。 |
| `fastpq:v1:fri:final` | クエリオープン前の最終 FRI ルートを記録する。 |
| `fastpq:v1:query_index:0` | 検証者のクエリインデックスを決定的に導出する。 |

これらのタグと前述の列ハッシュ用ドメインの組み合わせが、ステージ 2 で参照される「ドメインタグカタログ」である。文字列や順序を変更すると Fiat–Shamir チャレンジが変化し、後方互換性が失われる点に注意すること。
