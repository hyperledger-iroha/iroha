---
id: constant-rate-profiles
lang: ja
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Canonical Source
このページは `docs/source/soranet/constant_rate_profiles.md` を反映する。従来の docs が廃止されるまで両方を同期しておくこと。
:::

SNNet-17B は固定レートの transport lane を導入し、relays が payload サイズに関係なく 1,024 B のセルでトラフィックを流すようにする。オペレーターは 3 つの preset から選択する:

- **core** - データセンターまたはプロホスティングの relays。トラフィックを賄うために >=30 Mbps を割り当てられる。
- **home** - 低 uplink の住宅系オペレーター。プライバシー重視の circuits に匿名 fetch が必要。
- **null** - SNNet-17A2 の dogfood preset。同じ TLVs/envelope を維持しつつ、低帯域 staging 向けに tick と ceiling を伸ばす。

## Preset 概要

| プロファイル | Tick (ms) | セル (B) | Lane cap | Dummy floor | Lane あたり payload (Mb/s) | Ceiling payload (Mb/s) | Uplink ceiling % | 推奨 uplink (Mb/s) | Neighbor cap | Auto-disable トリガー (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Lane cap** - 同時に維持できる定率 neighbor の最大数。cap 到達時は extra circuits を拒否し、`soranet_handshake_capacity_reject_total` を増やす。
- **Dummy floor** - 実需要が低くても dummy トラフィックで維持する lane の最小数。
- **Payload ceiling** - ceiling の比率を適用した後に定率 lane に割り当てる uplink 予算。追加帯域があってもこの予算は超えないこと。
- **Auto-disable トリガー** - preset ごとの平均で算出される持続的な飽和率。runtime が dummy floor まで落ちる。容量は recovery しきい値 ( `core` 75%、`home` 60%、`null` 45% ) で復帰する。

**重要:** `null` preset は staging と dogfooding 専用であり、本番 circuits に必要なプライバシー保証は満たさない。

## Tick -> bandwidth 表

各 payload セルは 1,024 B を運ぶため、KiB/sec 列は 1 秒あたりのセル数に等しい。カスタム tick を追加する場合は helper を使う。

| Tick (ms) | Cells/sec | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

式:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI helper:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` は preset 概要と任意の tick 表の両方を GitHub 形式で出力し、ポータルに決定論的な出力を貼り付けられるようにする。`--json-out` と組み合わせて、レンダリングされたデータをガバナンス証跡として保存する。

## 設定と overrides

`tools/soranet-relay` は presets を設定ファイルと runtime overrides の両方で提供する:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

設定キーは `core`、`home`、`null` を受け付ける (default `core`)。CLI overrides は staging drills や SOC 要請で一時的に duty cycle を下げたい時に有用で、config を書き換えずに済む。

## MTU guardrails

- Payload セルは 1,024 B と ~96 B の Norito+Noise フレーミング、最小の QUIC/UDP headers を使い、各 datagram を IPv6 最小 MTU 1,280 B 未満に保つ。
- トンネル (WireGuard/IPsec) が追加のカプセル化を行う場合、`cell_size + framing <= 1,280 B` となるよう **必ず** `padding.cell_size` を下げること。relay バリデータは `padding.cell_size <= 1,136 B` (1,280 B - 48 B UDP/IPv6 overhead - 96 B framing) を強制する。
- `core` プロファイルは idle 時でも >=4 neighbors を固定し、dummy lanes が常に PQ guards の一部をカバーするようにする。`home` プロファイルは wallets/aggregators 向けに定率 circuits を制限してよいが、saturation が 70% を 3 つの telemetry ウィンドウで超える場合は back-pressure を適用すること。

## Telemetry と alerts

Relays は preset ごとに次のメトリクスを出力する:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Alert 条件:

1. Dummy ratio が preset floor (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) を 2 ウィンドウ以上下回る。
2. `soranet_constant_rate_ceiling_hits_total` が 5 分あたり 1 hit より速く増加する。
3. `soranet_constant_rate_degraded` が計画外の drill で `1` に切り替わる。

インシデントレポートには preset ラベルと neighbor リストを記録し、監査で定率ポリシーがロードマップ要件に合致していたことを示す。
