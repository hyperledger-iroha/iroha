---
lang: ja
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee2fc3d436de10f6a05a0b73cf3f5a0e07bc1ef1c421203cfcbd09d8d0e039b
source_last_modified: "2025-11-20T04:33:21.759984+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/mochi/troubleshooting.md -->

# MOCHI トラブルシューティングガイド

ローカルの MOCHI クラスタが起動しない、再起動ループで詰まる、
またはブロック/イベント/ステータスのストリーム更新が止まった場合に
このランブックを使用する。`mochi-core` にある supervisor の挙動を
具体的な復旧手順に変換し、ロードマップ項目「Documentation & rollout」を
補完する。

## 1. 初動チェックリスト

1. MOCHI が使用している data root を把握する。デフォルトは
   `$TMPDIR/mochi/<profile-slug>`。カスタムパスは UI タイトルバーや
   `cargo run -p mochi-ui-egui -- --data-root ...` で確認できる。
2. workspace ルートで `./ci/check_mochi.sh` を実行する。core/UI/integration
   クレートを検証してから設定変更に着手する。
3. プリセット（`single-peer` または `four-peer-bft`）を記録する。
   生成されるトポロジにより data root 下で期待する peer フォルダ/ログ数が決まる。

## 2. ログとテレメトリの証拠収集

`NetworkPaths::ensure`（`mochi/mochi-core/src/config.rs`）は安定したレイアウトを作成する:

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

変更前に次を実行する:

- **Logs** タブを使うか `logs/<alias>.log` を直接開いて、各 peer の直近 200 行を取得する。
  supervisor は `PeerLogStream` で stdout/stderr/system を追跡するため、これらのログは
  UI 出力と一致する。
- **Maintenance → Export snapshot**（または `Supervisor::export_snapshot`）でスナップショットを
  エクスポートする。スナップショットは storage/config/log を
  `snapshots/<timestamp>-<label>/` にまとめる。
- ストリームウィジェットが関係する場合、Dashboard の `ManagedBlockStream`,
  `ManagedEventStream`, `ManagedStatusStream` のヘルス指標を取得する。
  UI は最後の再接続試行とエラー理由を表示するため、インシデント用に
  スクリーンショットを残す。

## 3. peer 起動問題の解消

多くの起動失敗は 3 つのカテゴリに分かれる:

### バイナリ不足または誤ったオーバーライド

`SupervisorBuilder` は `irohad`, `kagami`, （将来的に）`iroha_cli` をシェル実行する。
UI が「failed to spawn process」や「permission denied」を報告する場合は、
既知の正常バイナリを指定する:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

`MOCHI_IROHAD`, `MOCHI_KAGAMI`, `MOCHI_IROHA_CLI` を設定してフラグ入力を省略できる。
バンドルビルドのデバッグでは、`mochi/mochi-ui-egui/src/config/` の `BundleConfig` と
`target/mochi-bundle` のパスを比較する。

### ポートの衝突

`PortAllocator` はループバックを検査してから設定を書き込む。
`failed to allocate Torii port` または `failed to allocate P2P port` が出た場合は、
既に別プロセスが 8080/1337 を占有している。明示的なベースを指定して再起動する:

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

builder はベースから連番でポートを割り当てるため、プリセットの peer 数に合わせて
十分なレンジを確保する。

### Genesis/ストレージの破損

Kagami がマニフェスト出力前に終了すると peer は即時クラッシュする。
データルート内の `genesis/*.json`/`.toml` を確認し、
`--kagami /path/to/kagami` で再実行するか **Settings** ダイアログで正しいバイナリを指定する。
ストレージ破損の場合は、フォルダを手動削除するのではなく、
Maintenance の **Wipe & re-genesis** を使用する（後述）。peer ディレクトリと
snapshot ルートを再作成してからプロセスを再起動する。

### 自動再起動の調整

`config/local.toml` の `[supervisor.restart]`（または CLI フラグ `--restart-mode`,
`--restart-max`, `--restart-backoff-ms`）が再試行頻度を制御する。
`mode = "never"` にすると初回失敗を UI に即表示できる。CI で迅速に失敗させたい場合は、
`max_restarts`/`backoff_ms` を短くする。

## 4. 安全な peer リセット

1. Dashboard から該当 peer を停止するか、UI を終了する。
   supervisor は稼働中の peer がいる場合ストレージを消せない
   （`PeerHandle::wipe_storage` は `PeerStillRunning` を返す）。
2. **Maintenance → Wipe & re-genesis** を実行する。MOCHI は次を行う:
   - `peers/<alias>/storage` を削除;
   - Kagami を再実行し、`genesis/` 配下に config/genesis を再構築;
   - 保持している CLI/環境オーバーライドで peer を再起動。
3. 手動で行う場合:
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # 表示された root を確認してから:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   その後 MOCHI を再起動し、`NetworkPaths::ensure` がツリーを再作成するようにする。

ローカル開発であっても、ワイプ前に `snapshots/<timestamp>` を必ずアーカイブする。
これらのバンドルには再現に必要な `irohad` のログと設定が含まれる。

### 4.1 スナップショットから復元

実験でストレージが壊れたり、既知の正常状態に戻す必要がある場合は、
Maintenance ダイアログの **Restore snapshot**（または `Supervisor::restore_snapshot`）を
使い、手動コピーを避ける。絶対パスまたは `snapshots/` 配下のサニタイズ済みフォルダ名を
指定する。supervisor は次を実行する:

1. 稼働中の peer を停止;
2. スナップショットの `metadata.json` が現在の `chain_id` と peer 数に一致するか検証;
3. `peers/<alias>/{storage,snapshot,config.toml,latest.log}` をアクティブプロファイルへ復元;
4. 必要に応じて `genesis/genesis.json` を復元してから peer を再起動。

スナップショットが異なるプリセット/チェーン ID 向けの場合、
`SupervisorError::Config` が返るため、異なるアーティファクトを混在させずに
適合するバンドルを取得できる。プリセットごとに新しいスナップショットを 1 つ以上保持し、
復旧ドリルを迅速化する。

## 5. ブロック/イベント/ステータスストリームの修復

- **ストリーム停止だが peer は正常。** **Events**/**Blocks** の赤いステータスバーを確認。
  「Stop」→「Start」で再購読を強制する。supervisor は peer 別の再接続試行と
  エラーをログするため、backoff ステージを確認できる。
- **Status オーバーレイが古い。** `ManagedStatusStream` は `/status` を 2 秒毎にポーリングし、
  `STATUS_POLL_INTERVAL * STATUS_STALE_MULTIPLIER`（デフォルト 6 秒）後にデータを stale と判定する。
  バッジが赤のままなら、peer 設定の `torii_status_url` を確認し、
  gateway/VPN がループバック接続を阻害していないか確認する。
- **イベントのデコード失敗。** UI はデコード段階（raw bytes, `BlockSummary`, Norito decode）と
  問題のトランザクションハッシュを表示する。クリップボードボタンでイベントをエクスポートし、
  テスト内で再現する（`mochi-core` のヘルパーは `mochi/mochi-core/src/torii.rs` にある）。

ストリームが繰り返しクラッシュする場合は、正確な peer alias と
エラー文字列（`ToriiErrorKind`）を issue に記載し、ロードマップのテレメトリ
マイルストーンを具体的な証拠に結び付ける。
