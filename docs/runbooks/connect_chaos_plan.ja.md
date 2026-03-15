---
lang: ja
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2026-01-01
---

# Connect カオス/障害リハーサル計画 (IOS3 / IOS7)

このプレイブックは、ロードマップのアクション _"plan joint chaos rehearsal"_ (`roadmap.md:1527`) を満たす、繰り返し実施できるカオスドリルを定義する。クロス SDK のデモを行う際は、Connect プレビューのランブック (`docs/runbooks/connect_session_preview_runbook.md`) と組み合わせること。

## 目的と成功基準
- 共有の Connect retry/back-off ポリシー、オフラインキュー上限、テレメトリエクスポータを、プロダクションコードを変更せずに制御された障害下で検証する。
- 決定論的な成果物 (`iroha connect queue inspect` の出力、`connect.*` のメトリクススナップショット、Swift/Android/JS SDK のログ) を収集し、ガバナンスが各ドリルを監査できるようにする。
- ウォレットと dApp が設定変更 (manifest drift、salt のローテーション、attestation 失敗) を尊重し、標準の `ConnectError` カテゴリとレダクション安全なテレメトリイベントが出ることを示す。

## 前提条件
1. **環境のブートストラップ**
   - デモ用 Torii スタックを起動する: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - 少なくとも 1 つの SDK サンプルを起動する (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **インストゥルメンテーション**
   - SDK 診断を有効化する (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     Swift の `ConnectSessionDiagnostics`; Android/JS の `ConnectQueueJournal` + `ConnectQueueJournalTests` 相当)。
   - CLI の `iroha connect queue inspect --sid <sid> --metrics` が SDK が生成した
     キューパス (`~/.iroha/connect/<sid>/state.json` と `metrics.ndjson`) を解決することを確認する。
   - 次の時系列が Grafana と `scripts/swift_status_export.py telemetry` から見えるように
     テレメトリエクスポータを接続する: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **証跡フォルダ** - `artifacts/connect-chaos/<date>/` を作成して次を保存する:
   - 生ログ (`*.log`)、メトリクススナップショット (`*.json`)、ダッシュボードエクスポート
     (`*.png`)、CLI 出力、PagerDuty IDs。

## シナリオマトリクス

| ID | 障害 | 注入手順 | 期待されるシグナル | 証跡 |
|----|------|----------|--------------------|------|
| C1 | WebSocket 停止と再接続 | `/v2/connect/ws` を proxy の背後に置く (例: `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) か、サービスを一時的に停止する (`kubectl scale deploy/torii --replicas=0` を <=60 s)。ウォレットにフレーム送信を続けさせてオフラインキューを満たす。 | `connect.reconnects_total` が増加し、`connect.resume_latency_ms` がスパイクするが <1 s p95 に収まる。キューは `ConnectQueueStateTracker` 経由で `state=Draining` に入る。SDK は `ConnectError.Transport.reconnecting` を 1 回出してから復帰する。 | - `iroha connect queue inspect --sid <sid>` の出力で `resume_attempts_total` が非ゼロであること。<br>- 障害ウィンドウのダッシュボード注釈。<br>- reconnect + drain のログ抜粋。 |
| C2 | オフラインキュー overflow / TTL 失効 | サンプルをパッチしてキュー上限を縮小する (Swift: `ConnectSessionDiagnostics` 内で `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` を使う; Android/JS は同等のコンストラクタ)。dApp がリクエストを積み続ける間、ウォレットを >=2x `retentionInterval` 停止する。 | `connect.queue_dropped_total{reason="overflow"}` と `{reason="ttl"}` が増加し、`connect.queue_depth` は新しい上限で頭打ちになる。SDK は `ConnectError.QueueOverflow(limit: 4)` (または `.QueueExpired`) を出す。`iroha connect queue inspect` は `state=Overflow` を示し、`warn/drop` が 100% に達する。 | - メトリクスカウンタのスクリーンショット。<br>- overflow を示す CLI JSON。<br>- `ConnectError` 行を含む Swift/Android ログ。 |
| C3 | Manifest drift / admission 拒否 | ウォレットに提供する Connect manifest を改変する (例: `docs/connect_swift_ios.md` のサンプル manifest を変更、または `--connect-manifest-path` で `chain_id` や `permissions` が異なるコピーを指定)。dApp に承認を要求させ、ウォレットがポリシーで拒否することを確認する。 | Torii が `/v2/connect/session` に `HTTP 409` と `manifest_mismatch` を返す。SDK は `ConnectError.Authorization.manifestMismatch(manifestVersion)` を出し、テレメトリは `connect.manifest_mismatch_total` を増加させ、キューは空のまま (`state=Idle`)。 | - mismatch 検知を示す Torii ログ抜粋。<br>- SDK のエラー画面。<br>- テスト中にキューが空であることを示すメトリクス。 |
| C4 | キー回転 / salt バージョン更新 | セッション途中で Connect の salt/AEAD キーを回転させる。dev 環境では `CONNECT_SALT_VERSION=$((old+1))` で Torii を再起動する (Android の塩テストに対応: `docs/source/sdk/android/telemetry_schema_diff.md`)。回転が完了するまでウォレットをオフラインにし、その後再開する。 | 最初の再開は `ConnectError.Authorization.invalidSalt` で失敗し、キューがフラッシュされる (dApp は `salt_version_mismatch` でキャッシュフレームを破棄)。テレメトリは `android.telemetry.redaction.salt_version` と `swift.connect.session_event{event="salt_rotation"}` を出す。SID を更新した 2 回目のセッションは成功する。 | - 塩エポックの前後を示すダッシュボード注釈。<br>- invalid-salt と成功を示すログ。<br>- `iroha connect queue inspect` で `state=Stalled` から `state=Active` に遷移する証跡。 |
| C5 | Attestation / StrongBox 失敗 | Android ウォレットで `ConnectApproval` に `attachments[]` + StrongBox attestation を含める。attestation harness (`scripts/android_keystore_attestation.sh` と `--inject-failure strongbox-simulated`) を使うか、dApp に渡す前に attestation JSON を改変する。 | dApp が `ConnectError.Authorization.invalidAttestation` で拒否し、Torii は理由をログに残す。エクスポータは `connect.attestation_failed_total` を増やし、キューは該当エントリを削除する。Swift/JS dApp はエラーをログしつつセッションを継続する。 | - 注入失敗 ID を含む harness ログ。<br>- SDK エラーログ + テレメトリカウンタ。<br>- キューが悪いフレームを削除した証跡 (`recordsRemoved > 0`)。 |

## シナリオ詳細

### C1 - WebSocket 停止と再接続
1. Torii を proxy (toxiproxy, Envoy, `kubectl port-forward`) の背後に置き、
   ノード全体を落とさずに可用性を切り替えられるようにする。
2. 45 s の停止を発生させる:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. テレメトリダッシュボードと `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json` を確認する。
4. 停止直後にキュー状態をダンプする:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. 成功 = 1 回だけ再接続し、キュー増加が制限され、proxy 復旧後に自動 drain される。

### C2 - オフラインキュー overflow / TTL 失効
1. ローカルビルドでキュー閾値を下げる:
   - Swift: サンプル内の `ConnectQueueJournal` 初期化を更新
     (例: `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     して `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` を渡す。
   - Android/JS: `ConnectQueueJournal` 構築時に同等の設定を渡す。
2. dApp が `ConnectClient.requestSignature(...)` を発行し続けている間、
   ウォレットを >=60 s 停止する (シミュレータのバックグラウンドまたは機内モード)。
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) もしくは JS の診断ヘルパーで
   証跡バンドル (`state.json`, `journal/*.to`, `metrics.ndjson`) をエクスポートする。
4. 成功 = overflow カウンタが増加し、SDK が `ConnectError.QueueOverflow` を一度出し、
   ウォレット復帰後にキューが回復する。

### C3 - Manifest drift / admission 拒否
1. admission manifest をコピーする例:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. `--connect-manifest-path /tmp/manifest_drift.json` で Torii を起動する (または
   docker compose/k8s の設定を drill 用に更新する)。
3. ウォレットからセッションを開始し、HTTP 409 を期待する。
4. Torii + SDK ログと `connect.manifest_mismatch_total` をダッシュボードから取得する。
5. 成功 = キューが増えずに拒否され、ウォレットが共通の分類エラー
   (`ConnectError.Authorization.manifestMismatch`) を表示する。

### C4 - キー回転 / salt 変更
1. 現在の salt バージョンをテレメトリから記録する:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. `CONNECT_SALT_VERSION=$((OLD+1))` で Torii を再起動 (または config map を更新)。
   再起動完了までウォレットをオフラインにする。
3. ウォレットを再開すると、最初の resume は invalid-salt で失敗し、
   `connect.queue_dropped_total{reason="salt_version_mismatch"}` が増える。
4. セッションディレクトリを削除してキャッシュフレームを落とす
   (`rm -rf ~/.iroha/connect/<sid>` またはプラットフォーム別のキャッシュ削除) か、
   新しいトークンでセッションを再開する。
5. 成功 = テレメトリが salt 変更を示し、invalid-resume が 1 回だけ記録され、
   次のセッションは手動介入なしで成功する。

### C5 - Attestation / StrongBox 失敗
1. `scripts/android_keystore_attestation.sh` で attestation bundle を生成する
   (`--inject-failure strongbox-simulated` で署名ビットを反転)。
2. ウォレットが `ConnectApproval` API で bundle を添付するようにし、dApp は
   payload を検証して拒否する。
3. テレメトリ (`connect.attestation_failed_total`, Swift/Android の incident メトリクス) を確認し、
   キューが悪いエントリを削除したことを確認する。
4. 成功 = 拒否は不正な approval のみに限定され、キューは健全で、attestation ログが証跡と一緒に保存される。

## 証跡チェックリスト
- `scripts/swift_status_export.py telemetry` からの
  `artifacts/connect-chaos/<date>/c*_metrics.json` exports。
- `iroha connect queue inspect` の CLI 出力 (`c*_queue.txt`).
- SDK + Torii のログ (タイムスタンプと SID ハッシュ付き)。
- 各シナリオの注釈付きダッシュボードスクリーンショット。
- Sev 1/2 アラートが発火した場合の PagerDuty / incident IDs。

四半期に一度このマトリクスを完了することでロードマップのゲートを満たし、
Swift/Android/JS の Connect 実装が高リスクな故障モードでも決定論的に
応答することを示せる。
