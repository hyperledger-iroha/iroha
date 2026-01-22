---
lang: ja
direction: ltr
source: docs/source/sorafs_proof_streaming_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a3af98cf966b3fd9927c0029d145b7e50b84b289cf6d52dcf55a4b643f86acd
source_last_modified: "2025-11-02T18:05:35.356355+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sorafs_proof_streaming_plan.md -->

# SoraFS 証明ストリーミング／監視計画（ドラフト）

## 目標

- CLI/SDK に PoR サンプルと PoTR（期限証明）のリクエスト／検証用ストリーミング API を提供する。
- 証明の成功／失敗、レイテンシ、プロバイダ応答の観測データを出力する。
- オーケストレーターおよびゲートウェイのテレメトリと統合する。

> **ステータス（2026年3月）:** `sorafs_cli proof stream` は PoR サンプルをストリーミングし、
> キャッシュ済み PoTR レシートを構造化メトリクス付きでリプレイできる
> （`docs/source/sorafs_proof_streaming.md`）。
> PDP ストリーミングは SF-13 の CDC コミットメントと並行してロードマップ上に残り、
> PoTR のライブプローブは SF-14 のプロバイダ・プロトコル拡張で提供する。
> Prometheus のエクスポートは SF-7 のテレメトリ・デリバラブルと同時に出荷予定。
> 最新の設計レビュー決定は `docs/source/sorafs_proof_streaming_minutes_20250305.md` を参照。

## API コンセプト

- `ProofStreamRequest`:
  - `manifest_digest`
  - `provider_id`
  - `sample_count`
  - `nonce`
- `ProofStreamResponse`（ストリーミング項目）:
  - `sample_index`, `chunk_index`, `proof`, `verification_status`, `latency_ms`。

CLI コマンド（ドラフト）:
- `sorafs stream-por --manifest manifest.to --provider <id> --samples 128`
- `sorafs stream-potr --manifest manifest.to --provider <id> --deadline 90s`

SDK 機能:
- Rust の async iterator（`ProofStream`）。
- TS の Promise ベース API（`sorafsSDK.proof.streamPor({...})`）。
- Go のコンテキストマネージャ。

## テレメトリ

- Counters: `sorafs_proof_stream_success_total`, `sorafs_proof_stream_failure_total{reason}`
- Histograms: `sorafs_proof_stream_latency_ms_bucket`
- Gauges: `sorafs_proof_stream_inflight`

## 統合ポイント

- ゲートウェイの `POST /proof/{manifest_cid}`（PoR）と将来の PoTR API。
- チャンク取得後に証明を要求するオーケストレーター。
- 証明ストリーミングのスモークテストを実行する CI パイプライン。

## スキーマ整合（SF-13 PDP & SF-14 PoTR）

- **統一リクエスト封筒。**
  ```norito
  struct ProofStreamRequestV1 {
      manifest_digest: Hash,
      provider_id: ProviderId,
      proof_kind: ProofKind,         // Por | Pdp | Potr
      sample_count: Option<u32>,     // Required for PoR/PDP
      deadline_ms: Option<u32>,      // Required for PoTR
      nonce: [u8; 16],               // Client-supplied to prevent replay
      orchestrator_job_id: Option<Uuid>,
      tier: Option<ProofTier>,       // hot | warm | archive (maps to PDP/PoTR tiers)
  }
  enum ProofKind { Por, Pdp, Potr }
  enum ProofTier { Hot, Warm, Archive }
  ```
  このスキーマにより、オーケストレーターと CLI は PoR（SF-9）、PDP（SF-13）、PoTR
  （SF-14）のリクエストを分岐せずに扱える。PDP リクエストは `proof_kind=Pdp`、
  `sample_count`、`tier` を必須とする。PoTR リクエストは `deadline_ms` を必須とし、
  `sample_count` は省略する。
- **ストリーミング応答項目。**
  ```norito
  struct ProofStreamItemV1 {
      manifest_digest: Hash,
      provider_id: ProviderId,
      proof_kind: ProofKind,
      sample_index: Option<u32>,
      chunk_index: Option<u32>,
      receipt: ProofReceiptV1,
      verification_status: VerificationStatus,
      latency_ms: u32,
      failure_reason: Option<FailureReason>,
      trace_id: Option<Uuid>,
  }
  ```
  - PoTR では `sample_index` は `None` で、`receipt` に SF-14 の署名済み期限証明を格納する。
  - PDP では `receipt` が PDP 計画で定義される CDC ベースのコミットメント証明を参照し、
    `Sora-PDP-Proof` フィールド（コミットメントルート、チャレンジソルト）を含む。
  - PoR は標準チャンク証明と Merkle パスを格納する。
- **テレメトリフック。** 各ストリーミング項目は前述のカウンタ／ヒストグラムに反映される。
  PDP 失敗は共通の `FailureReason` を通じて SF-13 のスラッシングパイプラインへ伝播する。

## Failure Reason タクソノミ

- `timeout` — オーケストレーター期限（PoR/PDP）内に応答がない、または PoTR SLA 違反。
- `invalid_proof` — 検証失敗（ハッシュ不一致、Merkle パス不正、PDP コミットメント不一致）。
- `admission_mismatch` — マニフェスト／アドミッション不整合でプロバイダが拒否。
- `token_exhausted` — ストリーム中にトークン枯渇。
- `provider_unreachable` — 伝送エラー（接続拒否、TLS 失敗）。
- `orchestrator_aborted` — クライアント／オーケストレーターがストリームを中断。
- `unsupported_capability` — 要求された証明種別／ティアをプロバイダが未対応。

これらの列挙値はオーケストレーション・テレメトリ（`failure_reason` ラベル）と共有され、
ダッシュボードとアラートの一貫性を保つ。CLI/SDK はこれらをユーザー向けエラーメッセージと
終了コードにマップする。

## トランスポート方針

- **主要メカニズム: HTTP/2 ストリーミング。**
  - ゲートウェイは `POST /v1/proof/stream` を公開し、`ProofStreamRequestV1` を受け取り、
    `application/x-ndjson` 本体で応答する（行ごとに `ProofStreamItemV1`）。HTTP/2 でチャンク
    フェッチと多重化でき、既存のゲートウェイ基盤と整合する。
  - フロー制御でバックプレッシャを扱う。ゲートウェイは 64 件以上をバッファしない。
  - 応答には `Sora-Trace-Id` ヘッダを付与し、オーケストレーターが OpenTelemetry スパンと
    相関できるようにする。
- **任意の gRPC エンドポイント。**
  - `sorafs.proof.v1.ProofStreamService/StreamProofs` は双方向ストリームを返し、gRPC を使う環境
    （内部テスト、SDK 統合）に向けて提供する。HTTP のセマンティクスを踏襲し、
    Norito ペイロードを共用する。
- **非ゴール。** WebSocket は不要と判断する。HTTP/2 ストリーミングで双方向要件を満たし、
  既存の mTLS ゲートウェイと同等のセキュリティ姿勢を維持する。
- **CLI/SDK 実装。**
  - Rust の async iterator は NDJSON を読み取り、`trace_id` を検証して検証パイプラインに供給する。
  - TypeScript SDK はランタイムに応じて `ReadableStream` または `AsyncGenerator` を使用し、
    Node/CDN ビルドは fetch ストリーミング（WHATWG）に依存する。
  - Go SDK は HTTP レスポンスボディをデコーダで包み、コンテキストキャンセルを尊重する
    チャネル経由で項目を返す。

これらの決定により、証明ストリーミングは SF-13/SF-14 と整合し、決定論的なエラーコードを
公開しつつ、SoraFS ゲートウェイで既に強化済みのトランスポートを活用できる。
