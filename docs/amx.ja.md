---
lang: ja
direction: ltr
source: docs/amx.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 68b349701a8c9ac7f0a22fc46f1af38d06011e63283bcd2431d9b707cb45a392
source_last_modified: "2025-12-12T15:13:07.309952+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/amx.md -->

# AMX 実行・運用ガイド

**ステータス:** ドラフト (NX-17)  
**対象:** コアプロトコル、AMX/コンセンサスエンジニア、SRE/テレメトリ、SDK & Torii チーム  
**コンテキスト:** ロードマップ項目「Documentation (owner: Docs) — update `docs/amx.md` with timing diagrams, error catalog, operator expectations, and developer guidance for generating/using PVOs.」を完成させる。【roadmap.md:2497】

## サマリー

AMX（Atomic cross-data-space）トランザクションは、単一の提出で複数の
データスペース（DS）に触れつつ、1 秒スロットのファイナリティ、
決定論的な失敗コード、プライベート DS 断片の機密性を維持します。
本ガイドは、タイミングモデル、正規のエラー処理、オペレーターの
証跡要件、Proof Verification Objects (PVOs) の生成/利用に関する開発者
期待を整理し、Nexus 設計書（`docs/source/nexus.md`）外でも完結する
ロードマップ成果物にします。

主な保証:

- すべての AMX 提出は決定論的な prepare/commit 予算を受け取り、超過は
  文書化されたコードで中止され、lane をハングさせない。
- 予算を超えた DA サンプルは missing availability evidence として記録され、
  トランザクションは次のスロットへ再キューされる（スループットを止めない）。
- PVO は重い証明を 1 秒スロット外へ切り出し、クライアント/バッチャが
  事前登録した成果物をホストがスロット内で高速検証できる。
- IVM ホストは Space Directory から DS ごとの AXT ポリシーを導出する。
  handles はカタログの lane に一致し、最新の manifest root を提示し、
  `expiry_slot`/`handle_era`/`sub_nonce` の最小値を満たし、未知 DS は
  実行前に `PermissionDenied` で拒否される。
- スロット失効は `nexus.axt.slot_length_ms`（デフォルト `1` ms、`1`〜`600_000` ms）
  と制限付き `nexus.axt.max_clock_skew_ms`（デフォルト `0` ms、スロット長および
  `60_000` ms 以下）を使用。ホストは
  `current_slot = block.creation_time_ms / slot_length_ms` を計算し、許容スキューを
  失効・証明チェックに適用し、設定以上の skew を広告する handle を拒否する。
- 証明キャッシュ TTL: `nexus.axt.proof_cache_ttl_slots`（デフォルト `1`, `1`〜`64`）が
  証明のキャッシュ期間を制限し、TTL または proof の `expiry_slot` 到達で削除される。
- リプレイ台帳の保持: `nexus.axt.replay_retention_slots`（デフォルト `128`, `1`〜`4_096`）
  が handle 利用履歴の保持ウィンドウを設定。期待する handle 有効期間に合わせる。
  台帳は WSV に永続化され、起動時に復元され、保持ウィンドウと handle 失効の
  両方が過ぎた時点で決定論的に剪定される（peer 変更でもリプレイ穴を開けない）。
- キャッシュ状態のデバッグ: Torii は `/v2/debug/axt/cache`（telemetry/dev ゲート）を
  公開し、AXT ポリシーのスナップショット版、直近の拒否（lane/理由/版）、
  キャッシュ済み証明（dataspace/状態/manifest root/slots）、拒否ヒント
  (`next_min_handle_era`/`next_min_sub_nonce`) を返す。スロット/manifest
  ローテーションの反映や、handle 更新に使う。

## スロットタイミングモデル

### タイムライン

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- 予算配分はグローバル台帳計画に一致: mempool 70ms、DA commit ≤300ms、
  コンセンサス 300ms、IVM/AMX 250ms、settlement 40ms、guard 40ms。【roadmap.md:2529】
- DA ウィンドウ超過は missing availability evidence として記録し、次のスロットへ
  再試行。その他の超過は `AMX_TIMEOUT` や `SETTLEMENT_ROUTER_UNAVAILABLE` などの
  コードを返す。
- guard スライスはテレメトリ出力と最終監査を吸収し、exporter が遅れても 1 秒で
  スロットを閉じる。
- 設定の目安: デフォルトは厳格（`slot_length_ms = 1`, `max_clock_skew_ms = 0`）。
  1 秒 cadence なら `slot_length_ms = 1_000`、`max_clock_skew_ms = 250`。
  2 秒 cadence なら `slot_length_ms = 2_000`、`max_clock_skew_ms = 500`。検証範囲
  （`1`〜`600_000` ms、またはスロット長/`60_000` ms を超える skew）は
  パース時に拒否され、handle が広告する skew も同範囲内である必要がある。

### Cross-DS swim lane

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

各 DS フラグメントは 30ms の prepare ウィンドウ内に完了する必要がある。
不足した証明は次スロットへ回され、peer をブロックしない。

### 計測チェックリスト

| メトリクス / トレース | ソース | SLO / アラート | 備考 |
|----------------|--------|-------------|-------|
| `iroha_slot_duration_ms` (histogram) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | CI ゲートは `ans3.md` に記載。 |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (commit hook) | 30 分窓で ≥0.95 | missing-availability テレメトリから導出され、全ブロックで更新 (`crates/iroha_core/src/telemetry.rs:3524`,`crates/iroha_core/src/telemetry.rs:4558`). |
| `iroha_amx_prepare_ms` | IVM ホスト | DS ごと p95 ≤ 30 ms | `AMX_TIMEOUT` を引き起こす。 |
| `iroha_amx_commit_ms` | IVM ホスト | DS ごと p95 ≤ 40 ms | デルタ統合 + トリガ実行を含む。 |
| `iroha_ivm_exec_ms` | IVM ホスト | lane ごと >250 ms でアラート | IVM の overlay 実行ウィンドウに対応。 |
| `iroha_amx_abort_total{stage}` | Executor | >0.05 abort/slot または継続的スパイクで警告 | `prepare`/`exec`/`commit` のラベル。 |
| `iroha_amx_lock_conflicts_total` | AMX scheduler | >0.1 conflicts/slot で警告 | R/W セットの不正確さを示す。 |
| `iroha_axt_policy_reject_total{lane,reason}` | IVM ホスト | スパイクを監視 | manifest/lane/era/sub_nonce/expiry の拒否を区別。 |
| `iroha_axt_policy_snapshot_cache_events_total{event}` | IVM ホスト | cache_miss は起動/manifest 変更時のみ | 継続的 miss は古いポリシーを示す。 |
| `iroha_axt_proof_cache_events_total{event}` | IVM ホスト | 主に `hit`/`miss` が期待 | `reject`/`expired` のスパイクは manifest ドリフトや証明の陳腐化を示す。 |
| `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | IVM ホスト | キャッシュ証明の確認 | gauge 値は適用スキュー後の expiry_slot。 |
| Missing availability evidence (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) | Lane テレメトリ | DS あたり >5% で警告 | attesters や証明の遅延を示す。 |

`/v2/debug/axt/cache` は `iroha_axt_proof_cache_state` の gauge を dataspace
ごとのスナップショット（status, manifest root, verified/expiry slots）として
提供する。

`iroha_amx_commit_ms` と `iroha_ivm_exec_ms` は `iroha_amx_prepare_ms` と同じ
レイテンシーバケットを共有する。abort カウンタは拒否を lane id と stage
（`prepare`=overlay 構築/検証、`exec`=IVM chunk 実行、`commit`=デルタ統合+トリガ再生）
でタグ付けし、競合が R/W 不一致か後段マージかを可視化する。

オペレーターはこれらのメトリクスを証跡としてアーカイブし、
`status.md` に回帰を記録する必要がある。

### AXT ゴールデン fixtures

descriptor/handle/policy snapshot 用の Norito fixtures は
`crates/iroha_data_model/tests/fixtures/axt_golden.rs` にあり、
`crates/iroha_data_model/tests/axt_policy_vectors.rs` の `print_golden_vectors` が
再生成ヘルパー。CoreHost は
`core_host_enforces_fixture_snapshot_fields`（`crates/ivm/tests/core_host_policy.rs`）
で同じ fixtures を使い、lane binding、manifest root 一致、expiry_slot の新鮮さ、
handle_era/sub_nonce 最小値、missing dataspace の拒否を検証する。
- マルチ dataspace JSON fixture（`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`）
  は descriptor/touch スキーマ、正規 Norito bytes、Poseidon バインディング
  （`compute_descriptor_binding`）を固定する。`axt_descriptor_fixture` が
  エンコード bytes を検証し、SDK は `AxtDescriptorBuilder::builder` と
  `TouchManifest::from_read_write` を使って決定論的サンプルを生成できる。

### Lane カタログとマニフェスト

- AXT ポリシー snapshot は Space Directory の manifest セットと lane カタログから
  構築される。各 dataspace は構成された lane にマップされ、アクティブ manifest が
  manifest hash、activation epoch（`min_handle_era`）、sub-nonce フロアを提供する。
  アクティブ manifest がない UAID バインディングも 0 の manifest root を持つ
  ポリシーを出力し、lane gating を維持する。
- snapshot の `current_slot` は最新コミットブロックのタイムスタンプ
  （`creation_time_ms / slot_length_ms`）から導出し、コミットヘッダがない場合は
  ブロック高へフォールバックする。
- テレメトリは hydrated snapshot を `iroha_axt_policy_snapshot_version`
  （Norito エンコード hash の下位 64 bit）で公開し、
  `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}` で
  キャッシュイベントを表示する。拒否カウンタは `lane`, `manifest`, `era`,
  `sub_nonce`, `expiry` ラベルを使い、ブロック要因を即時把握できる。

### クロス dataspace コンポーザビリティのチェックリスト

- Space Directory に列挙された各 dataspace に lane エントリとアクティブ manifest が
  あることを確認する。ローテーションは新しい handle 発行前に binding と manifest
  root を更新する必要がある。root が 0 の場合、handle は拒否される。
- 起動時や Space Directory 変更後は `cache_miss` が 1 回発生し、以降は
  `cache_hit` が安定するはず。持続する miss は manifest 供給の欠落を示す。
- handle が拒否されたら `iroha_axt_policy_reject_total{lane,reason}` と snapshot
  バージョンを確認し、`expiry`/`era`/`sub_nonce` なら更新を、`lane`/`manifest` なら
  バインディング修復を行う。Torii の `/v2/debug/axt/cache` は `reject_hints` を返し、
  `dataspace`, `target_lane`, `next_min_handle_era`, `next_min_sub_nonce` を提供する。

### SDK サンプル: トークン流出なしのリモート支払い

1. AXT descriptor を構築し、対象資産を所有する dataspace とローカルな read/write
   タッチを列挙する。バインディング hash が安定するよう決定論性を維持する。
2. リモート dataspace へ `AXT_TOUCH` を実行し、期待する manifest view を指定する。
   必要なら `AXT_VERIFY_DS_PROOF` で証明を添付する。
3. 資産 handle を取得/更新し、`AXT_USE_ASSET_HANDLE` と `RemoteSpendIntent` を
   使ってリモート dataspace 内で支払いを実行する（ブリッジ不要）。
4. `AXT_COMMIT` で確定する。`PermissionDenied` の場合は拒否理由を参照して
   handle を更新するか lane/manifest バインディングを修復する。

## オペレーター期待事項

1. **スロット前の準備**
   - プロファイル別 DA attester プール（A=12, B=9, C=7）が健全であることを確認。
     attester の変動は Space Directory snapshot に記録される。
   - 代表的なランナーで `iroha_amx_prepare_ms` が予算内であることを確認してから
     新しいワークロードを有効化する。

2. **スロット内監視**
   - missing-availability のスパイク（2 連続スロットで >5%）と `AMX_TIMEOUT` を
     アラート対象にする。
   - PVO キャッシュの利用率（`iroha_pvo_cache_hit_ratio`）を追跡し、オフパス検証が
     提出に追随していることを証明する。

3. **証跡の収集**
   - DA 受領セット、AMX prepare ヒストグラム、PVO キャッシュレポートを nightly
     アーティファクトに添付し、`status.md` から参照する。
   - DA jitter/オラクル停止/バッファ枯渇のカオスドリル結果を
     `ops/drill-log.md` に記録する。

4. **Runbook メンテナンス**
   - AMX のエラーコードやオーバーライドが変わったら Android/Swift SDK の runbook を更新。
   - `iroha_config.amx.*` などの設定スニペットを `docs/source/nexus.md` の
     正規パラメータと同期させる。

## テレメトリとトラブルシューティング

### テレメトリ簡易リファレンス

| ソース | 取得対象 | コマンド/パス | 証跡期待 |
|--------|----------|---------------|----------|
| Prometheus (`iroha_telemetry`) | スロット/AMX SLO: `iroha_slot_duration_ms`, `iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, `iroha_da_quorum_ratio`, `iroha_amx_abort_total{stage}` | `https://$TORII/telemetry/metrics` をスクレイプまたは `docs/source/telemetry.md` のダッシュボードからエクスポート。 | ヒストグラムのスナップショット（必要ならアラート履歴）を nightly `status.md` に添付する。 |
| Torii RBC snapshots | DA/RBC backlog: セッションごとの chunk backlog、view/height メタデータ、availability カウンタ（`sumeragi_da_gate_block_total{reason="missing_local_data"}`; `sumeragi_rbc_da_reschedule_total` は legacy）。 | `GET /v2/sumeragi/rbc` と `GET /v2/sumeragi/rbc/sessions`（`docs/source/samples/sumeragi_rbc_status.md` 参照）。 | AMX DA アラート時に JSON 応答を保存し、インシデントバンドルに添付。 |
| Proof service metrics | PVO キャッシュ健全性: `iroha_pvo_cache_hit_ratio`, キャッシュ fill/evict カウンタ, proof queue depth | 証明サービスの `GET /metrics`（`IROHA_PVO_METRICS_URL`）または共通 OTLP collector。 | AMX スロットメトリクスと並べて cache hit ratio と queue depth を保存する。 |
| Acceptance harness | slot/DA/RBC/PVO 混在負荷 | `ci/acceptance/slot_1s.yml` を再実行し `artifacts/acceptance/slot_1s/<timestamp>/` に保存。 | GA 前および pacemaker/DA 設定変更時に必須。YAML サマリと Prometheus スナップショットを添付。 |

### トラブルシューティング・プレイブック

| 症状 | 最初の確認 | 推奨対処 |
|------|------------|----------|
| `iroha_slot_duration_ms` の p95 が 1 000 ms を超える | `/telemetry/metrics` の Prometheus エクスポートと最新 `/v2/sumeragi/rbc` で DA ディファーを確認し、`ci/acceptance/slot_1s.yml` の直近アーティファクトと比較。 | AMX バッチサイズを下げる、または追加の RBC collectors（`sumeragi.collectors.k`）を有効化し、acceptance harness を再実行して証跡を取得。 |
| missing availability の急増 | `/v2/sumeragi/rbc/sessions` の backlog と attester の健全性ダッシュボード。 | 不健全な attesters を外し、`redundant_send_r` を一時的に上げて配信を速め、`status.md` に記録。 |
| `PVO_MISSING_OR_EXPIRED` が頻発 | Proof service のキャッシュメトリクスとスケジューラログ。 | 古い PVO を再生成し、ローテーション周期を短縮し、SDK が `expiry_slot` 前に handle を更新するようにする。 |
| `AMX_LOCK_CONFLICT` または `AMX_TIMEOUT` が頻発 | `iroha_amx_lock_conflicts_total`, `iroha_amx_prepare_ms`, 該当 manifest。 | Norito static analyzer を再実行し、read/write selectors を修正（またはバッチ分割）、更新済み manifest fixtures を公開。 |
| `SETTLEMENT_ROUTER_UNAVAILABLE` のアラート | Settlement router ログ（`docs/settlement-router.md`）、treasury バッファのダッシュボード、該当レシート。 | XOR バッファの補充や lane の XOR-only モード切替を行い、トレジャリーの対応を記録、スロット acceptance テストを再実行。 |

### AXT 拒否シグナル

- 理由コードは `AxtRejectReason`（`lane`, `manifest`, `era`, `sub_nonce`, `expiry`,
  `missing_policy`, `policy_denied`, `proof`, `budget`, `replay_cache`, `descriptor`,
  `duplicate`）に記録される。ブロック検証は
  `AxtEnvelopeValidationFailed { message, reason, snapshot_version }` を返し、
  拒否がどのポリシー snapshot に結びつくかを明示する。
- `/v2/debug/axt/cache` は `{ policy_snapshot_version, last_reject, cache, hints }`
  を返す。`last_reject` には直近の拒否 (lane/理由/版) が入り、`hints` は
  `next_min_handle_era`/`next_min_sub_nonce` を提供する。
- アラートテンプレート: `iroha_axt_policy_reject_total{reason="manifest"}` または
  `{reason="expiry"}` が 5 分窓で急増したらページし、Torii の debug エンドポイント
  から取得した `last_reject` と `policy_snapshot_version` を添付し、ヒントに従って
  handle 更新を依頼する。

## Proof Verification Objects (PVOs)

### 構造

PVO は Norito エンコードのエンベロープで、重い作業を事前に証明できる。
正規フィールドは以下の通り:

| フィールド | 説明 |
|-------|-------------|
| `circuit_id` | 証明システム/ステートメントの静的識別子（例: `amx.transfer.v1`）。 |
| `vk_hash` | DS manifest が参照する検証鍵の Blake2b-256 ハッシュ。 |
| `proof_digest` | オフスロット PVO レジストリに保存されるシリアライズ済み証明の Poseidon digest。 |
| `max_k` | AIR ドメインの上限。ホストは宣言サイズを超える証明を拒否する。 |
| `expiry_slot` | 期限切れ後の無効化スロット高。 |
| `profile` | 省略可能ヒント（DS profile A/B/C など）。 |

Norito スキーマは `crates/iroha_data_model/src/nexus` にあり、SDK は serde を使わず
導出できる。

### 生成パイプライン

1. **回路メタデータのコンパイル** — `circuit_id`、検証鍵、最大トレースサイズを
   prover ビルドから抽出する（通常 `fastpq_prover` レポート）。
2. **証明アーティファクト生成** — スロット外 prover を走らせ、完全な transcript と
   commitment を保存する。
3. **証明サービスへ登録** — Norito PVO をオフスロット verifier に送信し、検証後に
   digest を固定し Torii から handle を提供する。
4. **トランザクションで参照** — AMX builders（`amx_touch` 等）に PVO handle を添付。
   ホストは digest を参照し、キャッシュ結果を検証し、キャッシュが冷たい場合のみ
   スロット内再検証を行う。
5. **失効でローテーション** — SDK は `expiry_slot` 前に handle を更新する。
   期限切れは `PVO_MISSING_OR_EXPIRED` を返す。

### 開発者チェックリスト

- 正確な read/write セットを宣言し、AMX がロックを先取りできるようにする。
- クロス DS 転送で規制 DS に触れる場合は、同じ UAID manifest 更新に
  決定論的 allowance 証明を同梱する。
- リトライ戦略: missing availability evidence は無操作（mempool に残る）。
  `AMX_TIMEOUT`/`PVO_MISSING_OR_EXPIRED` はアーティファクトを再生成し指数的
  backoff を使う。
- テストは cache hit と cold start の両方を含め、同じ `max_k` でホストが検証できる
  ことを確認する。
- Proof blobs（`ProofBlob`）は **必ず** `AxtProofEnvelope { dsid, manifest_root,
  da_commitment?, proof }` をエンコードする。ホストは Space Directory の manifest
  root に結び付けて証明をキャッシュし、`iroha_axt_proof_cache_events_total{event="hit|miss|expired|reject|cleared"}`
  で pass/fail を記録する。期限切れや manifest 不一致は commit 前に拒否され、同一
  スロットの再試行はキャッシュされた `reject` を再利用する。
- 証明キャッシュはスロットスコープ。検証済み証明は同一スロットで共有され、
  スロットが進むと自動で破棄される。

### 静的 read/write 解析

コンパイル時の selectors は契約の実動作と一致する必要がある。新しい
`ivm::analysis` モジュール（`crates/ivm/src/analysis.rs`）は
`analyze_program(&[u8])` を提供し、`.to` アーティファクトをデコードして
レジスタ R/W、メモリ操作、syscall 使用を集計し、JSON フレンドリーなレポートを
生成する。UAID 発行時に `koto_lint` と併用し、R/W サマリを証跡に含める。

## Space Directory ポリシー強制

AXT handle 検証は、ホストが Space Directory snapshot にアクセスできる場合はそれを
デフォルトで使用する（テストの CoreHost、統合フローの WsvHost）。各 dataspace の
ポリシーエントリは `manifest_root`, `target_lane`, `min_handle_era`,
`min_sub_nonce`, `current_slot` を持つ。ホストは以下を強制する:

- lane binding: handle の `target_lane` が Space Directory の値と一致すること。
- manifest binding: 0 以外の `manifest_root` が handle の `manifest_view_root` と一致すること。
- epoch gating: `handle_era` ≥ `min_handle_era` かつ `sub_nonce` ≥ `min_sub_nonce`。
- slot expiry: `expiry_slot` ≥ `current_slot`（許容 skew を考慮）。
- policy gating: `policy` が `AxtPolicyKind` と一致しない handle は拒否。
- proof gating: manifest が証明を要求する場合、証明なし handle は拒否。
- replay gating: 保持ウィンドウ内の同一 handle 再利用は拒否。

いずれかの条件が失敗するとホストは `PermissionDenied` を返し、対応する
`AxtRejectReason` が記録されるため、クライアントは決定論的な失敗理由を
表示できる。
