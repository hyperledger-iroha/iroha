<!-- Japanese translation of docs/source/connect_architecture_strawman.md -->

---
lang: ja
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
translator: manual
---

# Connect セッションアーキテクチャ ストローマン（Swift / Android / JS）

本ストローマンは、Swift・Android・JavaScript 向け Nexus Connect ワークフローの共通設計をまとめたものです。2026 年 2 月に予定されているクロス SDK ワークショップに向けて整備されており、実装に入る前に未解決事項を洗い出すことを目的としています。

> 最終更新日: 2026-01-12  
> 著者: Swift SDK リード、Android ネットワーキング TL、JS リード  
> ステータス: 評議会レビュー向けドラフト

## ゴール

1. ウォレット ↔ dApp のセッションライフサイクル（接続ブートストラップ、承認、署名リクエスト、終了処理）を揃える。
2. すべての SDK で共有する Norito エンベロープスキーマ（open/approve/sign/control）を定義し、`connect_norito_bridge` との整合を確保する。
3. トランスポート（WebSocket / WebRTC）、暗号化（Norito Connect フレーム + 鍵交換）、アプリケーション層（SDK ファサード）の責務を分離する。
4. デスクトップ／モバイル間で決定論的な挙動を保証し、オフラインバッファリングや再接続も含めて一貫性を保つ。

## セッションライフサイクル（高レベル）

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## エンベロープ / Norito スキーマ

すべての SDK は `connect_norito_bridge` で定義された正準 Norito スキーマを使用する必要があります。

- `EnvelopeV1`（open / approve / sign / control）
- `ConnectFrameV1`（AEAD ペイロードを持つ暗号化フレーム）
- コントロールコード
  - `open_ext`（メタデータ・許可）
  - `approve_ext`（アカウント、許可、証明、署名）
  - `reject`, `close`, `ping/pong`, `error`

Swift には以前プレースホルダーの JSON エンコーダ（`ConnectCodec.swift`）が存在しましたが、2026 年 4 月時点で ConnectCodec は Norito ブリッジ必須となり、JSON フォールバックは削除されています。本ストローマンは当初の要件を記録したものです。

| 関数 | 説明 | ステータス |
|------|------|------------|
| `connect_norito_encode_control_open_ext` | dApp の open フレーム | ブリッジで実装済み |
| `connect_norito_encode_control_approve_ext` | ウォレット承認フレーム | 実装済み |
| `connect_norito_encode_envelope_sign_request_tx/raw` | 署名リクエスト | 実装済み |
| `connect_norito_encode_envelope_sign_result_ok/err` | 署名結果 | 実装済み |
| `connect_norito_decode_*` | ウォレット／dApp 向けデコード | 実装済み |

### 必要な作業

- Swift: プレースホルダーの `ConnectCodec` JSON ヘルパーをブリッジ呼び出しに置き換え、共有 Norito 型を用いた `ConnectFrame` / `ConnectEnvelope` ラッパーを追加する。✅（2026 年 4 月）
- Android / JS: 同様のラッパーを実装し、エラーコードやメタデータキーを揃える。
- 共通事項: Norito 仕様に沿った暗号化（X25519 鍵交換 + AEAD）を文書化し、Rust ブリッジを用いたサンプル統合テストを提供する。

## トランスポート契約

- 主なトランスポートは WebSocket（`/v1/connect/ws?sid=<session_id>`）。
- 将来的なオプションとして WebRTC を検討（初期ストローマンでは範囲外）。
- SDK の責務:
  - Ping/Pong ハートビートを維持し、モバイルでの電池消費を抑制する。
  - オフライン時に送信フレームをバッファリング（上限つきキュー、dApp 用に永続化）。
  - イベントストリーム API を提供（Swift Combine の `AsyncStream`、Android Flow、JS async iterator）。
  - 再接続フックを公開し、再購読を手動で要求できるようにする。

## 暗号化と鍵管理

- 曲線: X25519 を用いた鍵交換（共有秘密 → HKDF → AEAD 鍵）。
- AEAD: ChaCha20-Poly1305（Norito 仕様準拠）。
- ノンス: セッション方向ごとの単調増加カウンタ（`sequence`）。
- 鍵ローテーション: 任意。初期ストローマンではセッション中 1 鍵を想定。
- JS クライアント: セキュアコンテキストな WebCrypto を必須とし、Node.js 環境では `npm run build:native` で構築するネイティブ `iroha_js_host` ブリッジまたは HSM プラグインを利用する。
- StrongBox / Secure Enclave:
  - ウォレット SDK は利用可能ならハードウェアバックの鍵保管を優先する。
  - 署名承認には証明データ（Android StrongBox、iOS Secure Enclave）を含める場合がある。
  - ハードウェア非対応デバイス向けフォールバック手順を文書化する。

## 許可と証明

- 許可 JSON の内容:
  - 要求スコープ（トランザクション署名、バイト列署名など）。
  - 制約条件（チェーン ID、アカウントフィルタ、TTL）。
  - 任意のコンプライアンスペイロード（KYC 証明、同意情報）。
- ウォレット承認が含むもの:
  - 選択されたアカウント（マルチアカウントウォレットの場合）。
  - 証明バンドル（ZK 証明またはアテステーション）。
  - 拒否時に任意の理由メッセージ。

## SDK ファサード

| SDK | 提案 API | 備考 |
|-----|----------|------|
| Swift | `ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval` | プレースホルダーを型付きラッパー + 非同期ストリームで置換 |
| Android | Kotlin コルーチン + シールドクラス | Swift 構造と整合 |
| JS | Async iterator + TypeScript enum | ブラウザ／Node 向け SDK を提供 |

### 共通動作

- `ConnectSession` がライフサイクルを管理:
  1. WebSocket を確立し、ハンドシェイクを実施。
  2. open / approve フレームを交換。
  3. 署名リクエスト／結果を処理。
  4. アプリケーション層へイベントを配信。
- 高レベルヘルパーの提供:
  - `requestSignature(tx, metadata)`
  - `approveSession(account, permissions)`
  - `reject(reason)`
- エラーハンドリング: Norito エラーコードを SDK 固有のエラーにマップし、ドメイン固有コードを UI へ伝播する。

## オフラインと再接続

### ジャーナル契約

各 SDK はセッションごとに append-only ジャーナルを保持し、オフライン中のフレームを損失なく再送できるようにします。Norito ブリッジ型そのままのバイト列を保存するため、モバイル／JS 間で互換性があります。

- ジャーナルはハッシュ化された `sid`（`sha256(sid)`）単位で `app_to_wallet.queue` / `wallet_to_app.queue` の 2 ファイルを生成。Swift は Application Support 配下のサンドボックス済みファイル、Android は Room/FileChannel（暗号化）、JS は IndexedDB を利用します。
- レコード形式 `ConnectJournalRecordV1`:
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]`（暗号文+ヘッダーの Blake3）
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext`（Norito で暗号化済みのフレーム）
- 暗号文をそのまま fsync するだけでよく、再暗号化は行いません。`ConnectQueueState` 構造体が深さ／バイト数／最古・最新シーケンスを追跡し、テレメトリと FlowControl を駆動します。
- 既定の上限は 32 フレームまたは 1 MiB。超過すると最古レコードを `reason=overflow` で破棄します。`ConnectFeatureConfig.max_queue_len` で調整可能。
- データ保持は 24 時間（`expires_at_ms`）。バックグラウンド GC が期限切れセグメントを即時削除してフットプリントを抑制します。
- クラッシュ安全性: 追記→fsync→メモリミラー更新の順で行い、復帰時はチェックサム検証後に `ConnectQueueState` を再構築。壊れたレコードはスキップし、テレメトリで通知し、必要に応じて隔離します。
- プライバシー要件上、ハッシュ化された `sid` 以外の付加情報は持ちません。`telemetry_opt_in=false` の場合でもジャーナルは保持されますが、深さ/`sid` はログ出力されません。
- すべての SDK が `ConnectQueueObserver` を公開し、アプリが UI からキュー深さ・ドレイン状況・GC 結果を監視できるようにします。

### リプレイ／レジュームのセマンティクス

1. 再接続時に `Control::Resume {seq_app_max, seq_wallet_max, queued_app, queued_wallet, journal_hash}` を送信。`journal_hash` はジャーナル全体の Blake3 で、乖離検知に使います。
2. 受信側は状態を比較し、ギャップがあれば再送を要求し、再送済みフレームは `Control::ResumeAck` で確認。
3. 再送は挿入順（sequence → 書き込み時刻）を厳守。ウォレットは FlowControl トークンを発行して dApp がオフライン中にキューを溢れさせないようにします。
4. ジャーナルは暗号文を保持するだけなので、再生は記録済みバイト列をトランスポートへ再送するだけです。SDK 側での再エンコードは禁止です。

### 再接続フロー

1. トランスポート層が WebSocket を張り直し、ping 間隔を再確立。
2. dApp は FlowControl を尊重しながらキューされたフレームを順に送信。
3. ウォレットは復号し、シーケンス単調性を検証し、承認／結果を再生。
4. 両者が `resume` コントロールで `seq_app_max/seq_wallet_max` とキュー深さを報告。
5. `sequence` + `payload_hash` が一致する重複フレームは ACK 後に破棄。差異があれば `ConnectError.Internal` でセッションを再初期化。

### フェイルモード

- `offline_timeout_ms`（既定 5 分）を超えたセッションはキューを破棄し `ConnectError.sessionExpired` を返します。
- ジャーナル破損時は 1 回だけ Norito デコーダで修復を試み、失敗した場合 `connect.queue_repair_failed` を送ってジャーナルを隔離・削除。
- シーケンス不一致は `ConnectError.replayDetected` で通知し、新しい `sid` を要求します。

### オフラインバッファ計画と運用コントロール

ワークショップの受け入れ条件として、各 SDK が同一の状態機械／操作フロー／証跡エクスポートを持つことが求められます。Swift (`ConnectSessionDiagnostics`)、Android (`ConnectDiagnosticsSnapshot`)、JS (`ConnectQueueInspector`) は以下の状態を共通実装します。

| 状態 | トリガー | 自動対応 | 手動操作 | テレメトリ |
|------|----------|----------|-----------|-------------|
| `Healthy` | `disk_watermark_warn`（60 %）未満 && TTL 有効 | なし | — | `connect.queue_state="healthy"` |
| `Throttled` | `disk_watermark_warn` 以上または 1 分あたり 5 回超のリトライ | 新規署名要求を停止、FlowControl を 50% に減速 | `clearOfflineQueue(.app|.wallet)` が片側バッファを消去し、復帰時に再同期 | `connect.queue_state="throttled"`、ゲージ `connect.queue_watermark` |
| `Quarantined` | `disk_watermark_drop`（85 %）以上、破損 2 回、`offline_timeout_ms` 超過 | バッファ停止、`ConnectError.QueueQuarantined` を返し、オペレーター承認が必要 | `ConnectSessionDiagnostics.forceReset()` が証跡取得後に削除 | `connect.queue_state="quarantined"`, `connect.queue_quarantine_total` |

- 閾値は `ConnectFeatureConfig`（`disk_watermark_warn/drop`, `max_disk_bytes`, `offline_timeout_ms`）から取得。未設定の場合は既定値を使い、警告ログを残します。
- 診断 API:
  - Swift: `ConnectSessionDiagnostics.snapshot()` で `{state, depth, bytes, reason}`、`exportJournalBundle(url:)` で両キューを保存。
  - Android: `ConnectDiagnostics.snapshot()` / `exportJournalBundle(path)`.
  - JS: `ConnectQueueInspector.read()` が同じ構造体とアップロード用 Blob を返却。
- アプリが `offline_queue_enabled=false` にした場合、即座に両ジャーナルを消去し `Disabled` 状態を記録、終端テレメトリを出力。設定は Norito 承認フレームにも反映され、相手機がリプレイ可否を判断できます。
- `connect queue inspect --sid <sid>` CLI は各 SDK の診断をラップし、カオス訓練時に状態推移・ウォーターマーク・レジューム証跡を取得します。

### 証跡バンドル手順

サポート／コンプライアンス向けに決定論的な証跡を生成する 3 ステップエクスポート:

1. `exportJournalBundle(..)` が `{app_to_wallet,wallet_to_app}.queue` と Build/feature/ウォーターマークを記したマニフェストを書き出す。
2. `exportQueueMetrics(..)` が直近 1,000 サンプルのテレメトリ（ユーザー同意時は `sid` ハッシュ込み）を出力。
3. CLI ヘルパーが両者を zip 化し、Norito 署名済みメタデータ `ConnectQueueEvidenceV1` を付与して Torii/SoraFS に投入。

検証に失敗したバンドルは `connect.evidence_invalid` を送って再取得を促します。

## テレメトリと診断

- Norito JSON イベントを OpenTelemetry で送出。必須メトリクス:
  - `connect.queue_depth{direction}`, `connect.queue_bytes{direction}`
  - `connect.queue_dropped_total{reason}`（`overflow|ttl|repair`）
  - `connect.offline_flush_total{direction}`, `connect.offline_flush_failed`
  - `connect.replay_success_total`, `connect.replay_error_total`
  - `connect.resume_latency_ms`, `connect.resume_attempts_total`, `connect.session_duration_ms`
  - `connect.error` 構造化イベント（`code`, `fatal`, `telemetry_profile`）
- すべて `{platform, sdk_version, feature_hash}` ラベルを付与。`sid` ハッシュは telemetry opt-in 時のみ送信。
- SDK レベルのフック:
  - Swift: `ConnectSession.addObserver(_:) -> ConnectEvent`
  - Android: `Flow<ConnectEvent>`
  - JS: 非同期イテレーターまたはコールバック
- CI: Swift `make swift-ci`, Android `./gradlew sdkConnectCi`, JS `npm run test:connect` を必須化し、テレメトリがグリーンであることを保証。
- 構造化ログは `sid` ハッシュ, `seq`, `queue_depth`, `sid_epoch` を含み、修復失敗時は `connect.queue_repair_failed{reason}` とダンプパスを出力。

### テレメトリフックとガバナンス証跡

- `connect.queue_state` をロードマップ上のリスク指標として利用し、ダッシュボードで `{platform, sdk_version}` ごとの滞在時間を追跡。ガバナンスは月次ドリル証跡をサンプリングできます。
- `connect.queue_watermark` と `connect.queue_bytes` は `risk.connect.offline_buffer` スコアを形成し、セッションの 5% 超が 10 分以上 `Throttled` の場合に SRE をページ。
- すべてのイベントに `feature_hash` を添付し、レビュー済みビルドと一致しない場合は SDK CI が即座に失敗。
- ポリシー閾値を超過すると `connect.policy_violation` を発火し、ハッシュ化された `sid`・状態・対処 (`drain|purge|quarantine`) を記録。
- `exportQueueMetrics` の成果物は Connect ランブックと同じ SoraFS 名前空間に保存され、評議会が CLI なしで監査できるようにします。

## 未解決の質問

1. **セッションディスカバリ:** WalletConnect のような QR コード / 帯域外ハンドシェイクは必要か？（将来検討）
2. **マルチシグ:** マルチシグ承認をどのように表現するか？（署名結果に複数署名をサポートする拡張が必要）
3. **コンプライアンス:** 規制対象フローで必須となるフィールドは？（コンプライアンスチームの指針待ち）
4. **SDK パッケージング:** Norito Connect コーデックなどの共通コードをクロスプラットフォームクレートに切り出すべきか？（要検討）

## 次のステップ

- 本ストローマンを SDK 評議会へ回覧（2026 年 2 月会合）。
- 未解決事項へのフィードバックを収集し、ドキュメントを更新。
- SDK ごとの実装タスクをスケジュール（Swift IOS7、Android AND7、JS Connect マイルストーン）。
- ロードマップのホットリストで進捗を追跡し、ストローマン確定後に `status.md` を更新する。
