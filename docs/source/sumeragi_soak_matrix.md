## Sumeragi NPoS Multi-Peer Soak Matrix

Milestone A6 closes once the NPoS chaos/performance harness runs across the
multi-peer configurations operators will use in staging. This document tracks
the soak matrix, how to execute it, and what to attach to the sign-off pack we
hand to SREs.

### Default Matrix

| Scenario label | Peers | `collectors_k` | `redundant_send_r` | Purpose |
|----------------|-------|----------------|--------------------|---------|
| `peers4_k2_r2` | 4     | 2              | 2                  | Baseline 4-peer stress run (matches existing CI jobs). |
| `peers6_k3_r2` | 6     | 3              | 2                  | Validates redundant fan-out and DA availability with an extra collector tier. |
| `peers8_k3_r3` | 8     | 3              | 3                  | Exercises large-cluster gossip fallback and RBC backpressure. |

The stress scenarios executed for each row map to
`integration_tests/tests/sumeragi_npos_performance.rs` (queue backpressure, RBC
store pressure, chunk loss, redundant send retries, and pacemaker jitter
validation).

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
   - `--scenario name=...,peers=...,collectors_k=...,redundant_send_r=...`
     replaces the default rows; pass multiple flags to build a larger matrix.
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

- **Peer counts / collector fan-out:** the integration tests honour the
  environment variables `SUMERAGI_NPOS_STRESS_PEERS`,
  `SUMERAGI_NPOS_STRESS_COLLECTORS_K`, and
  `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`. The helper sets these automatically
  per scenario; advanced users can export them manually before running
  `run_sumeragi_stress.py`.
- **Additional scenarios:** use repeated `--scenario` flags or maintain a JSON
  list and feed it through your own wrapper. All scenarios are recorded in
  `matrix_report.json`.

Keep the matrix under version control alongside the artefacts you share with
SREs so the release audit trail captures exactly which topologies were soaked.
