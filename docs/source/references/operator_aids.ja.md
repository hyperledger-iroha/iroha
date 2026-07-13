<!-- Japanese translation of docs/source/references/operator_aids.md -->

---
lang: ja
direction: ltr
source: docs/source/references/operator_aids.md
status: needs-update
translator: manual
---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/references/operator_aids.md` for current semantics.

# Torii エンドポイント — オペレーター向け早見表

本ページはコンセンサスに影響しない、オペレーター視点の可観測性／トラブルシューティング用エンドポイントをまとめています。特記がない限りレスポンスは JSON です。

## コンセンサス（Sumeragi）

- メトリクス: `sumeragi_new_view_receipts_by_hv{height,view}` ゲージが同じカウントを公開。
- `GET /v1/sumeragi/status`
  - リーダーインデックス、Highest/Locked QCs（`highest_qc`/`locked_qc` の高さ・ビュー・サブジェクトハッシュ）、コレクタ／VRF カウンター、ペースメーカーの猶予、トランザクションキュー深さ、RBC ストア状態（`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`）を取得。
- `GET /v1/sumeragi/status/sse`
  - `/v1/sumeragi/status` と同じペイロードの SSE（約 1 秒間隔）。
- `GET /v1/sumeragi/qc`
  - highest/locked QCs のスナップショット。highest QC のブロックハッシュが判明していれば `subject_block_hash` を含む。
- `GET /v1/sumeragi/pacemaker`
  - ペースメーカーのタイマー／設定値 `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`。
- `GET /v1/sumeragi/leader`
  - リーダーインデックスの現在値。NPoS モードでは PRF コンテキスト `{ height, view, epoch_seed }` を含む。
- `GET /v1/sumeragi/telemetry`
  - 集約コンセンサス・テレメトリ。`availability.collectors` は観測された collector index、peer ID、取り込み vote 数、`rbc_backlog` は欠落 chunk の集計、`rbc_pending` は pre-session キュー、drop、上限を示します。決定論的 collector plan やセッション単位の RBC API ではありません。
- `GET /v1/sumeragi/params`
  - オンチェーンの Sumeragi パラメータ `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`。
  - `da_enabled` が true の場合、availability evidence は追跡されますが commit gate ではありません。ローカル payload は RBC `DELIVER` または block sync で取得できます。診断には集約 telemetry、Prometheus、status、ログを使用します。

## エビデンス（監査・非コンセンサス）

- `GET /v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- `GET /v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - DoublePrepare/DoubleCommit、InvalidQc、InvalidProposal などの基本フィールドを含む。
  - 例:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- `POST /v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - CLI ヘルパー:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>`（または `--evidence-hex-file <path>`）

## オペレーター認証（WebAuthn/mTLS）

- `POST /v1/operator/auth/registration/options`
  - 初回の資格情報登録向けに WebAuthn 登録オプション（`publicKey`）を返す。
- `POST /v1/operator/auth/registration/verify`
  - WebAuthn のアテステーションを検証してオペレーター資格情報を永続化。
- `POST /v1/operator/auth/login/options`
  - オペレーターのログイン用 WebAuthn 認証オプション（`publicKey`）を返す。
- `POST /v1/operator/auth/login/verify`
  - WebAuthn のアサーションを検証し、オペレーターセッショントークンを返す。
- ヘッダー:
  - `x-iroha-operator-session`: オペレーター用エンドポイントのセッショントークン（login verify が発行）。
  - `x-iroha-operator-token`: ブートストラップトークン（`torii.operator_auth.token_fallback` が許可する場合）。
  - `x-api-token`: `torii.require_api_token = true` または `torii.operator_auth.token_source = "api"` の場合に必須。
  - `x-forwarded-client-cert`: `torii.operator_auth.require_mtls = true` の場合に必須（エッジプロキシで付与）。
- 登録フロー:
  1. ブートストラップトークンで registration options を呼ぶ（`token_fallback = "bootstrap"` では初回の資格情報登録前のみ許可）。
  2. オペレーター UI で `navigator.credentials.create` を実行し、アテステーションを registration verify に送信。
  3. login options と login verify を呼び、`x-iroha-operator-session` を取得。
  4. オペレーター向けエンドポイントに `x-iroha-operator-session` を付与して呼び出す。

備考
- これらエンドポイントはノードローカルのビュー（必要に応じてメモリ上）であり、コンセンサスや永続化には影響しません。
- Torii の設定に応じて API トークン、オペレーター認証（WebAuthn/mTLS）、レート制限で保護されている場合があります。
