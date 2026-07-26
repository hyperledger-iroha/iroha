---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/orchestrator-config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 679b05638ff936a1286ae087a631652b8524b7dc8e30795bb2ad11f50cd7e10c
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: orchestrator-config
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
このページは `docs/source/sorafs/developer/orchestrator.md` を反映しています。レガシーのドキュメントが退役するまで、両方のコピーを同期したままにしてください。
:::

# マルチソース fetch オーケストレーター ガイド

SoraFS のマルチソース fetch オーケストレーターは、ガバナンスに裏付けられた
adverts で公開されたプロバイダー集合から、決定的かつ並列にダウンロードを
実行します。このガイドでは、オーケストレーターの設定方法、ロールアウト時に
想定すべき失敗シグナル、ヘルス指標を示すテレメトリストリームを説明します。

## 1. 設定概要

オーケストレーターは 3 つの設定ソースを統合します。

| ソース | 目的 | 備考 |
|--------|------|------|
| `OrchestratorConfig.scoreboard` | プロバイダー重みの正規化、テレメトリの鮮度検証、監査用 JSON scoreboard の永続化。 | `crates/sorafs_car::scoreboard::ScoreboardConfig` によって支えられます。 |
| `OrchestratorConfig.fetch` | ランタイム制限（retry 予算、並列数上限、検証トグル）を適用。 | `crates/sorafs_car::multi_fetch` の `FetchOptions` に対応。 |
| CLI / SDK パラメータ | ピア数の上限、テレメトリ地域の付与、deny/boost ポリシーの露出。 | `sorafs_cli fetch` が直接フラグを提供し、SDK は `OrchestratorConfig` を経由して渡します。 |

`crates/sorafs_orchestrator::bindings` の JSON ヘルパーは、設定全体を Norito
JSON にシリアライズし、SDK バインディングと自動化の間で持ち運べるようにします。

### 1.1 サンプル JSON 設定

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

`iroha_config` の通常のレイヤー（`defaults/`, user, actual）を通じてファイルを
永続化し、全ノードで同一の制限を継承するようにします。SNNet-5a のロールアウトに
合わせた direct-only の fallback プロファイルについては
`docs/examples/sorafs_direct_mode_policy.json` と
`docs/source/sorafs/direct_mode_pack.md` を参照してください。

### 1.2 コンプライアンスのオーバーライド

SNNet-9 はガバナンス主導のコンプライアンスをオーケストレーターに組み込みます。
Norito JSON 設定に新設された `compliance` オブジェクトが、fetch パイプラインを
direct-only に強制する carve-out を保持します。

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` は、このオーケストレーターが稼働する ISO‑3166
  alpha‑2 コードを宣言します。パース時に大文字へ正規化されます。
- `jurisdiction_opt_outs` はガバナンス・レジストリを反映します。運用地域のいずれかが
  このリストに含まれる場合、`transport_policy=direct-only` が強制され、
  `compliance_jurisdiction_opt_out` の fallback 理由が出力されます。
- `blinded_cid_opt_outs` は、マニフェスト digest（blinded CID、16 進大文字）を
  列挙します。一致する payload は direct-only のスケジューリングを強制し、
  `compliance_blinded_cid_opt_out` をテレメトリに露出します。
- `audit_contacts` は、ガバナンスがオペレーターの GAR playbook に掲載を求める URI を
  記録します。
- `attestations` は、ポリシーを支える署名済みコンプライアンス・パケットを収録します。
  各エントリは任意の `jurisdiction`（ISO‑3166 alpha‑2）、`document_uri`、
  64 文字の `digest_hex`、発行時刻 `issued_at_ms`、任意の `expires_at_ms` を定義します。
  これらのアーティファクトは監査チェックリストに流れ、ガバナンスのツールが
  署名書類とオーバーライドを紐付けられるようにします。

コンプライアンス・ブロックは通常の設定レイヤーで提供し、運用側に決定的な
オーバーライドが適用されるようにしてください。オーケストレーターは write-mode
ヒントの _後_ にコンプライアンスを適用します。SDK が `upload-pq-only` を要求しても、
地域やマニフェストの opt-out は direct-only へのフォールバックを強制し、
適合するプロバイダーが無い場合は即座に失敗します。

正規の opt-out カタログは `governance/compliance/soranet_opt_outs.json` にあり、
Governance Council がタグ付きリリースで更新を公開します。attestations を含む完全な
設定例は `docs/examples/sorafs_compliance_policy.json` にあり、運用プロセスは
[GAR コンプライアンス・プレイブック](../../../source/soranet/gar_compliance_playbook.md)
にまとめられています。

### 1.3 CLI & SDK のノブ

| フラグ / フィールド | 効果 |
|--------------------|------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | scoreboard フィルタを通過するプロバイダー数を制限。`None` で全 eligible を使用。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | チャンク単位の retry 上限。超過すると `MultiSourceError::ExhaustedRetries`。 |
| `--telemetry-json` | レイテンシ/失敗率のスナップショットを scoreboard に注入。`telemetry_grace_secs` を超えるテレメトリは不適格扱い。 |
| `--scoreboard-out` | 計算済み scoreboard（eligible + ineligible）を保存。 |
| `--scoreboard-now` | scoreboard タイムスタンプ（Unix 秒）を上書きし、fixtures の再現性を維持。 |
| `--deny-provider` / score policy hook | adverts を削除せずにプロバイダーを決定的に除外。迅速なブラックリストに有効。 |
| `--boost-provider=name:delta` | ガバナンスの重みを変えずに、プロバイダーの weighted round-robin クレジットを調整。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | メトリクス/構造化ログにリージョンラベルを付与し、ダッシュボードを地理/波で切り替え可能にする。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | マルチソースが標準化されたため、既定は `soranet-first`。ダウングレードやコンプライアンス指示には `direct-only`、PQ-only パイロットには `soranet-strict` を使用。コンプライアンスのオーバーライドは常にハード上限。 |

SoraNet-first は出荷時のデフォルトであり、ロールバックは該当する SNNet ブロッカーを
明記する必要があります。SNNet-4/5/5a/5b/6a/7/8/12/13 が昇格した後は、
ガバナンスが必要姿勢をより厳格に（`soranet-strict` へ）引き上げます。そこまでは、
`direct-only` を優先するのはインシデント由来の override のみに限り、rollout ログに
記録してください。

上記のフラグは `sorafs_cli fetch` と開発者向けの `sorafs_fetch` バイナリの両方で
`--` 形式に対応します。SDK は同等のオプションを型付き builder で提供します。

### 1.4 Guard キャッシュ管理

CLI は SoraNet の guard セレクターを組み込み、SNNet-5 の本格ロールアウト前に
エントリー・リレーを決定的にピンできるようになりました。新しい 3 つのフラグで
ワークフローを制御します。

| フラグ | 目的 |
|--------|------|
| `--guard-directory <PATH>` | 最新のリレー合意を記述した JSON を指す（下に一部）。指定すると fetch 前に guard cache を更新。 |
| `--guard-cache <PATH>` | Norito でエンコードした `GuardSet` を永続化。新しいディレクトリが無くても再利用。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | エントリー guard の数（既定 3）と保持期間（既定 30 日）の任意 override。 |
| `--guard-cache-key <HEX>` | Blake3 MAC で guard cache にタグを付ける任意の 32 バイトキー。再利用前に検証可能。 |

Guard directory の payload はコンパクトなスキーマです。

`--guard-directory` は Norito エンコードされた `GuardDirectorySnapshotV2`
payload を期待します。バイナリ snapshot には次が含まれます。

- `version` — スキーマバージョン（現在 `2`）。
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  すべての埋め込み証明書と一致すべき consensus メタデータ。
- `validation_phase` — 証明書ポリシーゲート（`1` = 単一 Ed25519 署名を許可、
  `2` = 二重署名を優先、`3` = 二重署名を必須）。
- `issuers` — `fingerprint`, `ed25519_public`, `mldsa65_public` を持つガバナンス発行者。
  Fingerprint は `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`
  で計算されます。
- `relays` — SRCv2 bundle のリスト（`RelayCertificateBundleV2::to_cbor()` 出力）。
  各 bundle は relay descriptor、capability flags、ML-KEM ポリシー、Ed25519/ML-DSA-65
  の二重署名を含みます。

CLI は declared issuer keys で各 bundle を検証してから directory を guard cache と
マージします。旧来の JSON スケッチは受け付けず、SRCv2 snapshot が必須です。

`--guard-directory` を指定して最新の consensus を既存 cache と統合します。セレクターは
保持期間内で directory 上も eligible な guard を保持し、期限切れは新しい relay で
置き換えます。fetch 成功後は更新済み cache が `--guard-cache` のパスへ書き戻され、
次回も決定的になります。SDK は
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` を呼び、
得られた `GuardSet` を `SorafsGatewayFetchOptions` に渡すことで同じ挙動を再現できます。

`ml_kem_public_hex` は SNNet-5 rollout 中に PQ 対応 guard を優先させます。ステージ
トグル（`anon-guard-pq`, `anon-majority-pq`, `anon-strict-pq`）はクラシック relay を
自動的に降格させます。PQ guard が利用可能な場合、余分なクラシック pin を削除し、
以降のセッションでハイブリッド handshake を優先します。CLI/SDK のサマリーは
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` と、候補/不足/供給差分の各フィールドで mix を公開し、
brownout やクラシック fallback を明示します。

Guard directory は `certificate_base64` を通じて完全な SRCv2 bundle を内包できます。
オーケストレーターは各 bundle をデコードして Ed25519/ML-DSA 署名を再検証し、解析済み
証明書を guard cache と共に保持します。証明書が存在する場合、それが PQ keys、
handshake 設定、重み付けの正規ソースになります。期限切れの証明書は破棄し、
管理にも反映され、`telemetry::sorafs.guard` と `telemetry::sorafs.circuit` で
有効期間、handshake スイート、二重署名の有無が記録されます。

CLI ヘルパーで snapshot を発行元と同期させます。

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` は SRCv2 snapshot を検証してから書き込みます。`verify` は他チーム由来の
アーティファクトに対して検証パイプラインを再実行し、CLI/SDK の guard セレクター出力に
一致する JSON サマリーを生成します。

### 1.5 Circuit ライフサイクル・マネージャ

relay directory と guard cache の両方が提供されると、オーケストレーターは
circuit ライフサイクル・マネージャを有効化し、各 fetch 前に SoraNet 回線を事前構築・
更新します。設定は `OrchestratorConfig`（`crates/sorafs_orchestrator/src/lib.rs:305`）の
新しい 2 つのフィールドで行います。

- `relay_directory`: SNNet-3 の directory snapshot を保持し、middle/exit hops を
  決定的に選定します。
- `circuit_manager`: 回線 TTL を制御するオプション設定（既定で有効）。

Norito JSON は `circuit_manager` ブロックを受け付けます。

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK は `SorafsGatewayFetchOptions::relay_directory` を通じて directory を渡し
（`crates/iroha/src/client.rs:320`）、CLI は `--guard-directory` が指定されると自動で
配線します（`crates/iroha_cli/src/commands/sorafs.rs:365`）。

マネージャは guard のメタデータ（endpoint、PQ key、ピン時刻）が変わった場合や TTL
満了時に回線を更新します。各 fetch 前に呼ばれる `refresh_circuits`
（`crates/sorafs_orchestrator/src/lib.rs:1346`）は `CircuitEvent` ログを出力し、
ライフサイクル判断を追跡できるようにします。soak テスト
`circuit_manager_latency_soak_remains_stable_across_rotations`
（`crates/sorafs_orchestrator/src/soranet.rs:1479`）は 3 回の guard ローテーションでも
安定したレイテンシを示し、報告書は
`docs/source/soranet/reports/circuit_stability.md:1` にあります。

### 1.6 ローカル QUIC プロキシ

オーケストレーターは、ブラウザ拡張や SDK アダプタが証明書や guard cache key を
管理しなくて済むよう、ローカル QUIC プロキシを起動できます。プロキシは loopback で
待ち受け、QUIC 接続を終端し、証明書と guard cache key を記述する Norito manifest を
クライアントへ返します。プロキシが発行する transport イベントは
`sorafs_orchestrator_transport_events_total` でカウントされます。

オーケストレーター JSON の `local_proxy` ブロックでプロキシを有効化します。

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` はプロキシのリッスン先（`0` でエフェメラルポート）。
- `telemetry_label` はメトリクスに伝播し、ダッシュボードでプロキシと fetch セッションを区別。
- `guard_cache_key_hex`（任意）は CLI/SDK と同じ keyed guard cache を公開し、拡張機能の同期を保つ。
- `emit_browser_manifest` は、拡張が保存・検証可能な manifest を返すかどうかを切り替え。
- `proxy_mode` はローカルでブリッジする（`bridge`）か、メタデータのみ返す（`metadata-only`）かを選択。
  既定は `bridge`。manifest の提示だけ必要な場合は `metadata-only`。
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs` は、並列ストリームの
  予算や回線再利用の強さをブラウザに伝えるヒント。
- `car_bridge`（任意）はローカル CAR アーカイブキャッシュ。`extension` は `*.car` が
  省略された際のサフィックス。`allow_zst = true` で `*.car.zst` を直接提供。
- `kaigi_bridge`（任意）は Kaigi の spool ルートを公開。`room_policy` は `public` / `authenticated` を
  宣言し、GAR ラベルの事前選択を可能に。
- `sorafs_cli fetch` は `--local-proxy-mode=bridge|metadata-only` と
  `--local-proxy-norito-spool=PATH` を提供し、JSON を変更せずにモードや spool を切替可能。
- `downgrade_remediation`（任意）は自動ダウングレード hook。relay テレメトリの downgrade
  バーストを監視し、`window_secs` 内の `threshold` 超過で `target_mode`
  （既定 `metadata-only`）に強制。downgrade が止まると `cooldown_secs` 後に `resume_mode`
  に戻ります。`modes` 配列で特定の relay ロールに限定（既定 entry relays）。

プロキシが bridge モードで動作する場合、2 つのアプリケーションサービスを提供します。

- **`norito`** — クライアントの stream target は `norito_bridge.spool_dir` を基準に解決。
  サニタイズ（パストラバーサルや絶対パス禁止）され、拡張子が無い場合は設定サフィックスを付与して
  そのままストリームします。
- **`car`** — stream target は `car_bridge.cache_dir` 内で解決され、既定拡張子を継承。
  `allow_zst` が無い場合は圧縮 payload を拒否。成功時は `STREAM_ACK_OK` を返してから
  アーカイブを転送し、検証のパイプライン化を可能にします。

両方のケースで、プロキシは cache-tag HMAC（guard cache key が存在する場合）を提供し、
`norito_*` / `car_*` のテレメトリ理由コードを記録して、ダッシュボードで成功・欠損・
サニタイズ失敗を識別できるようにします。

`Orchestrator::local_proxy().await` は実行中のハンドルを公開し、証明書 PEM の取得、
ブラウザ manifest の取得、終了時のグレースフルシャットダウン要求に使えます。

プロキシが有効な場合、現在は **manifest v2** を提供します。既存の証明書と guard cache key
に加え、v2 では次を追加します。

- `alpn`（`"sorafs-proxy/1"`）と `capabilities` 配列で、使用すべきストリームプロトコルを確認。
- ハンドシェイクごとの `session_id` と `cache_tagging` ブロックで、セッション単位の guard
  アフィニティと HMAC タグを導出。
- `circuit`, `guard_selection`, `route_hints` による回線・guard 選択のヒント。
- ローカル計測向けの `telemetry_v2`（サンプリング/プライバシー knobs）。
- 各 `STREAM_ACK_OK` には `cache_tag_hex` が含まれ、クライアントは HTTP/TCP リクエストで
  `x-sorafs-cache-tag` ヘッダに反映し、guard selection を暗号化して保持します。

これらは後方互換で、既存クライアントは新しいキーを無視して v1 サブセットに依存できます。

## 2. 失敗セマンティクス

オーケストレーターは、1 バイトも転送する前に厳格な能力/予算チェックを行います。
失敗は次の 3 区分です。

1. **適格性エラー（pre-flight）**。レンジ能力を持たないプロバイダー、期限切れの adverts、
   期限超過のテレメトリは scoreboard アーティファクトに記録され、スケジューリングから除外されます。
   CLI サマリーは `ineligible_providers` 配列に理由を入れ、ログをスクレイプせずに governance
   のズレを確認できます。
2. **ランタイム枯渇**。各プロバイダーは連続失敗数を追跡し、`provider_failure_threshold`
   に達するとセッションの残り期間で `disabled` になります。全プロバイダーが `disabled` に
   なると `MultiSourceError::NoHealthyProviders { last_error, chunk_index }` を返します。
3. **決定的アボート**。ハード制限は構造化エラーとして表出します。
   - `MultiSourceError::NoCompatibleProviders` — マニフェストが要求する chunk span / alignment を
     残りのプロバイダーが満たせない。
   - `MultiSourceError::ExhaustedRetries` — chunk の retry 予算を消費。
   - `MultiSourceError::ObserverFailed` — downstream observers（ストリーミングフック）が検証済み
     chunk を拒否。

各エラーには問題の chunk index と、可能であれば最終的な provider failure reason が含まれます。
同じ入力でのリトライは、advert/テレメトリ/プロバイダー健全性が変わらない限り同じ失敗を再現します。

### 2.1 Scoreboard の永続化

`persist_path` が設定されていると、オーケストレーターは各 run 後に最終 scoreboard を
書き込みます。JSON ドキュメントには次が含まれます。

- `eligibility`（`eligible` または `ineligible::<reason>`）。
- `weight`（当該 run の正規化ウェイト）。
- `provider` メタデータ（識別子、endpoint、並列予算）。

scoreboard snapshot を release アーティファクトと併せて保管し、ブラックリストや
rollout 判断の監査性を維持してください。

## 3. テレメトリとデバッグ

### 3.1 Prometheus メトリクス

オーケストレーターは `iroha_telemetry` を通じて次のメトリクスを出力します。

| メトリクス | Labels | 説明 |
|-----------|--------|------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | 実行中の fetch の gauge。 |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | エンドツーエンドの fetch レイテンシを記録するヒストグラム。 |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | 終端失敗（retry 枯渇、プロバイダー無し、observer 失敗）のカウンタ。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | プロバイダー別 retry 試行回数。 |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | セッション内のプロバイダー失敗（無効化に至る）カウンタ。 |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | 匿名性ポリシー決定（達成/ブラウンアウト）のカウントを stage と reason で集計。 |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | 選択された SoraNet セット内の PQ relay 割合ヒストグラム。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | scoreboard snapshot における PQ relay 供給割合のヒストグラム。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | ポリシー不足（目標と実際の PQ 比率の差）のヒストグラム。 |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | 各セッションのクラシック relay 割合ヒストグラム。 |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | セッションごとのクラシック relay 選択数ヒストグラム。 |

本番ノブを切り替える前に、staging ダッシュボードへ統合してください。推奨レイアウトは
SF-6 観測性計画に準拠します。

1. **Active fetches** — 完了と釣り合わない増加を検知。
2. **Retry ratio** — `retry` カウンタが過去ベースラインを超えたら警告。
3. **Provider failures** — 15 分以内に `session_failure > 0` が発生したらページャー。

### 3.2 構造化ログ ターゲット

オーケストレーターは決定的ターゲットに構造化イベントを発行します。

- `telemetry::sorafs.fetch.lifecycle` — `start`/`complete` のライフサイクル、chunk 数、retry 数、総時間。
- `telemetry::sorafs.fetch.retry` — retry イベント（`provider`, `reason`, `attempts`）。
- `telemetry::sorafs.fetch.provider_failure` — 反復エラーで無効化されたプロバイダー。
- `telemetry::sorafs.fetch.error` — 終端失敗の `reason` と任意のプロバイダーメタデータ。

これらのストリームは既存の Norito ログパイプラインに送ってください。ライフサイクルイベントは
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio` と
付随カウンタにより PQ/クラシックの混在を示すため、メトリクスをスクレイプせずに
ダッシュボード化できます。GA rollout 中は lifecycle/retry を `info`、終端エラーを
`warn` で運用してください。

### 3.3 JSON サマリー

`sorafs_cli fetch` と Rust SDK は次を含む構造化サマリーを返します。

- `provider_reports`（成功/失敗数と無効化 여부）。
- `chunk_receipts`（各 chunk を満たしたプロバイダー）。
- `retry_stats` と `ineligible_providers` 配列。

問題のあるプロバイダーをデバッグする際はサマリーを保管してください。receipts は上記の
ログメタデータに直接対応します。

## 4. 運用チェックリスト

1. **CI で設定をステージング。** `sorafs_fetch` を対象設定で実行し、`--scoreboard-out` で
   eligibility view を保存し、前リリースと差分比較。予期しない ineligible が出たら昇格停止。
2. **テレメトリ検証。** ユーザー向け multi-source fetch を有効化する前に、`sorafs.fetch.*`
   メトリクスと構造化ログが出ることを確認。メトリクス欠落はオーケストレーターが呼ばれていない
   可能性が高い。
3. **オーバーライドの記録。** 緊急 `--deny-provider` / `--boost-provider` を使う場合、JSON
   または CLI 実行を変更ログに残す。ロールバックでは override を解除し、新しい scoreboard snapshot
   を取得。
4. **スモークテスト再実行。** retry 予算や provider caps を変更したら、正規 fixture
   （`fixtures/sorafs_manifest/ci_sample/`）で再 fetch し、chunk receipts が決定的であることを確認。

上記の手順により、段階的 rollouts での再現性と、インシデント対応に必要なテレメトリが確保されます。

### 4.1 ポリシーのオーバーライド

`policy_override.transport_policy` と `policy_override.anonymity_policy` を
`orchestrator` JSON に設定するか、`sorafs_cli fetch` に
`--transport-policy-override=` / `--anonymity-policy-override=` を渡すことで、
ベース設定を編集せずに輸送/匿名性ステージを固定できます。いずれかの override が
設定されると、通常の brownout fallback は実行されず、要求された PQ tier が満たせない場合は
`no providers` で失敗します。デフォルト動作への復帰は override フィールドを空にするだけです。
