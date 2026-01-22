---
lang: ja
direction: ltr
source: docs/source/soranet_vpn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b7e738790d81688d947760a65575f7f20e57539a93d77b37c9e84b0e41b224a
source_last_modified: "2026-01-04T17:06:57.110055+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet_vpn.md -->

# SoraNet ネイティブ VPN ブリッジ

ネイティブ VPN ブリッジは IP トラフィックを固定 1,024 バイトの SoraNet セルに
ラップし、PacketTunnel クライアント、出口ゲートウェイ、ガバナンス/課金面が
同じ決定論的フレーミングを共有できるようにする。

- **セルフォーマット:** `crates/iroha_data_model/src/soranet/vpn.rs` がヘッダ
  （version, class, flags, circuit id, flow label, seq/ack, padding budget,
  payload length）を固定し、padding（`VpnCellV1::into_padded_frame`）や
  control-plane/課金 payload の helpers を提供する。payload 容量は
  `1024 - 42 = 982` バイトで、ヘッダは padding budget をミリ秒で持つ。
- **Control plane:** `crates/iroha_config/src/parameters/{defaults,user,actual}.rs`
  が `network.soranet_vpn.*` のノブ（セルサイズ、flow label 幅、cover 比率、
  burst、heartbeat、jitter、padding budget、guard refresh、lease、DNS push 間隔、
  exit class、meter family）を追加。クライアント API サマリは同じ項目を SDK に
  公開する。
- **Cover scheduling:** `xtask/src/soranet_vpn.rs` は設定から決定論的な cover/data
  プランを構築し、全 32 バイト seed を用いた BLAKE3 XOF で生成する。burst を
  クランプし、設定された padding budget で payload をフレーム化し、exit class に
  紐付いた課金レシートを生成する。
- **Cover ratio + seeding:** `cover_to_data_per_mille` は 0-1000 を受け付ける。
  `0` を明示すると `vpn.cover.enabled=true` でも cover を無効化し、burst 上限は
  data スロットを挿入しつつ cover 連続をリセットする。`VpnBridge` は circuit id
  と flow label から per-circuit のデフォルト cover seed を導出し、`set_cover_seed`
  で上書きできる。【crates/iroha_config/src/parameters/user.rs:6380】【tools/soranet-relay/src/config.rs:740】【crates/iroha_data_model/src/soranet/vpn.rs:509】【tools/soranet-relay/src/vpn_adapter.rs:224】
- **Flow-label enforcement:** `flow_label_bits` は設定/クライアント入力で
  1–24 ビットにクランプ（デフォルト 24）。フレームビルダーは幅を検証し、
  パーサは許容幅を超える flow label のフレームを拒否する。
- **Exit/lease validation:** exit class ラベルは `standard`/`low-latency`/
  `high-security` の allowlist に限定（ハイフン/アンダースコアは許容）され、
  ワイヤに載る前に正規化される。未知ラベルは client/config 解析や xtask
  helper でエラーになる。control-plane の lease は `u32` 秒に収まり、
  config 解析、client サマリ、control-plane builder で早期拒否される。
  【crates/iroha_data_model/src/soranet/vpn.rs:548】【crates/iroha_config/src/parameters/user.rs:5529】【crates/iroha_config/src/client_api.rs:1549】【xtask/src/soranet_vpn.rs:42】
- **クライアント面:** `IrohaSwift/Sources/IrohaSwift/SoranetVpnTunnel.swift` は
  PacketTunnel 向けフレーマーを提供し、1,024 バイトにパディングし、ヘッダ
  レイアウトを強制し、DNS/route push 用の小さな
  `NEPacketTunnelNetworkSettings` helper を提供する。ユニットテスト
  （`IrohaSwift/Tests/IrohaSwiftTests/`）は Rust レイアウトを反映する。
- **レシート/課金:** 出口ゲートウェイは `VpnSessionReceiptV1`（ingress/egress/cover
  bytes、uptime、exit class、meter hash）を生成する。`xtask/src/soranet_vpn.rs`
  の helper を用いて Norito payload を既定設定とガバナンス meter に揃える。
  relay runtime は circuit teardown 時に receipts を発行し、Prometheus
  カウンタ（`soranet_vpn_session_receipts_total`, `soranet_vpn_receipt_*_bytes_total`）
  を公開する。runtime カウンタは data/cover の frames/bytes を分離
  （`soranet_vpn_{data,cover}_{frames,bytes}_total`）し、byte カウンタは payload
  bytes を追跡する（on-wire bytes は `frames * 1024`）。control/keepalive
  セルは `soranet_vpn_control_{frames,bytes}_total` で別計測され、payload
  メトリクスとレシートから除外される。【tools/soranet-relay/src/runtime.rs:1984】【tools/soranet-relay/src/metrics.rs:744】【tools/soranet-relay/tests/vpn_adapter.rs:1】
- **エンドツーエンド・メトリクスハーネス:** adapter suite には、bridge→adapter
  往復のペースド・テストが追加され、duplex リンク上で data/cover セルを流して
  両端の ingress/egress カウンタを検証する。出口側への payload 配達も検証し、
  SNNet-18f7 で約束された cover/data 会計を強化する。
  【tools/soranet-relay/tests/vpn_adapter.rs:1】
- **Frame I/O + padding enforcement:** relay ビルダーは設定から padding budget
  を再書き込みし、固定 1,024 バイトサイズと flag allowlist を強制する。
  async read/write helper は切り詰めフレームを破棄しつつ ingress/egress bytes
  をカウントする。overlay/adapter テストはゼロ padding、payload 長制限、
  切断ストリーム拒否を保護する。【tools/soranet-relay/src/vpn.rs:1】【tools/soranet-relay/tests/vpn_overlay.rs:1】【tools/soranet-relay/tests/vpn_adapter.rs:1】【xtask/src/soranet_vpn.rs:1】
- **Pacing + cover injection:** `schedule_frames` は `pacing_millis` を適用して
  BLAKE3 シードのプラン（burst/jitter 制限）から cover/data フレームを
  インターリーブし、`send_scheduled_frames` は計算された cadence で送信する。
  回帰テストは送信間隔を検証する。【tools/soranet-relay/src/vpn.rs:303】【tools/soranet-relay/tests/vpn_runtime.rs:1】
- **Runtime guard & telemetry:** frame I/O、pacing、receipt 発行は relay runtime
  で動作し、exit-bridge/control-plane wiring と並行する。Prometheus gauge
  `soranet_vpn_runtime_status{state="disabled|active|stubbed"}`（`vpn_session_meter`/
  `vpn_byte_meter` ラベル付き）と receipt カウンタが、VPN ハンドリングが
  active vs stubbed/disabled であることを示す。
  【tools/soranet-relay/src/runtime.rs:1】【tools/soranet-relay/src/config.rs:1】【tools/soranet-relay/src/metrics.rs:1】

デプロイ時の heartbeat/cover 予算は `network.soranet_vpn` で調整し、
`xtask/src/soranet_vpn.rs` を使って再現可能なスケジュールとレシートを
生成し、受け入れ証跡を作成する。
