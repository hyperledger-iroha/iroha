<!-- Japanese translation of docs/source/references/configuration.md -->

---
lang: ja
direction: ltr
source: docs/source/references/configuration.md
status: complete
translator: manual
---

# 設定概要

このページでは Iroha の設定ファイル（TOML）のトップレベルセクションとそれぞれの役割をまとめます。具体的なテンプレートは `docs/source/references/peer.template.toml` を参照してください。

まずはデフォルト。Iroha の設定値は一般的なブロックチェーン運用向けに選定されています。多くの場合はデフォルトのままで動かせます。特定の運用要件（例: 独自ポート、厳格なレート制限、既知のネットワーク制約）がある場合のみ調整してください。

- Sora 固有のセクション（`torii.sorafs`、`sorafs.*`、複数レーンの `nexus.*`）は `irohad --sora …`（または `IROHA_SORA_PROFILE=1`）で起動しないと使用できません。フラグなしでこれらを有効にするとローダが設定を拒否します。
- `nexus.enabled = false`（Iroha 2 モード）の場合、設定は単一レーンのままです。`/status` と `/v1/sumeragi/status` はレーン/データスペースのセクションを隠し、`/metrics` はレーン/データスペースのラベルを一切付けません。レーン指定のステータステイルは `StatusSegmentNotFound` を返すため、SDK はレーンのパラメータを送る前にフラグで分岐してください。このモードではレーン/データスペース/ルーティングの上書きを拒否します。`nexus.enabled=true` か `--sora` で Nexus/Sora を有効化しない限り、`nexus.*` セクションは削除してください。
- `[common]`: チェーン識別子（`chain`）、ノード鍵ペア、信頼済みピア、アカウントドメインの暗黙ラベル（`default_account_domain_label`）、IH58 チェーン識別子（`chain_discriminant`）。ジェネシスのドメインやネットワークプレフィクスがデフォルトと異なる場合はここを上書きし、IH58/圧縮アドレスを正規化してください。
- バリデータの受け入れ: `trusted_peers` には BLS‑Normal のバリデータ鍵を列挙し、PoP は `[[trusted_peers_pop]] { public_key, pop_hex }` に置きます。PoP はロスター全体をカバーする必要があり、不足/無効な PoP は設定パースで拒否されます。設定パースは旧 `trusted_peers_bls` マッピングと非 BLS バリデータ鍵も拒否します。トランスポートの識別（P2P/Torii）は引き続き `public_key`/`private_key` を使いますが、コンセンサス署名は BLS キー + PoP のみを使用します。
- アイデンティティ分離: `public_key`/`private_key` はトランスポート（P2P, Torii）を保護し、`allowed_signing` に関係なく Ed25519/SECP などで構いません。コンセンサスは `trusted_peers` の BLS‑Normal キーと PoP を使用します。トランザクションの受け入れだけが `crypto.allowed_signing`/`allowed_curve_ids` の影響を受けます。BLS 署名トランザクションを送信したい場合は、`allowed_signing` に `bls_normal` を追加し、対応する curve id を `crypto.curves.allowed_curve_ids` に設定してください。`allowed_signing` を Ed25519/secp のデフォルトにしても、BLS バリデータのコンセンサスは動作します。
- `[genesis]`: 署名済みジェネシスファイルの場所と検証用の公開鍵。参照マニフェストには `transaction.max_signatures`、`transaction.max_instructions`、`transaction.ivm_bytecode_size`、`transaction.max_tx_bytes`、`transaction.max_decompressed_bytes` といったパラメータが含まれます。入場制限を強めるときはマニフェストも更新してください。ローカル `genesis.file` がない空のストレージで起動するノードは、`expected_hash` を任意にピン留めし、`bootstrap_allowlist`（デフォルト `trusted_peers`）を指定し、`bootstrap_max_bytes` と `bootstrap_response_throttle_ms` で転送を制限することで、信頼済みピアからジェネシスを取得できます。bootstrap は `bootstrap_request_timeout_ms`、`bootstrap_retry_interval_ms`、`bootstrap_max_attempts`、`bootstrap_enabled` を尊重し、レスポンダは chain‑id/公開鍵/ハッシュの一致を必ず検証し、過大なペイロードや allowlist 外のピアを拒否します。
- `[network]`: P2P アドレス、gossip の周期/サイズ、DNS リフレッシュ、キュー容量の上限、トランスポートオプション（TLS/QUIC など）。
- `[ivm]`: VM 実行時の設定。`ivm.memory_budget_profile`（デフォルト: `compute.default_resource_profile`）は、ゲストスタック上限 `max_stack_bytes` を提供する計算リソースプロファイルを選択します。
- `[tiered_state]`: WSV のホット/コールド階層設定（`enabled`, `hot_retained_keys`, `hot_retained_bytes`, `hot_retained_grace_snapshots`, `cold_store_root`, `da_store_root`, `max_snapshots`, `max_cold_bytes`）。`cold_store_root` が未設定ならスナップショットは `da_store_root` に保存され、いずれかのルートを変更するとティアリングのメタデータがリセットされます。
- `[ivm.banner]`（デフォルト: `show = true`, `beep = true`）: Norito/IVM の起動バナーと任意のジングルを制御します。`beep` はバイナリが `beep` フィーチャでビルドされているときのみ有効です。これは旧 `IROHA_BEEP` 環境変数の置き換えです。バナー/ビープは環境変数ではなく設定で制御してください。
- `[concurrency]`: スレッドプール規模と scheduler/prover ワーカーおよび IVM ゲストのスタック予算。`gas_to_stack_multiplier`（デフォルト `4`）は `min(guest_stack_bytes, gas_limit * multiplier, stack_budget)` でゲストスタックを制限します（最小 64 KiB、最大 1 GiB）。`stack_budget` は `ivm.memory_budget_profile` に由来します。範囲外はクランプされ、ログと `/status.stack` および `iroha_ivm_stack_*` メトリクス（pool‑fallback/budget‑hit カウンタ含む）に出力されます。低 gas ルートの再帰を制限したい場合は multiplier を下げ、より深い Kotodama フレームを許可したい場合は（ゲストスタック上限の範囲で）増やしてください。
- `network.peer_gossip_period_ms`（デフォルト: `250`ms）はトポロジ更新の間隔にも使われます。`IROHA_P2P_TOPOLOGY_UPDATE_MS` 環境変数の上書きは無視されるため、設定で調整してください。`block_gossip_period_ms`、`transaction_gossip_period_ms`、`peer_gossip_period_ms` と `idle_timeout_ms` は 100ms 以上にクランプされます。
- `network.trust_gossip`（デフォルト: `true`）は TrustGossip フレームの対応を広告します。`false` にするとハンドシェイクで能力を無効化し、TrustGossip メッセージをドロップしつつピアアドレス gossip は維持します。ドロップは `p2p_trust_gossip_skipped_total{direction,reason}` でタグ付けされ、opt‑out や拒否したピアを検知できます。
- `[network]` の信頼スコア設定（`trust_decay_half_life_ms`, `trust_penalty_bad_gossip`, `trust_penalty_unknown_peer`, `trust_min_score`）は permissioned overlay で有効です。アクティブトポロジ外のピアを参照する trust フレームは unknown‑peer ペナルティを受け、`p2p_trust_penalties_total{reason="unknown_peer"}` を増加させます。`trust_min_score` 未満の送信者は decay で回復するまで無視されます。Public/NPoS overlay では unknown‑peer ペナルティを適用しません。
- トランザクション gossip の調整: `network.transaction_gossip_drop_unknown_dataspace` は lane カタログに存在しない dataspace へのバッチをドロップします。`network.transaction_gossip_restricted_target_cap` は restricted fanout（デフォルトは commit topology 全体）を制限します。`network.transaction_gossip_public_target_cap` は public レーン fanout（デフォルト 16、`null` でブロードキャスト）を制限します。`network.transaction_gossip_public_target_reshuffle_ms` と `network.transaction_gossip_restricted_target_reshuffle_ms` はターゲット再選定の周期を制御します（デフォルト `transaction_gossip_period_ms`、最低 100ms）。`network.transaction_gossip_resend_ticks` は同一トランザクションの再送までに待つ gossip 周期数を指定します（値を大きくすると重複送信が減ります）。`network.transaction_gossip_restricted_fallback`（`drop` | `public_overlay`）は commit topology が空のときに restricted gossip を online peer にフォールバックするかどうかを制御します。`network.transaction_gossip_restricted_public_payload`（`refuse` | `forward`, デフォルト `refuse`）は public overlay しかない場合の restricted payload の転送を拒否します（暗号化トランスポートが来るまで forward は無効）。テレメトリは drop/forward ポリシーを `/status.tx_gossip` と各種ゲージで可視化します。
- `network.soranet_handshake.pow`: SoraNet 入場チャレンジの設定。`required`, `difficulty`, `max_future_skew_secs`, `min_ticket_ttl_secs`, `ticket_ttl_secs`, `revocation_store_capacity`, `revocation_max_ttl_secs`, `revocation_store_path`を指定します。revocation store は署名済みチケットのディスク保持数と有効期間を制御します。
- `network.soranet_handshake.pow.puzzle`: Argon2 パズルゲート。`enabled = true` でメモリハードチケットを要求し、`memory_kib`（>=4096）、`time_cost`、`lanes`（1–16）を調整します。有効時も `difficulty`, `max_future_skew_secs`, `min_ticket_ttl_secs` は尊重されます。無効化すると従来の hashcash PoW に戻ります。
- `network.soranet_handshake.kem_suite`: ML‑KEM プロファイルのテキスト上書き（`"mlkem512"`, `"mlkem768"`, `"mlkem1024"`。`kyber*` も可）。指定すると `kem_id` を上書きし、指定プロファイルに適合する Kyber 鍵長を検証します。
- `[sumeragi]`: コンセンサスの役割（validator/observer）とコレクタ設定（K, redundant‑send r）およびタイミング調整。DA ゲートは `da_enabled` で availability evidence または RBC `READY` quorum の到達まで commit を遅延します（ローカル RBC 配達は待ちません）。キューごとの backpressure は `msg_channel_cap_votes`, `msg_channel_cap_block_payload`, `msg_channel_cap_rbc_chunks`, `msg_channel_cap_blocks`, `control_msg_channel_cap` で設定します。旧 `msg_channel_cap` は per‑queue caps が無い場合のみフォールバックです。`mode_flip_enabled`（デフォルト `true`）は permissioned↔NPoS のライブ切替を抑止する kill switch で、`false` の間はステージング状態を広告するだけで切替しません。永続化失敗は `kura_store_retry_interval_ms`/`kura_store_retry_max_attempts` に従って backoff し、`commit_inflight_timeout_ms` が詰まった commit ジョブを中断します。`missing_block_signer_fallback_attempts` 回は commit certificate 署名者優先（デフォルト 1、0 で無効化）で不足ブロックを取得し、その後 commit topology 全体にフォールバックします。メンバーシップのハッシュ乖離は `membership_mismatch_alert_threshold`（連続ミスマッチ数でアラート）と `membership_mismatch_fail_closed`（ミスマッチが続くピアのコンセンサスメッセージをドロップ）で可視化されます。pacemaker の提案間隔は `npos.block_time_ms`, `npos.timeouts.propose`, `pacemaker_rtt_floor_multiplier` から導出され `pacemaker_max_backoff_ms` により上限がかかります。INIT 前の RBC スタッシュはセッション横断で `sumeragi.rbc_pending_session_limit` により上限が設定されます。RBC READY/DELIVER の quorum は常に commit topology（`Topology::min_votes_for_commit()`）に従います。
- `[torii]`: 公開 API サーバの設定（listen アドレス、リクエストサイズ上限、クエリストアなど）。
- `torii.data_dir`（デフォルト: `./storage/torii`）: Torii の保存場所（添付、webhook レジストリ、DA キャッシュ）。旧 `IROHA_TORII_DATA_DIR` を置き換えます。テストでは `data_dir::OverrideGuard` で一時上書きできます。
- `torii.events_buffer_capacity`（デフォルト: `10000`）: `/v1/events/sse` と webhook キューの broadcast チャネル容量。遅延がある場合は低く設定してメモリを制御します。
- `torii.ws_message_timeout_ms`（デフォルト: `10000`）: Torii のイベント/ブロック WebSocket ストリームに適用するメッセージ read/write タイムアウト（ミリ秒）。遅いクライアントや高遅延リンクでは増やしてください。
- `torii.app_api.*`: JSON 便利 API のページング/バックプレッシャ設定。`default_list_limit` は `limit` の初期値、`max_list_limit` と `max_fetch_size` が上限、`rate_limit_cost_per_row` が行コストです。
- `torii.webhook.*`: webhook 配信の backoff 設定。`queue_capacity`（デフォルト 10000）でオンディスク待ち数、`max_attempts`（デフォルト 12）でリトライ数、`backoff_initial_ms`/`backoff_max_ms`（デフォルト 1000/60000）で指数バックオフ、`{connect,write,read}_timeout_ms` が HTTP タイムアウトです。
- `torii.push.*`（feature stub）: Push 通知ブリッジ。`enabled` で有効化し、`rate_per_minute`/`burst` でトークンバケット、`max_topics_per_device` で購読上限、`connect_timeout_ms`/`request_timeout_ms` で HTTP 制限。`fcm_api_key` または APNS `apns_endpoint` + `apns_auth_token` を設定します。
- `torii.da_ingest.*`: DA パイプラインのリプレイキャッシュとマニフェストスプール。
  - `telemetry_cluster_label`（任意）: Taikai ingest/viewer メトリクスの `cluster` ラベル。`region-a` と `staging` のように環境を区別できます。
    `governance_metadata_key_hex` に 32 バイトの ChaCha20‑Poly1305 キーを設定するとガバナンス専用メタデータを封印できます。`governance_metadata_key_label`（例: `"primary"`）と組み合わせると、暗号化されたエントリにキーラベルが表示されます。Torii は平文のガバナンスメタデータを自動暗号化し、ラベル欠落/不一致の pre‑encrypted payload を拒否します。`replication_policy` は blob クラスごとの `RetentionPolicy` を定義します（`docs/source/da/replication_policy.md` 参照）。`default_retention` で基準の hot/cold 窓を変更し、`replication_policy.overrides` に `class = "taikai_segment"` などを追加してクラス固有の厳格化が可能です。Torii はマニフェストを再書き込みし不一致を拒否します。
  - `debug_match_filters`（デフォルト: `false`）は旧 `TORII_DEBUG_MATCH` で制御していたフィルタのデバッグトレースを有効化します。プロダクションでは無効にしてください。
- `[torii.iso_bridge.reference_data]`: ISO 参照データ（`isin_crosswalk_path`, `bic_lei_path`, `mic_directory_path`）のパスと `refresh_interval_secs`、および任意の `cache_dir`（スナップショットのバージョン/ソース/チェックサム付きキャッシュ）。
- `torii.strict_addresses`（デフォルト: `true`）: 有効時は `alias@domain` や生の公開鍵を全て拒否して `ERR_STRICT_ADDRESS_REQUIRED` を返します。診断時の dev/test に限り `false` にし、改善後は `true` に戻してください。
- `[torii.soranet_privacy_ingest]`: `/v1/soranet/privacy/{event,share}` のガード。`enabled` は `true` 必須。`require_token` + `tokens` で `X-SoraNet-Privacy-Token`（または `X-API-Token`）を強制します。`allow_cidrs` が送信者を CIDR 制限し、`rate_per_sec`/`burst` がトークンバケットを適用します。拒否は 401/403/429 と retry ヒントを返し、テレメトリカウンタも増えます。`docs/source/sorafs_authz_runbook.md` のチェックリストを参照してください。
- `[torii.sorafs]`: Torii 側の SoraFS discovery キャッシュ。値は `[sorafs.*]` からコピーされるため root セクションで設定します。
- `[sorafs.discovery]`: discovery のトグルと能力 allow‑list。`discovery_enabled` はデフォルト `false`（Sora プロファイルが明示的に有効化）、`known_capabilities` は `["torii_gateway", "chunk_range_fetch", "vendor_reserved"]`、`[sorafs.discovery.admission]` は `envelopes_dir` を参照します。
- `[sorafs.storage]`: 組み込み SoraFS ワーカー設定。`enabled`, `data_dir`, `max_capacity_bytes`, `max_parallel_fetches`, `max_pins`, `por_sample_interval_secs`, optional `alias` と、必要に応じて `governance_dag_dir` を設定します。`[sorafs.storage.adverts]` は `stake_pointer`, `availability`, `max_latency_ms`, `topics` を提供し、`torii.sorafs_storage` に反映されます。
- `[sorafs.storage.pin]`: `/v1/sorafs/storage/pin` の認証/レート制限。`require_token=true` は bearer トークン（`Authorization` または `X-SoraFS-Pin-Token`）を必須化。`tokens` が allow‑list、`allow_cidrs` が CIDR 制限、`rate_limit` がトークンバケット（任意の ban 付き）です。失敗時は 401/403/429 と `Retry-After`。
- `[sorafs.repair]`: 修復スケジューラ設定。キー: `enabled`（既定 `false`）、任意の `db_dsn`、`db_pool_max_connections`（既定 `8`）、`claim_ttl_secs`（既定 `900`）、`heartbeat_interval_secs`（既定 `60`）、`max_attempts`（既定 `3`）、`worker_concurrency`（既定 `4`）、`backoff_initial_secs`（既定 `5`）、`backoff_max_secs`（既定 `60`）。
- `[sorafs.gc]`: GC スケジューラ設定。キー: `enabled`（既定 `false`）、任意の `db_dsn`、`db_pool_max_connections`（既定 `4`）、`interval_secs`（既定 `900`）、`max_deletions_per_run`（既定 `500`）、`retention_grace_secs`（既定 `86400`）、`pre_admission_sweep`（既定 `true`）。
- `[sorafs.quota]`: control‑plane クォータ（容量宣言、storage pin、telemetry、deal telemetry、dispute、PoR 提出）。`torii.sorafs_quota` は値をそのまま反映します。
- `[sorafs.alias_cache]`: alias‑proof キャッシュポリシー（`positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`）。デフォルトは 10 分 / 2 分 / 15 分 / 60 秒 / 5 分 / 6 時間。`torii.sorafs_alias_cache` に同じ値が反映されます。
- `sorafs.rollout_phase`: PQ ロールアウトのフェーズ（`canary`, `ramp`, `default`）。`sorafs.anonymity_policy` が未設定の場合はこれを継承します。
- `sorafs.anonymity_policy`: SoraNet PQ ロールアウト段階（`anon-guard-pq`, `anon-majority-pq`, `anon-strict-pq`）。デフォルトは Stage A（`anon-guard-pq`）。カバレッジが十分になったら Stage B/C へ。
- `[nts]`: Network Time Service のサンプリング周期と平滑化。`sample_interval_ms` は 100ms 以上に制限されます。コンセンサスは依存しません。
- `[kura]`: ブロックストレージのパスとメモリウィンドウ。`block_sync_roster_retention` は commit‑roster スナップショットの保持数（デフォルト 7200）、`roster_sidecar_retention` は `pipeline/roster_sidecars.*` に残る sidecar の数（デフォルト 512）。
- `[logger]`: ログレベルとフォーマット。
- `[queue]`: トランザクションキューの制限と TTL。
- `[nexus]`: マルチレーンのルーティングカタログ、マニフェストレジストリ、ガバナンスモジュール。レーンは `visibility`, `lane_type`, `governance`, `settlement`, `proof_scheme`（デフォルト `merkle_sha256`。`kzg_bls12_381` レーンは deterministic KZG commitments）と `metadata = { key = "value" }` を受け入れます。`registry` は `manifest_directory`, `cache_directory`, `poll_interval_ms` を管理し、`governance` は `default_module` と `modules` マップを提供します。
  - `da_manifest_policy` はレーンごとの DA マニフェスト適用。`strict` がデフォルト。`audit`/`audit_only`/`warn` は missing を警告付きで許可します（ハッシュ不一致は致命）。
  - `nexus.staking.public_validator_mode` / `nexus.staking.restricted_validator_mode` はレーンのバリデータライフサイクルを選択します。`stake_elected` は staking/NPoS を維持、`admin_managed` は admin フローで admission。stake‑elected レーンはコミットトポロジに有効なコンセンサス鍵を持つ登録ピアが必要で、エポック境界で有効化します。admin‑managed レーンは genesis roster を維持します。
  - `[nexus.axt]`: `slot_length_ms`（`1..=600_000`）、`max_clock_skew_ms`（<=`slot_length_ms`）、`proof_cache_ttl_slots`（`1..=64`）、`replay_retention_slots`（`1..=4_096`）などの guardrails。例は `crates/iroha_config/tests/fixtures/nexus_axt_full.toml` を参照。
- `[governance]`: 検証鍵、conviction パラメータ、承認閾値、参加率下限、評議会サイズ/資格。
  - `debug_tie_break` / `debug_tx_eval` はデバッグ用 overlay トレース。
- `[norito]`: Norito シリアライズ調整。`max_archive_len` のデフォルトは 512 MiB で、`sumeragi.rbc_store_max_bytes` と `network.max_frame_bytes` 以上にクランプして合意ペイロードのデコードを保証します。
- `[streaming]`: Norito streaming の control‑plane 鍵設定。
- `[streaming.soranet]`: SoraNet ブリッジ設定（`enabled`, `exit_multiaddr`, `padding_budget_ms`, `access_kind`, `channel_salt`, `provision_spool_dir`, `provision_spool_max_bytes`, `provision_window_segments`, `provision_queue_capacity`）。
- `[streaming.soravpn]`: SoraVPN のローカルプロビジョニングスプール設定（`provision_spool_dir`, `provision_spool_max_bytes`）。
- `[relay]` と `exit_routing.*` は上記の通り。
ノート
- ハードウェアアクセラレーションはデフォルトで自動です。最初の使用時に golden self‑tests が実行され、ミスマッチがあれば無効化されます。
- `peer.template.toml` 形式のファイルを編集し、デプロイで拡張することを推奨します。運用時の挙動は `iroha_config` で制御し、環境変数は開発用途のみにしてください。
