---
lang: ba
direction: ltr
source: docs/source/soranet/chaos_game_day.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f47063da1b63735991ae432d6f9472efa7cace04554cefd0b1af9e532474aa36
source_last_modified: "2025-12-29T18:16:36.183791+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet Chaos GameDay (SNNet-15F1)

Quarterly GameDays exercise the CDN control plane by withdrawing a prefix, pausing the
trustless verifier, and throttling the resolver so SRE can validate alerts, drills, and
recovery steps. The automation lives in `cargo xtask soranet-chaos-kit` and records evidence
for each run.

## Generate the kit

```bash
cargo xtask soranet-chaos-kit \
  --out artifacts/soranet/chaos_game_day/$(date +%Y%m%d) \
  --pop soranet-pop-m0 \
  --gateway soranet-gw-m0 \
  --resolver soradns-resolver-m0
```

Outputs:

- `plan.json` and `plan.md` — scenario metadata, targets, and detection/success criteria.
- `chaos_events.ndjson` — shared log for inject/detect/recover events.
- `scripts/` — per-scenario inject/rollback helpers plus `log_event.sh` for manual event
  logging.

Set `--quarter` to pin the report label (default: derived from `--now`/current time).

## Run a drill

1. `cd` into the kit directory with SSH access to the target hosts.
2. Run a scenario script with `APPLY=1` to perform the fault; omit `APPLY` for a dry-run:
   - `APPLY=1 CHAOS_HOLD_SECONDS=120 ./scripts/prefix_withdrawal.sh`
   - `APPLY=1 ./scripts/trustless_verifier_failure.sh`
   - `APPLY=1 CHAOS_HOLD_SECONDS=90 ./scripts/resolver_brownout.sh`
3. Record detection and recovery timestamps:
   - `./scripts/log_event.sh prefix-withdrawal detect "alerts fired"`
   - `./scripts/log_event.sh prefix-withdrawal recover "routes restored"`
4. Summarise the NDJSON log:

   ```bash
   cargo xtask soranet-chaos-report \
     --log chaos_events.ndjson \
     --out chaos_summary.json
   ```

The summary reports detection/recovery deltas per scenario plus maxima across the run; attach
it to the GameDay evidence bundle.

## Scenario expectations

- **Prefix withdrawal** — BGP/route alerts fire, gateway traffic falls back, prefix restored and
  health clears within the drill window.
- **Trustless verifier failure** — Proof-required fetches fall back with clear errors; service
  restart drains backlog and alerts auto-resolve.
- **Resolver brownout** — Resolver latency/timeout alerts trigger, gateway header audits show
  fallback cache behaviour; latency returns under SLO after restart.

Keep the `chaos_events.ndjson` log and the generated summary in `artifacts/soranet/chaos_game_day/`
per run to preserve the quarterly evidence trail.
