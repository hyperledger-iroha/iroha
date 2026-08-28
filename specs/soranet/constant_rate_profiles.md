---
title: SoraNet Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production nodes plus the SNNet-17A2 null dogfood profile, alongside tick→bandwidth tables and operational guardrails.
---

# SoraNet Constant-Rate Profiles (SNNet-17B1)

SNNet-17B defines constant-rate transport lanes in which every SoraFS fetch hops across fixed-size
cells independent of payload size. The preset catalogue ships with:

- **core** – datacentre or professionally hosted relays that can dedicate ≥30 Mbps of uplink to
  constant-rate cover traffic.
- **home** – residential or lower-bandwidth operators that still need anonymous fetches for the
  most sensitive circuits without exhausting the access link.
- **null** – a SNNet-17A2 dogfood preset that keeps the exact same envelope/TLVs but stretches the
  tick and ceiling so operators can test capability negotiation without paying the full bandwidth
  cost. This preset should remain on staging or tightly scoped pilots.

All presets share an exact 1,024 B wire cell containing an authenticated mux record plus canonical
zero fill, and the hybrid Noise+QUIC envelope defined in SNNet-17A. This document records the
normative parameters mandated by SNNet-17B1, the
tick→bandwidth conversion table used by SDKs, and the CLI interface that operators can call when
generating configs or status reports.

> **Current relay status:** all relay QUIC endpoint creation is deliberately unreachable in production
> until quinn-proto 0.11.17 or later replaces vulnerable 0.11.15. Strict mode is implemented but remains
> independently gated. The locked receive queue charges payload bytes but no fixed cost per
> DATAGRAM entry, so an unauthenticated remote peer could enqueue unbounded zero-length entries without
> consuming the configured byte budget. Configuration and live handshake preflight independently
> reject strict mode until per-entry accounting is available and the complete end-to-end path is
> requalified. The dormant mux carries application/exit, measurement, and VPN bytes with cover in
> exact 1,024-byte cells and fails closed on transport or scheduling errors; its presence is not an
> activation claim. Best-effort cover and the authenticated VPN stream remain dormant with the
> broader QUIC runtime.

## Preset summary

| Profile | Tick (ms) | Cell (B) | Lanes | Dummy floor | Per-lane wire rate (Mb/s) | Ceiling wire rate (Mb/s) | Ceiling % of uplink | Recommended uplink (Mb/s) | Neighbor cap | Auto-disable trigger (%) |
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
- **Ceiling wire rate (Mb/s)** – uplink budget dedicated to constant-rate cells after applying the
  uplink ceiling percentage. Operators should never schedule constant-rate payloads above this
  budget even if spare bandwidth exists.
- **Auto-disable trigger** – dequeue-based saturation percentage (averaged over a 60 s window for
  `core`, 180 s for `home`). When telemetry observes a sustained value at or above the trigger,
  the relay automatically drops the neighbor cap to the preset dummy floor. Capacity is restored only
  once saturation falls below the profile’s recovery threshold (75 % for `core`, 60 % for `home`,
  45 % for `null`), guaranteeing that residential operators cannot oversubscribe their access links
  indefinitely.

**Null preset usage:** after the Quinn upgrade, operators may use `null` when validating SNNet-17A2 capability negotiation,
mixed hops, downgrade policies, and the best-effort cover loop without consuming the bandwidth that
the production profiles require. It does not exercise scheduler-bound application payload, and its
low lane cap and ceiling make it unsuitable for production privacy guarantees. Limit the preset to
staging clusters or constrained pilots. While the dependency gate is active, the preset remains
configuration and requalification material only; relay startup fails before binding a QUIC endpoint.

Relays now enforce the `neighbor_cap` directly during the handshake: once the number of
constant-rate circuits reaches the preset limit, additional `snnet.constant_rate` sessions are
rejected with a `constant-rate capacity exceeded` close reason and the rejection is tracked in
`soranet_handshake_capacity_reject_total`. Operators can monitor the live count via the
`soranet_constant_rate_active_neighbors` Prometheus gauge, which carries the constant-rate profile
labels so audits can prove that the cap is being applied correctly.

## Tick → bandwidth cheat sheet

The transport uses fixed 1,024 B cells. Table values follow:

| Tick (ms) | Cells/sec | Cell KiB/sec | Wire Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

**Formula:** `wire_mbps = (cell_bytes × 8 / 1_000_000) × (1000 / tick_ms)` with `cell_bytes = 1024`.
Because the wire cell equals 1 KiB, the KiB/sec column matches `cells/sec`.

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

When `--format markdown` is supplied the command emits GitHub-flavoured
Markdown tables for both the preset summary and the optional tick cheat sheet,
making it easy to update this source-adjacent document or coordinate current
public guidance in `iroha-docs`.

Passing `--json-out -` writes the prettified JSON to stdout so scripts can capture the same
structure without re-running the command. The JSON payload mirrors the preset table fields and
includes the optional `tick_bandwidth` section when `--tick-table` (or `--tick-values`) is supplied,
allowing SDKs or ops tooling to load the canonical parameters without scraping documentation.

The relay daemon retains the same presets through configuration and runtime overrides for post-upgrade
requalification:

```bash
# Persisted in the relay JSON config
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

The `constant_rate_profile` key accepts `core`, `home`, or `null`. The default remains `core` so
data-centre deployments stay on the higher-duty-cycle plan unless explicitly reconfigured. `null`
is a staging/dogfood preset; stage it only when exercising the SNNet‑17A capability rollout plan after
the Quinn dependency gate has been cleared.

## MTU and padding guidance

- A constant-rate QUIC DATAGRAM carries one exact 1,024 B strict cell (up to 970 logical payload
  bytes after mux, record header, and authentication tag). With QUIC/UDP/IPv6 overhead the packet
  stays below the IPv6 minimum MTU,
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
    "strict": false
  }
  ```
  When enabled, the handshake includes the `snnet.constant_rate` TLV (type `0x0203`)
  with the following payload layout:

  | Offset | Field                          | Notes |
  |--------|--------------------------------|-------|
  | 0      | `version:u8`                   | Currently `1`. |
  | 1      | `flags:u8`                     | `0x01` = strict, `0x00` = best-effort. |
  | 2–3    | `cell_bytes:u16 (little-endian)` | Always `1024` for SNNet-17A profiles. |

  The payload is exactly four bytes. Clients reject alternate lengths, versions,
  reserved flag bits, and cell sizes; a capability advertisement does not carry
  a dummy cell or allocate storage proportional to `cell_bytes`.
- At the wire-protocol level, clients that set the strict flag require every hop to expose the same
  TLV; capability negotiation rejects a strict request when a server advertises best-effort or no
  constant-rate support. The dormant handshake preflight also rejects an otherwise matching strict
  result before responding; it never accepts strict as best-effort instead.
- Defaults keep the capability disabled. The current relay and Sora VPN helper reject all QUIC
  endpoint creation before binding while the lockfile resolves vulnerable Quinn 0.11.9 /
  quinn-proto 0.11.15, so neither best-effort nor strict constant-rate operation is a shipping path.
  Upgrade to quinn-proto 0.11.17 or later and complete end-to-end requalification before activating
  either mode; `strict=true` remains an independent startup configuration error until that work is
  complete.

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
   - `soranet_constant_rate_queue_depth_class{class}` exposes the scheduler's internal per-class
     queues. The dormant strict implementation binds these to authenticated application queues;
     currently reachable best-effort mode normally leaves them empty.
     `soranet_constant_rate_queue_depth` remains the aggregate view.
   - `soranet_constant_rate_low_dummy_events_total` increments whenever the live dummy ratio falls
     below 20 % in the scheduler loop. With no production payload producer, this ratio should stay
     at 100 %; a lower value currently indicates test or future scheduler integration.
   - `soranet_constant_rate_dummy_ratio` reflects cover cells divided by all cells emitted by the
     DATAGRAM scheduler. Once strict activation is qualified, all post-handshake payload must use
     that scheduler; current production negotiation cannot enter that mode.
   - Best-effort negotiation still runs the legacy cover-only DATAGRAM loop. Dashboards must retain
     the negotiated mode label when interpreting those metrics.
5. Observability assets: Grafana board `dashboards/grafana/soranet_constant_rate.json` charts
   queue depth per class, dummy ratio, live neighbor count, and degraded-state markers; the
   companion alert bundle `dashboards/alerts/soranet_constant_rate_rules.yml` fires when dummy
   share bottoms out (<20% for 5 m) or backlog exceeds 32 cells. Run these in staging first to
   validate thresholds before enabling in production.

These guardrails ensure residential operators do not destabilise their access links while still
keeping SoraNet the default transport surface for SoraFS traffic.
