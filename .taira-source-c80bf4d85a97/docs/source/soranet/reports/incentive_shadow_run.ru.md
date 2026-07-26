---
lang: ru
direction: ltr
source: docs/source/soranet/reports/incentive_shadow_run.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c518f612d788059136846e739033af35428932b5e80c0493f8b89dfde4c7be40
source_last_modified: "2026-01-22T15:38:30.719167+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraNet Relay Incentive Shadow Simulation
summary: Methodology and tooling for 60-day relay incentive shadow runs across live telemetry.
---

# Overview

To promote relay incentives safely, the economics and networking teams must replay long
telemetry windows and verify that payout weights remain fair, resistant to latency regressions,
and resilient to DoS strategies. The `iroha app sorafs incentives service shadow-run` command
replays Norito-encoded telemetry snapshots against the treasury reward engine and
produces fairness diagnostics without minting on-ledger payouts.

This report captures the simulation workflow and the artefacts required to publish
the mandatory **60-day shadow run** before enabling automated payouts.

## Prerequisites

- A fresh incentives state initialised with the production reward configuration
  (`iroha app sorafs incentives service init ...`).
- A daemon config JSON mapping relay fingerprints to beneficiary accounts and bond entries
  (see `DaemonConfigFile` in `crates/iroha_cli/src/commands/sorafs.rs`).
- A telemetry spool directory containing Norito-encoded `RelayEpochMetricsV1`
  snapshots named `relay-<hex>-epoch-<n>.to`. These are the snapshots emitted by the
  orchestrator metrics log (`RewardConfig::metrics_log_path`).
- Optional output directory for writing the generated JSON report.

## Running the Shadow Simulation

```bash
iroha app sorafs incentives service shadow-run \
  --state artifacts/incentives/shadow_state.json \
  --config configs/sorafs/shadow_daemon.json \
  --metrics-dir telemetry/soranet/relay_metrics \
  --report-out reports/soranet/incentive_shadow_run.json \
  --pretty
```

The command:

1. Replays every metrics snapshot in the directory (sorted by epoch and relay).
2. Evaluates the reward calculator using the supplied bond entries and beneficiaries.
3. Emits a fairness report capturing payout distribution, warning/suspended epochs,
   and per-relay averages.
4. Leaves the on-disk state untouched unless you explicitly persist it with the usual
   `process` command.

> ⚠️ **NOTE:** Run the simulation against a clone of the incentives state to keep
> replayed payouts separate from production accounting.

## Sample Output (synthetic data)

```json
{
  "processed_payouts": 3,
  "total_relays": 2,
  "total_payout_nanos": 450000000000,
  "gini_coefficient": 0.044444444444444446,
  "top_relay_share": 0.5555555555555556,
  "zero_score_epochs": 0,
  "warning_epochs": 1,
  "suspended_epochs": 0,
  "average_availability_per_mille": 983.3333333333334,
  "average_bandwidth_per_mille": 933.3333333333334,
  "relays": [
    {
      "relay_id_hex": "2121212121212121212121212121212121212121212121212121212121212121",
      "epochs": 2,
      "payout_nanos": 250000000000,
      "average_payout_nanos": 125000000000.0,
      "average_score_per_mille": 960.0,
      "average_availability_per_mille": 986.0,
      "average_bandwidth_per_mille": 900.0,
      "warning_epochs": 1,
      "suspended_epochs": 0,
      "zero_score_epochs": 0
    },
    {
      "relay_id_hex": "4343434343434343434343434343434343434343434343434343434343434343",
      "epochs": 1,
      "payout_nanos": 200000000000,
      "average_payout_nanos": 200000000000.0,
      "average_score_per_mille": 1000.0,
      "average_availability_per_mille": 944.0,
      "average_bandwidth_per_mille": 1000.0,
      "warning_epochs": 0,
      "suspended_epochs": 0,
      "zero_score_epochs": 0
    }
  ],
  "skipped_missing_config": 0,
  "skipped_missing_bond": 0,
  "skipped_duplicate": 0,
  "errors": []
}
```

The example above uses synthetic telemetry bundled with the CLI tests; real runs
must be carried out on the 60-day production spool to validate fairness and
latency behaviour. The report highlights warning epochs and provides a quick
view of payout skew (via the Gini coefficient and top-share percentage).

## Production 60-Day Replay (2025-10-01 -> 2025-11-29)

The first production replay used commit `9f84f3c7d6ab4d42f3c917e62c13a2ec7fd1f034`
and the live telemetry window `2025-10-01_2025-11-29`. The signed artefacts live
under `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}` and capture
360 processed payouts across the six active relays.

**Key metrics**

- Total payouts: 5,160 XOR (gini coefficient 0.121, top relay share 23.26%).
- No telemetry gaps: `skipped_missing_config|bond|duplicate == 0` and the
  engine produced zero recoverable errors.
- Nine warning epochs and three suspensions were recorded; compliance penalties
  reduced payouts for the affected relays as expected, while other relays held
  availability above 94%.

**Per-relay summary**

| Relay (`relay_id_hex`) | Epochs | Payout (XOR) | Avg Payout (XOR) | Avg Score (%) | Avg Availability (%) | Avg Bandwidth (%) | Warning Epochs | Suspended Epochs | Zero-Score Epochs |
|------------------------|--------|--------------|------------------|---------------|----------------------|-------------------|----------------|------------------|-------------------|
| `1111...1111`          | 60     | 1200         | 20.00            | 98.24         | 99.24                | 94.86             | 2              | 0                | 0                 |
| `2222...2222`          | 60     | 980          | 16.33            | 97.63         | 98.71                | 93.64             | 1              | 0                | 0                 |
| `3333...3333`          | 60     | 860          | 14.33            | 96.37         | 97.53                | 91.27             | 0              | 1                | 1                 |
| `4444...4444`          | 60     | 780          | 13.00            | 95.16         | 96.48                | 90.51             | 2              | 1                | 1                 |
| `5555...5555`          | 60     | 720          | 12.00            | 94.48         | 95.82                | 89.68             | 1              | 0                | 0                 |
| `6666...6666`          | 60     | 620          | 10.33            | 93.12         | 94.05                | 87.23             | 3              | 2                | 2                 |

The shadow-run JSON also encodes the `RelayRewardIteration` diagnostics that feed
the Grafana deck in `dashboards/grafana/soranet_incentives.json` so governance
can correlate payouts with alert history. The attestation bundle (`*.pub`/`*.sig`)
locks the report to the metrics spool and daemon configuration used for the run.

## Next Steps

- Align the telemetry collector to emit rolling 60-day logs and archive them under
  `telemetry/soranet/relay_metrics`.
- Archive each production replay beside this document (`docs/examples/`), commit
  the JSON + detached signature, and update this report with a short fairness
  analysis before requesting automation approvals. The current artefact lives at
  `docs/examples/soranet_incentive_shadow_run.json` with the detached Ed25519
  signature `docs/examples/soranet_incentive_shadow_run.sig`. Verify it with:

  ```bash
  base64 -d -i docs/examples/soranet_incentive_shadow_run.sig -o /tmp/shadow.sig
  openssl pkeyutl -verify -pubin \
    -inkey docs/examples/soranet_incentive_shadow_run.pub \
    -rawin \
    -in docs/examples/soranet_incentive_shadow_run.json \
    -sigfile /tmp/shadow.sig
  ```

  The `report_metadata.git_revision` field inside the JSON captures the workspace commit used for
  the run; the PEM public key shipped beside the artefact (`docs/examples/soranet_incentive_shadow_run.pub`)
  is dedicated to these shadow-run attestations.
- Feed the resulting JSON into Grafana or notebooks to correlate payout variance with
  latency alerts and DoS probes.
