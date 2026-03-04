---
lang: ja
direction: ltr
source: docs/source/soranet/lane_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c54af03ac031f706287033ae7158c6d58c76ef275cf88d4127ed0a3f296ec5a
source_last_modified: "2026-01-03T18:08:02.001573+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/soranet/lane_profiles.md -->

# SoraNet レーンプロファイル

`network.lane_profile` は、P2P レイヤーに適用されるデフォルトの接続上限と低優先アップリンク予算を制御する。
**core** プロファイルは従来の挙動 (明示的に設定されない限り上限を設けない) を反映し、データセンター/バリデーター配備向け。
**home** プロファイルは制約のあるリンク向けに保守的な制限を適用し、ピアごとの予算を自動的に導出する。

| プロファイル | Tick (ms) | MTU ヒント (bytes) | 目標 uplink (Mbps) | 定率隣接 | 最大受信 | 最大合計 | ピアごとの低優先予算 | ピアごとの低優先レート |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| core | 5 | 1500 | 設定を継承 | 設定を継承 | 設定を継承 | 設定を継承 | 設定を継承 | 設定を継承 |
| home | 10 | 1400 | 120 | 12 | 12 | 32 | 1,250,000 B/s (~10 Mbps) | 947 msgs/s (1,320 B payload を想定) |

### 挙動

- 選択されたプロファイルは設定面 (`network.lane_profile`) に記録され、CLI/SDK 呼び出し元がどの shaping プリセットが有効かを報告できる。
- home は `max_incoming`, `max_total_connections`, `low_priority_bytes_per_sec`,
  `low_priority_rate_per_sec` が未設定の場合に自動的に埋める。明示値があればそれが優先される。
- 設定更新 API でプロファイルを切り替えると、派生した上限が再適用され、制約リンクが自動的に余分なレーンを無効化できる。
- MTU ヒントは参考値。トピックごとのフレーム上限を上書きする場合は、home/コンシューマーリンクで断片化を避けるため、記載されたペイロード範囲内に保つこと。

### 例

```toml
[network]
lane_profile = "home"               # apply home shaping defaults
# Optional overrides if you need stricter or looser caps:
# max_total_connections = 40
# low_priority_bytes_per_sec = 900000
```
