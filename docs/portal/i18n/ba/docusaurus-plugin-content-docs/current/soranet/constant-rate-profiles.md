---
id: constant-rate-profiles
lang: ba
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet constant-rate profiles
sidebar_label: Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production relays plus the SNNet-17A2 null dogfood profile, with tick->bandwidth math, CLI helpers, and MTU guardrails.
---

:::note Canonical Source
:::

SNNet-17B introduces fixed-rate transport lanes so relays move traffic in 1,024 B cells regardless
of payload size. Operators pick from three presets:

- **core** - data-centre or professionally hosted relays that can dedicate >=30 Mbps to cover
  traffic.
- **home** - residential or low-uplink operators that still need anonymous fetches for
  privacy-critical circuits.
- **null** - the SNNet-17A2 dogfood preset. It retains the same TLVs/envelope but stretches the
  tick and ceiling for low-bandwidth staging.

## Preset summary

| Profile | Tick (ms) | Cell (B) | Lane cap | Dummy floor | Per-lane payload (Mb/s) | Ceiling payload (Mb/s) | Ceiling % of uplink | Recommended uplink (Mb/s) | Neighbor cap | Auto-disable trigger (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **Lane cap** - maximum concurrent constant-rate neighbors. The relay rejects extra circuits once
  the cap is hit and increments `soranet_handshake_capacity_reject_total`.
- **Dummy floor** - minimum number of lanes that stay alive with dummy traffic even when actual
  demand is lower.
- **Ceiling payload** - uplink budget dedicated to constant-rate lanes after applying the ceiling
  fraction. Operators should never exceed this budget even if extra bandwidth is available.
- **Auto-disable trigger** - sustained saturation percentage (averaged per preset) that causes the
  runtime to drop to the dummy floor. Capacity is restored after the recovery threshold
  (75% for `core`, 60% for `home`, 45% for `null`).

**Important:** the `null` preset is for staging and capability dogfooding only; it does not meet the
privacy guarantees required for production circuits.

## Tick -> bandwidth table

Each payload cell carries 1,024 B, so the KiB/sec column equals the number of cells emitted per
second. Use the helper to extend the table with custom ticks.

| Tick (ms) | Cells/sec | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

Formula:

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

`--format markdown` emits GitHub-style tables for both the preset summary and optional tick cheat
sheet so you can paste deterministic output into the portal. Pair it with `--json-out` to archive
the rendered data for governance evidence.

## Configuration & overrides

`tools/soranet-relay` exposes the presets in both config files and runtime overrides:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

The config key accepts `core`, `home`, or `null` (default `core`). CLI overrides are useful for
staging drills or SOC requests that temporarily reduce the duty cycle without rewriting configs.

## MTU guardrails

- Payload cells use 1,024 B plus ~96 B of Norito+Noise framing and the minimal QUIC/UDP headers,
  keeping each datagram below the IPv6 1,280 B minimum MTU.
- When tunnels (WireGuard/IPsec) add extra encapsulation you **must** reduce `padding.cell_size`
  so `cell_size + framing <= 1,280 B`. The relay validator enforces
  `padding.cell_size <= 1,136 B` (1,280 B - 48 B UDP/IPv6 overhead - 96 B framing).
- `core` profiles should pin >=4 neighbors even when idle so dummy lanes always cover a subset of
  PQ guards. `home` profiles may limit constant-rate circuits to wallets/aggregators but must apply
  back-pressure when saturation exceeds 70% for three telemetry windows.

## Telemetry & alerts

Relays export the following metrics per preset:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

Alert when:

1. Dummy ratio stays below the preset floor (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) for more than
   two windows.
2. `soranet_constant_rate_ceiling_hits_total` grows faster than one hit per five minutes.
3. `soranet_constant_rate_degraded` flips to `1` outside a planned drill.

Record the preset label and neighbor list in incident reports so auditors can prove constant-rate
policies matched the roadmap requirements.
