---
lang: mn
direction: ltr
source: docs/source/soranet/constant_rate_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9bc4e013d93fcafe7cd7c527527ac49f33297fdf9b985a144cb5f603c359b7ea
source_last_modified: "2025-12-29T18:16:36.184660+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production nodes plus the SNNet-17A2 null dogfood profile, alongside tick→bandwidth tables and operational guardrails.
---

# SoraNet Constant-Rate Profiles (SNNet-17B1)

SNNet-17B introduces constant-rate transport lanes so every SoraFS fetch hops across fixed-size
cells independent of payload size. The preset catalogue now ships with:

- **core** – datacentre or professionally hosted relays that can dedicate ≥30 Mbps of uplink to
  constant-rate cover traffic.
- **home** – residential or lower-bandwidth operators that still need anonymous fetches for the
  most sensitive circuits without exhausting the access link.
- **null** – a SNNet-17A2 dogfood preset that keeps the exact same envelope/TLVs but stretches the
  tick and ceiling so operators can test capability negotiation without paying the full bandwidth
  cost. This preset should remain on staging or tightly scoped pilots.

Both presets share a 1,024 B payload cell, 1024 B dummy fill, and the hybrid Noise+QUIC envelope
defined in SNNet-17A. This document records the normative parameters mandated by SNNet-17B1, the
tick→bandwidth conversion table used by SDKs, and the CLI interface that operators can call when
generating configs or status reports.

## Preset summary

| Profile | Tick (ms) | Cell (B) | Lanes | Dummy floor | Per-lane payload (Mb/s) | Ceiling payload (Mb/s) | Ceiling % of uplink | Recommended uplink (Mb/s) | Neighbor cap | Auto-disable trigger (%) |
|---------|-----------|----------|-------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12    | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4     | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2     | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

**Definitions**

- **Lanes** – concurrent constant-rate channels the relay may run. Operators MUST cap the number
  of constant-rate neighbors to the listed value to prevent residential uplinks from starving
  their priority peers.
- **Dummy floor** – minimum number of lanes that always transmit dummy traffic to maintain cover.
  When measured SoraNet demand is lower than this floor, relays still send dummy data at the
  advertised tick so circuit guards cannot infer usage.
- **Ceiling payload (Mb/s)** – uplink budget dedicated to constant-rate cells after applying the
  uplink ceiling percentage. Operators should never schedule constant-rate payloads above this
  budget even if spare bandwidth exists.
- **Auto-disable trigger** – dequeue-based saturation percentage (averaged over a 60 s window for
  `core`, 180 s for `home`). When telemetry observes a sustained value at or above the trigger,
  the relay automatically drops the neighbor cap to the preset dummy floor. Capacity is restored only
  once saturation falls below the profile’s recovery threshold (75 % for `core`, 60 % for `home`,
  45 % for `null`), guaranteeing that residential operators cannot oversubscribe their access links
  indefinitely.

**Null preset usage:** operators should use `null` when validating SNNet-17A2 capability negotiation,
mixed hops, and downgrade policies without consuming the bandwidth that the production profiles
require. The preset keeps the same TLVs/envelope so clients exercise the entire constant-rate stack,
but its low lane cap and ceiling make it unsuitable for production privacy guarantees. Limit the
preset to staging clusters or constrained pilots and switch back to `home`/`core` once telemetry
confirms the rollout.

Relays now enforce the `neighbor_cap` directly during the handshake: once the number of
constant-rate circuits reaches the preset limit, additional `snnet.constant_rate` sessions are
rejected with a `constant-rate capacity exceeded` close reason and the rejection is tracked in
`soranet_handshake_capacity_reject_total`. Operators can monitor the live count via the
`soranet_constant_rate_active_neighbors` Prometheus gauge, which carries the constant-rate profile
labels so audits can prove that the cap is being applied correctly.

## Tick → bandwidth cheat sheet

The transport uses fixed 1,024 B cells. Table values follow:

| Tick (ms) | Cells/sec | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

**Formula:** `payload_mbps = (cell_bytes × 8 / 1_000_000) × (1000 / tick_ms)` with `cell_bytes = 1024`.
Because the payload cell equals 1 KiB, the KiB/sec column matches `cells/sec`.

Operators can extend this table with the CLI helper and emit Markdown directly for documentation:

```bash
cargo xtask soranet-constant-rate-profile \
  --tick-table \
  --tick-values 5,7.5,12,18 \
  --format markdown
```

## CLI and automation support

The new helper command surfaces the presets in both human-readable and JSON form so automation
can stay in sync with the roadmap parameters. Use `--json-out <path|->` to persist the rendered
report even when you request table output, making it easy to attach the preset catalogue to
change-control tickets:

```bash
# Markdown table output for all presets plus the default tick table + JSON artefact on disk
cargo xtask soranet-constant-rate-profile \
  --tick-table \
  --format markdown \
  --json-out artifacts/soranet/constant_rate/report.json

# JSON summary for the core profile only (stdout)
cargo xtask soranet-constant-rate-profile --profile core --format json

# Quick report for the null dogfood preset
cargo xtask soranet-constant-rate-profile --profile null
```

When `--format markdown` is supplied the command emits GitHub-flavoured Markdown tables for both
the preset summary and the optional tick cheat sheet, making it easy to paste deterministic tables
into this document or the portal mirror.

Passing `--json-out -` writes the prettified JSON to stdout so scripts can capture the same
structure without re-running the command. The JSON payload mirrors the preset table fields and
includes the optional `tick_bandwidth` section when `--tick-table` (or `--tick-values`) is supplied,
allowing SDKs or ops tooling to load the canonical parameters without scraping documentation.

The relay daemon exposes the same presets through configuration and runtime overrides:

```bash
# Persisted in the relay JSON config
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

The `constant_rate_profile` key accepts `core`, `home`, or `null`. The default remains `core` so
data-centre deployments stay on the higher-duty-cycle plan unless explicitly reconfigured. `null`
is a staging/dogfood preset; only enable it when exercising the SNNet‑17A capability rollout plan.

## MTU and padding guidance

- A constant-rate cell carries 1,024 B of payload plus ~96 B of Norito+Noise framing, QUIC crypto,
  and telemetry tags. When transported over UDP/IPv6 the total datagram stays below 1,260 B,
  comfortably under the 1,280 B IPv6 minimum MTU and the 1,350 B QUIC handshake recommendation.
- Operators MUST keep `cell_size + framing <= 1,280 B` when tunnels encapsulate the relay traffic
  (WireGuard, IPsec). If overhead pushes the envelope above the MTU the relay must lower its
  cellular padding or enforce per-neighbor DF-bit fragment rejection.
- `core` profiles SHOULD pin at least four neighbors to constant-rate circuits even when the dummy
  floor covers idle traffic. `home` profiles MAY limit constant-rate neighbors to regulated or
  privacy-critical circuits (wallets, aggregator nodes), and MUST assert back-pressure on new
  neighbors when uplink saturation sits above 70 % for three consecutive telemetry windows.
- The relay validator now enforces `padding.cell_size <= 1,136 B`
  (1,280 B IPv6 MTU − 48 B UDP/IPv6 headers − 96 B Norito+Noise framing). Any larger value
  triggers a `ConfigError::Padding` during startup so operators cannot accidentally fragment
  constant-rate cells when editing configs by hand.

## Handshake capability TLV

- Relays advertise constant-rate support through the optional
  `constant_rate_capability` stanza:
  ```json
  "constant_rate_capability": {
    "enabled": true,
    "strict": true
  }
  ```
  When enabled, the handshake includes the `snnet.constant_rate` TLV (type `0x0203`)
  with the following payload layout:

  | Offset | Field                          | Notes |
  |--------|--------------------------------|-------|
  | 0      | `version:u8`                   | Currently `1`. |
  | 1      | `flags:u8`                     | `0x01` = strict, `0x00` = best-effort. |
  | 2–3    | `cell_bytes:u16 (little-endian)` | Always `1024` for SNNet-17A profiles. |
  | 4..    | `cell_envelope[cell_bytes]`    | Deterministic sample of the scheduler’s dummy cell (type byte, length field, and zeroed payload). |

  The sample envelope makes the layout self-describing: clients MUST parse it (see
  `Cell::from_bytes()` in the relay scheduler) and reject handshakes whose sample length,
  header, or padding diverge from the profile they support.
- Clients that set the strict flag require every hop to expose the same TLV; the relay now rejects
  handshakes when a strict request is received but the server advertises best-effort or no
  constant-rate support. Best-effort requests gracefully downgrade when a hop lacks the capability.
- Defaults keep the capability disabled so brownfield deployments can stage the rollout; enabling
  strict mode is recommended once every hop supports SNNet‑17A pacing.

## Telemetry-driven lane management

Relays and orchestrators should wire the `soranet_constant_rate_queue_depth`,
`soranet_constant_rate_saturation_percent`, `soranet_constant_rate_active_neighbors`, and
`soranet.guard_selection` gauges into their alerting pipelines so the auto-disable triggers are
auditable. Recommended actions:

1. Relay metrics now include `constant_rate_profile` and `constant_rate_neighbors` labels on
   every counter/gauge, and the CLI status/export paths mirror the same metadata so audits can
   prove which preset was active during an incident.
2. When the rolling utilisation exceeds the trigger, reduce the configured lane cap in steps of
   one lane until saturation drops below 60 % (home), 75 % (core), or 45 % (null).
3. Log every automatic change with the measured utilisation and neighbor list so audits can prove
   adherence to the SNNet-17B policy.
4. Track the new cover-traffic gauges and alerts:
   - `soranet_constant_rate_queue_depth_class{class}` exposes per-class queue pressure feeding the
     constant-rate sender; `soranet_constant_rate_queue_depth` remains the aggregate view.
   - `soranet_constant_rate_low_dummy_events_total` increments whenever the live dummy ratio falls
     below 20 % in the scheduler loop (real traffic should never fully crowd out cover).
   - `soranet_constant_rate_dummy_ratio` reflects the observed ratio across emitted cells (real +
     dummy); combine with `soranet_constant_rate_degraded` to alert on cover shrinkage.
   - The relay now runs a dedicated constant-rate datagram loop per circuit that emits the
     scheduler’s 1,024 B envelopes on the profile tick even when idle, so dashboards can chart live
     slot-rate health instead of inferring it from admission counters alone.
5. Observability assets: Grafana board `dashboards/grafana/soranet_constant_rate.json` charts
   queue depth per class, dummy ratio, live neighbor count, and degraded-state markers; the
   companion alert bundle `dashboards/alerts/soranet_constant_rate_rules.yml` fires when dummy
   share bottoms out (<20% for 5 m) or backlog exceeds 32 cells. Run these in staging first to
   validate thresholds before enabling in production.

These guardrails ensure residential operators do not destabilise their access links while still
keeping SoraNet the default transport surface for SoraFS traffic.
