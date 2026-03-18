---
lang: ja
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2026-01-03T18:07:57.107521+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## SM パフォーマンスの把握とベースライン計画

ステータス: ドラフト中 — 2025-05-18  
所有者: パフォーマンス WG (リーダー)、インフラ運用 (ラボのスケジュール設定)、QA ギルド (CI ゲート)  
関連ロードマップ タスク: SM-4c.1a/b、SM-5a.3b、FASTPQ ステージ 7 クロスデバイス キャプチャ

### 1. 目的
1. Neoverse 中央値を `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json` に記録します。現在のベースラインは、`artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (CPU ラベル `neoverse-proxy-macos`) の下の `neoverse-proxy-macos` キャプチャからエクスポートされ、SM3 比較許容値は aarch64 macOS/Linux の 0.70 に拡大されます。ベアメタル時間が開いたら、Neoverse ホストで `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` を再実行し、集計された中央値をベースラインに昇格させます。  
2. `ci/check_sm_perf.sh` が両方のホスト クラスを保護できるように、一致する x86_64 中央値を収集します。  
3. 将来のパフォーマンス ゲートが部族の知識に依存しないように、反復可能なキャプチャ手順 (コマンド、アーティファクト レイアウト、レビュー担当者) を公開します。

### 2. ハードウェアの可用性
現在のワークスペースで到達できるのは、Apple Silicon (macOS arm64) ホストのみです。 `neoverse-proxy-macos` キャプチャは暫定 Linux ベースラインとしてエクスポートされますが、ベアメタル Neoverse または x86_64 メディアンをキャプチャするには、`INFRA-2751` で追跡される共有ラボ ハードウェアが必要であり、ラボ ウィンドウが開いたらパフォーマンス WG によって実行されます。残りのキャプチャ ウィンドウはアーティファクト ツリーで予約および追跡されます。

- Neoverse N2 ベアメタル (東京ラック B) は 2026 年 3 月 12 日に予約されました。オペレーターはセクション 3 のコマンドを再利用し、アーティファクトを `artifacts/sm_perf/2026-03-lab/neoverse-b01/` の下に保存します。
- x86_64 Xeon (チューリッヒ ラック D) は、ノイズを低減するために SMT が無効になっており、2026 年 3 月 19 日に予約されました。アーティファクトは `artifacts/sm_perf/2026-03-lab/xeon-d01/` の下に配置されます。
- 両方の実行が成功したら、中央値をベースライン JSON にプロモートし、`ci/check_sm_perf.sh` で CI ゲートを有効にします (ターゲット切り替え日: 2026-03-25)。

これらの日付までは、macOS arm64 ベースラインのみをローカルで更新できます。### 3. キャプチャ手順
1. **ツールチェーンの同期**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **キャプチャ マトリックスの生成** (ホストごと)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   ヘルパーはターゲット ディレクトリに `capture_commands.sh` および `capture_plan.json` を書き込みます。このスクリプトはモードごとに `raw/*.json` キャプチャ パスを設定するため、ラボ技術者は決定的に実行をバッチ処理できます。
3. **キャプチャの実行**  
   `capture_commands.sh` から各コマンドを実行し (または同等のコマンドを手動で実行し)、すべてのモードが `--capture-json` 経由で構造化された JSON BLOB を発行するようにします。キャプチャ メタデータと後続のベースラインが中央値を生成した正確なハードウェアを記録できるように、常に `--cpu-label "<model/bin>"` (または `SM_PERF_CPU_LABEL=<label>`) 経由でホスト ラベルを指定します。ヘルパーはすでに適切なパスを提供しています。手動実行の場合、パターンは次のとおりです。
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **結果を検証する**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   実行間の差異が ±3% 以内に収まるようにします。そうでない場合は、影響を受けるモードを再実行し、ログに再試行を記録します。
5. **中央値を促進**  
   `scripts/sm_perf_aggregate.py` を使用して中央値を計算し、ベースライン JSON ファイルにコピーします。
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   ヘルパー グループは `metadata.mode` によってキャプチャし、各セットが
   同じ `{target_arch, target_os}` トリプルで、1 つのエントリを含む JSON 概要を出力します
   モードごとに。ベースライン ファイルに含まれる中央値は以下に存在します。
   `modes.<mode>.benchmarks`、付随する `statistics` ブロック レコード
   レビュー担当者と CI の完全なサンプル リスト、最小/最大、平均、および母集団の標準偏差。
   集約されたファイルが存在すると、ベースライン JSON を自動で書き込むことができます (
   標準公差マップ) 以下を介して:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   `--mode` をオーバーライドしてサブセットに制限するか、`--cpu-label` をオーバーライドして固定します
   集約ソースで CPU 名が省略されている場合に記録されます。
   アーキテクチャごとの両方のホストが完了したら、以下を更新します。
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (新規)

   `aarch64_unknown_linux_gnu_*` ファイルには `m3-pro-native` が反映されるようになりました。
   キャプチャ (CPU ラベルとメタデータのメモは保持される) なので、`scripts/sm_perf.sh` は
   手動フラグなしで aarch64-unknown-linux-gnu ホストを自動検出します。とき
   ベアメタル ラボの実行が完了したら、`scripts/sm_perf.sh --mode  を再実行します。
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   新しいキャプチャを使用して暫定中央値を上書きし、実際の中央値をスタンプします。
   ホストラベル。

   > 参考: 2025 年 7 月の Apple Silicon キャプチャ (CPU ラベル `m3-pro-local`) は
   > `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}` の下にアーカイブされています。
   > Neoverse/x86 アーティファクトを公開するときにそのレイアウトをミラーリングして、レビュー担当者が
   > 生の出力と集計された出力を一貫して比較できます。

### 4. アーティファクトのレイアウトと承認
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` は、コマンド ハッシュ、git リビジョン、オペレーター、および異常を記録します。
- 集約された JSON ファイルはベースライン更新に直接フィードされ、`docs/source/crypto/sm_perf_baseline_comparison.md` のパフォーマンス レビューに添付されます。
- QA ギルドは、ベースラインが変更される前にアーティファクトをレビューし、パフォーマンス セクションの `status.md` でサインオフします。### 5. CI ゲートのタイムライン
|日付 |マイルストーン |アクション |
|------|-----------|----------|
| 2025-07-12 |ネオバース攻略完了 | `sm_perf_baseline_aarch64_*` JSON ファイルを更新し、`ci/check_sm_perf.sh` をローカルで実行し、アーティファクトが添付された PR を開きます。 |
| 2025-07-24 | x86_64 キャプチャが完了しました | `ci/check_sm_perf.sh` に新しいベースライン ファイルとゲートを追加します。クロスアーチ CI レーンがそれらを確実に消費するようにします。 |
| 2025-07-27 | CI の施行 | `sm-perf-gate` ワークフローを両方のホスト クラスで実行できるようにします。回帰が設定された許容値を超える場合、マージは失敗します。 |

### 6. 依存関係とコミュニケーション
- `infra-ops@iroha.tech` 経由でラボへのアクセス変更を調整します。  
- パフォーマンス WG は、キャプチャの実行中に `#perf-lab` チャネルに毎日の更新情報を投稿します。  
- QA ギルドは、レビュー担当者がデルタを視覚化できるように比較差分 (`scripts/sm_perf_compare.py`) を準備します。  
- ベースラインが結合したら、`roadmap.md` (SM-4c.1a/b、SM-5a.3b) および `status.md` をキャプチャ完了メモで更新します。

この計画により、SM アクセラレーション作業により、再現可能な中央値、CI ゲート、および追跡可能な証拠追跡が得られ、「ラボ ウィンドウの予約と中央値のキャプチャ」アクション アイテムが満たされます。

### 7. CI ゲートとローカルスモーク

- `ci/check_sm_perf.sh` は正規の CI エントリポイントです。 `SM_PERF_MODES` (デフォルトは `scalar auto neon-force`) の各モードに対して `scripts/sm_perf.sh` にシェルアウトし、ベンチが CI イメージ上で確定的に実行されるように `CARGO_NET_OFFLINE=true` を設定します。  
- `.github/workflows/sm-neon-check.yml` は macOS arm64 ランナーのゲートを呼び出すようになり、すべてのプル リクエストがローカルで使用されるのと同じヘルパーを介してスカラー/オート/ネオンフォース トリオを実行します。 x86_64 キャプチャが確立され、Neoverse プロキシ ベースラインがベアメタル実行で更新されると、補完的な Linux/Neoverse レーンが接続されます。  
- オペレーターはモード リストをローカルでオーバーライドできます。`SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` は、迅速なスモーク テストのために実行を単一パスにトリミングし、追加の引数 (`--tolerance 0.20` など) は `scripts/sm_perf.sh` に直接転送されます。  
- `make check-sm-perf` は開発者の便宜のためにゲートをラップするようになりました。 CI ジョブは、macOS 開発者が make ターゲットに便乗しながら、スクリプトを直接呼び出すことができます。  
- Neoverse/x86_64 ベースラインが到着すると、同じスクリプトが `scripts/sm_perf.sh` にすでに存在するホスト自動検出ロジックを介して適切な JSON を取得するため、ホスト プールごとに必要なモード リストを設定する以外にワークフローで追加の配線は必要ありません。

### 8. 四半期ごとの更新ヘルパー- `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` を実行して、`artifacts/sm_perf/2026-Q1/<label>/` などの 4 分の 1 スタンプのディレクトリを作成します。ヘルパーは `scripts/sm_perf_capture_helper.sh --matrix` をラップし、`capture_commands.sh`、`capture_plan.json`、および `quarterly_plan.json` (所有者 + 四半期メタデータ) を出力するため、ラボ オペレーターは手書きの計画を行わずに実行をスケジュールできます。
- 生成された `capture_commands.sh` をターゲット ホスト上で実行し、生の出力を `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` で集約し、`scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite` を介して中央値をベースライン JSON にプロモートします。 `ci/check_sm_perf.sh` を再実行して、許容値が緑色のままであることを確認します。
- ハードウェアまたはツールチェーンが変更された場合は、`docs/source/crypto/sm_perf_baseline_comparison.md` の比較許容値/メモを更新し、新しい中央値が安定した場合は `ci/check_sm_perf.sh` 許容値を厳しくし、ダッシュボード/アラートのしきい値を新しいベースラインに合わせて、運用アラームが意味のある状態を維持できるようにします。
- `quarterly_plan.json`、`capture_plan.json`、`capture_commands.sh`、および集約された JSON をベースライン更新とともにコミットします。追跡可能性を確保するために、同じアーティファクトをステータス/ロードマップの更新に添付します。