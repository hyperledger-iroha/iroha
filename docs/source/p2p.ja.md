<!-- Japanese translation of docs/source/p2p.md -->

---
lang: ja
direction: ltr
source: docs/source/p2p.md
status: complete
translator: manual
---

## P2P キューとメトリクス

本セクションではピアツーピア（P2P）キューの容量設定と監視用メトリクスを説明します。

### キュー容量（`[network]` 設定）

Iroha のネットワーク層は常に有界チャネルを用いてメモリ使用量を予測可能に保ち、バックプレッシャーを明示します。旧来の `p2p_bounded_queues` フラグは互換性のために残っていますが挙動は変わりません。以下を `[network]` で設定します。

- `p2p_queue_cap_high`（usize、既定 8192）  
  - 高優先度ネットワークメッセージキューおよびピア受信ディスパッチバッファの容量
    （コンセンサス／制御メッセージ用）。
- `p2p_queue_cap_low`（usize、既定 32768）  
  - 低優先度ネットワークメッセージキューおよびピア受信ディスパッチバッファの容量
    （ゴシップ／同期メッセージ用）。
- `p2p_post_queue_cap`（usize、既定 2048）  
  - ピアごとの POST チャネル容量（特定ピアへの送信キュー）。
- `p2p_subscriber_queue_cap`（usize、既定 8192）  
  - ノードリレーに供給する各受信サブスクライバキューの容量。

既定値は約 2 万 TPS のブロックチェーンワークロードを想定し、コンセンサス／制御トラフィックの応答性を保ちつつゴシップ／同期に余裕を持たせています。ブロックサイズ、ブロック時間、ネットワーク条件に応じて調整してください。

備考: 高優先度キューはコンセンサス／制御トラフィックを、低優先度キューはゴシップと同期経路を扱います。リレーは高/低の2つのサブスクライバを登録するため、リレー側の合計バッファは `2 * p2p_subscriber_queue_cap` に加えて他の購読分（例: ジェネシスブートストラップ、Torii Connect）が加算されます。

### 低優先度のレート制限（`[network]` 設定）

- `low_priority_rate_per_sec`（任意・msgs/sec）  
  - 低優先度トラフィック（ゴシップ／同期）のピア単位トークンバケットを受信/送信の両方で有効化。未設定なら無効。
- `low_priority_burst`（任意・msgs）  
  - バースト容量。未設定時は `low_priority_rate_per_sec` と同値。

有効化すると、低優先度の受信フレーム（tx ゴシップ、peer/trust ゴシップ、health/time）はリレー投入前に破棄され、送信の POST／ブロードキャストはピア単位でスロットリングされます。ストリーミング制御フレームも制御プレーンの洪水を防ぐため入方向で制限されます。コンセンサス/制御の高優先度トラフィックはそれ以外影響を受けません。

### DNS ホスト名リフレッシュ（`[network]` 設定）

`P2P_PUBLIC_ADDRESS` にホスト名を使用している場合、以下で定期的な再接続を行い IP 変更を取り込めます。

- `dns_refresh_interval_ms`（任意・未設定で無効）  
  - 設定すると、当該ピアは定期的に切断・再接続して OS リゾルバがホスト名を再解決できるようにします。推奨値は 300,000〜600,000（5〜10 分）で、DNS TTL と運用要件に合わせて調整します。

### P2P テレメトリメトリクス

テレメトリが有効な場合、Prometheus で以下のゲージが公開されます。

- `p2p_dropped_posts`: バックプレッシャーにより POST メッセージがドロップされた回数（単調増加）。
- `p2p_dropped_broadcasts`: バックプレッシャーによりブロードキャストがドロップされた回数（単調増加）。
- `p2p_subscriber_queue_full_total`: サブスクライバキュー満杯によりドロップされた受信メッセージ数。
- `p2p_subscriber_queue_full_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`: トピック別のサブスクライバキュー満杯ドロップ件数。
- `p2p_subscriber_unrouted_total`: トピックに一致するサブスクライバがなくドロップされた受信メッセージ数。
- `p2p_subscriber_unrouted_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`: トピック別の未ルーティング受信ドロップ件数。
- `p2p_queue_depth{priority="High|Low"}`: 優先度別のネットワークメッセージキュー深さ。
- `p2p_queue_dropped_total{priority="High|Low",kind="Post|Broadcast"}`: 優先度／種別別のドロップ件数。
- `p2p_handshake_failures`: P2P ハンドシェイク失敗数（タイムアウト、署名検証エラー等）。
- `p2p_low_post_throttled_total`: 低優先度 POST がピア単位トークンバケットで抑制された回数。
- `p2p_low_broadcast_throttled_total`: 低優先度ブロードキャストが抑制された回数。
- `p2p_post_overflow_total`: ピア別 POST チャネルのオーバーフロー件数。
- `p2p_dns_refresh_total`: DNS 間隔リフレッシュ実行回数。
- `p2p_dns_ttl_refresh_total`: DNS TTL ベースのリフレッシュ実行回数。
- `p2p_dns_resolution_fail_total`: ホスト名ピアでの DNS 解決／接続失敗件数。
- `p2p_dns_reconnect_success_total`: リフレッシュ後の再接続成功件数。
- `p2p_backoff_scheduled_total`: アドレス単位でスケジュールされたバックオフ回数。
- `p2p_accept_throttled_total`: IP 単位スロットルにより拒否された受信接続数。
- `p2p_incoming_cap_reject_total`: `max_incoming` により拒否された受信接続数。
- `p2p_total_cap_reject_total`: `max_total_connections` により拒否された接続数。
- `p2p_ws_inbound_total`: 受け入れた WebSocket インバウンド P2P 接続数。
- `p2p_ws_outbound_total`: 成功した WebSocket アウトバウンド P2P 接続数。

これらのメトリクスは飽和状態やネットワーク障害の特定に役立ちます。キューが処理能力を維持している間はドロップカウンタは 0 のままです。

Prometheus の `/metrics` 例:

```
# HELP p2p_dropped_posts Number of p2p post messages dropped due to backpressure
# TYPE p2p_dropped_posts gauge
p2p_dropped_posts 0

# HELP p2p_subscriber_queue_full_total Number of inbound messages dropped because subscriber queues were full
# TYPE p2p_subscriber_queue_full_total gauge
p2p_subscriber_queue_full_total 3

# HELP p2p_subscriber_queue_full_by_topic_total Per-topic inbound drops caused by full subscriber queues
# TYPE p2p_subscriber_queue_full_by_topic_total gauge
p2p_subscriber_queue_full_by_topic_total{topic="Consensus"} 2

# HELP p2p_subscriber_unrouted_total Number of inbound messages dropped because no subscriber matches the topic
# TYPE p2p_subscriber_unrouted_total gauge
p2p_subscriber_unrouted_total 7

# HELP p2p_subscriber_unrouted_by_topic_total Per-topic inbound drops caused by no matching subscriber
# TYPE p2p_subscriber_unrouted_by_topic_total gauge
p2p_subscriber_unrouted_by_topic_total{topic="Consensus"} 1
...
```

### トピック別スケジューリング

- アウトバウンドトラフィックは論理トピックに分離され、ヘッドオブラインブロッキングを避けつつコンセンサス／制御メッセージを優先します。
  - 高優先度: `Consensus`, `Control`（バイアス付き優先）
  - 低優先度: `BlockSync`, `TxGossip`, `PeerGossip`, `Health`, `Other`（公平スケジューリング）
- メッセージペイロードは `iroha_p2p::network::message::ClassifyTopic` を実装してトピックを提供し、`iroha_core::NetworkMessage` がコアメッセージの割り当てを定義します。
- 小さなフェアネス予算により、高優先度 traffic が継続している場合でも低優先度トピックが前進できるようにします。

### プロキシサポート（HTTP CONNECT / SOCKS5）

- 設定（`[network]`）:
  - `p2p_proxy`（文字列; 任意）: アウトバウンドプロキシ URL（例: `http://user:pass@proxy.example.com:8080`、`https://proxy.example.com:8443`、`socks5://user:pass@proxy.example.com:1080`）。
  - `p2p_no_proxy`（文字列配列）: プロキシをバイパスするホストサフィックス（例: `.example.com`, `localhost`）。
- `p2p_proxy` が設定され、対象ホストが除外されていない場合、以下でトンネルします:
  - `http://...` / `https://...`: HTTP `CONNECT host:port`
    - `http://...` はプロキシまで平文 TCP で接続します。
    - `https://...` は `CONNECT` の前にプロキシへの接続自体を TLS で保護します（`iroha_p2p/p2p_tls` を有効にしたビルドが必要）。
  - `socks5://...` / `socks5h://...`: SOCKS5 `CONNECT`
- 注意:
  - ベーシック認証はプロキシ URL の `user:pass@...` として指定できます。
  - 除外は単純なホストサフィックスで照合します。
  - プロキシは TCP ベースのダイヤル（TCP/TLS/WS）にのみ適用されます。QUIC（UDP）はプロキシをバイパスするため、必ずプロキシ経由にしたい場合は `quic_enabled=false` にしてください。
  - プロキシ未設定時は直接接続します。

### リレーモード（Hub/Spoke/Assist）

Iroha は、NAT/ファイアウォール配下や検閲下のネットワークにいる一部のピアの到達性を改善するため、任意でリレーハブを利用できます。これはアプリケーションレベルのリレー（暗号化フレームの転送）であり、インターネット上の特別な経路制御ではありません。

- 設定（`[network]`）:
  - `relay_mode`（文字列; `disabled` | `hub` | `spoke` | `assist`）
    - `disabled`: 既定。ピア同士が直接接続するメッシュ。
    - `hub`: スポークを受け入れ、ピア間のトラフィックを転送します。
    - `spoke`: ハブにのみダイヤルし、転送に依存します（受信接続が難しい環境向け）。
    - `assist`: 可能な限り直接接続しつつハブ接続も維持し、対象ピアに直接接続していない場合はハブ経由でルーティングします。
  - `relay_hub_addresses`（ソケットアドレス配列; `spoke` と `assist` で必須）
    - ダイヤルする hub のアドレス一覧（`relay_mode=\"hub\"` で動作しているピア）。複数指定した場合は順に試行し、到達不能時は後続の hub にフォールバックします。
  - `relay_ttl`（u8; 既定 8）
    - 転送フレームのホップ制限（リレーループ防止）。

推奨デプロイ:
- 安定した公開アドレス上で少なくとも 1 台の hub を運用（例: データセンターノード）。
- 制約のあるノードは `spoke` として hub へのアウトバウンド接続のみを必要にする。
- バリデータ／接続性の高いノードは `assist` にして、全員をリレー必須にせず spokes に到達できるようにする。

`config.toml` 例:

```toml
[network]
relay_mode = "hub"
```

```toml
[network]
relay_mode = "spoke"
relay_hub_addresses = ["hub.example.com:1337"]
```

```toml
[network]
relay_mode = "assist"
relay_hub_addresses = ["hub.example.com:1337"]
```

複数ハブ例（検閲/フェイルオーバー）:

```toml
[network]
relay_mode = "assist"
relay_hub_addresses = ["hub1.example.com:1337", "hub2.example.com:1337"]
```

### ダイヤル戦略（Happy Eyeballs）

- Iroha は複数アドレス（ホスト名、IPv6、IPv4）を短いステップで並列ダイヤルし、到達可能なパスが迅速に勝つようにします。
- 既定の優先順位: ホスト名、IPv6、IPv4 の順。
- ステップ間隔は `[network]` の `happy_eyeballs_stagger_ms`（既定 100ms）で調整可能。大規模ピアリストで突発的なダイヤルを抑えたい場合は増やし、遅いネットワークで高速フェイルオーバーしたい場合は減らします。
- アドレス単位のバックオフは指数ジッタ（最大 5 秒）で独立管理し、集中ダイヤルを回避します。

### `[network]` TOML と機能フラグ例

最小構成例（値は約 2 万 TPS 向け既定値）:

```toml
[network]
# 内部バインドアドレスと広告アドレス
P2P_ADDRESS = "0.0.0.0:1337"
P2P_PUBLIC_ADDRESS = "peer1.example.com:1337"

# 有界キュー容量
p2p_queue_cap_high = 8192     # consensus/control
p2p_queue_cap_low  = 32768    # gossip/sync
p2p_post_queue_cap = 2048     # per-peer post channel
p2p_subscriber_queue_cap = 8192  # inbound relay subscriber queue

# 参考: その他のネットワークパラメータ
block_gossip_size = 4        # ブロック同期/可用性投票のファンアウト上限（ピアサンプル + ブロック同期更新）
block_gossip_period_ms = 10000
block_gossip_max_period_ms = 30000
peer_gossip_period_ms = 1000
peer_gossip_max_period_ms = 30000
transaction_gossip_size = 500
transaction_gossip_resend_ticks = 3
```

- Gossip/idle の間隔は 100ms 未満なら 100ms にクランプされます。
- ピアアドレス gossip は変更検知で送信され、アイドル時は `peer_gossip_max_period_ms`
  まで指数バックオフします。ブロック同期のサンプリングも同様に
  `block_gossip_max_period_ms` までバックオフします。
- トランザクション gossip は relay バックプレッシャ（subscriber queue の drop）発生時に一時停止し、
  `transaction_gossip_period_ms * transaction_gossip_resend_ticks` 経過後に再開します。
