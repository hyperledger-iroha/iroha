## Sumeragi NPoS Multi-Peer Soak Matrix

Milestone A6 closes once the NPoS chaos/performance harness runs across the
multi-peer configurations operators will use in staging. This document tracks
the soak matrix, how to execute it, and what to attach to the sign-off pack we
hand to SREs.

### Default Matrix

| Scenario label | Peers | Purpose |
|----------------|-------|---------|
| `peers4` | 4 | Baseline `f = 1` finality and finite queue-pressure run. |
| `peers7` | 7 | Exercises signed Set A/B geometry with `f = 2`. |
| `peers10` | 10 | Exercises the larger signed committee and queue load with `f = 3`. |

Revision 4 admits only exact `3f + 1` validator counts from 4 through 31.
The matrix runner rejects custom 5-, 6-, 8-, or otherwise nonconforming peer
counts before starting Cargo or creating partial scenario evidence.

The stress scenarios executed for each row map to
`integration_tests/tests/sumeragi_npos_performance.rs` (baseline finality,
and transaction-queue backpressure). The runner names only tests that exist in
the revision-4 harness; retired V1 global RBC, collector-retry, and adaptive
pacemaker test names are rejected by omission.

### Running the Matrix

1. Provision a dedicated host (the scenarios consume significant CPU and memory).
2. Run the helper:

   ```bash
   python3 scripts/run_sumeragi_soak_matrix.py \
     --artifacts-root artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M) \
     --pack artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M)/signoff.zip
   ```

   - `--tests` allows running a subset of stress tests (forwarded to
     `run_sumeragi_stress.py`).
   - `--scenario name=...,peers=...` replaces the default rows; pass multiple
     flags to build a larger matrix. Extra V1 collector fields fail closed.
3. Review the per-scenario subdirectories:
   - `summary.json` + `README.md` (produced by `render_sumeragi_stress_report.py`)
     capture pass/fail status with direct links to stdout/stderr logs.
   - `matrix_report.md` aggregates the results across all scenarios and links to
     the written artefacts.
   - `matrix_report.json` mirrors the Markdown table for automation.
4. When `--pack` is set a ZIP archive is written that contains the entire matrix
   directory and can be shared with on-call staff as part of the sign-off email.

### Sign-Off Pack Checklist

Include the following in the hand-off to operators:

- The zipped matrix artefacts (`signoff.zip` as produced above).
- A short note summarising the host, hardware, and Iroha commit used, plus any
  deviations from the default matrix.
- Links to Grafana dashboards or Prometheus snapshots collected during the run
  (if available).
- Confirmation that `matrix_report.md` shows `pass` for every scenario; failures
  require incident tickets or follow-up bugs before GA.

### Customising the Matrix

- **Peer counts:** the integration tests honour
  `SUMERAGI_NPOS_STRESS_PEERS`. The helper sets it automatically per scenario;
  advanced users can export it manually before running
  `run_sumeragi_stress.py`. Set A, Set B, and the proxy tail are derived from
  the immutable revision-4 roster and are not tunable matrix dimensions.
- **Additional scenarios:** use repeated `--scenario` flags or maintain a JSON
  list and feed it through your own wrapper. All scenarios are recorded in
  `matrix_report.json`.

Keep the matrix under version control alongside the artefacts you share with
SREs so the release audit trail captures exactly which topologies were soaked.
