## SM Performance Capture & Baseline Plan

Status: Drafted — 2025-05-18  
Owners: Performance WG (lead), Infra Ops (lab scheduling), QA Guild (CI gating)  
Related roadmap tasks: SM-4c.1a/b, SM-5a.3b, FASTPQ Stage 7 cross-device capture

### 1. Objectives
1. Record Neoverse medians in `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`. The current baselines are exported from the `neoverse-proxy-macos` capture under `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (CPU label `neoverse-proxy-macos`) with the SM3 compare tolerance widened to 0.70 for aarch64 macOS/Linux. When bare-metal time opens, rerun `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` on the Neoverse host and promote the aggregated medians into the baselines.  
2. Gather matching x86_64 medians so `ci/check_sm_perf.sh` can guard both host classes.  
3. Publish a repeatable capture procedure (commands, artefact layout, reviewers) so future perf gates do not rely on tribal knowledge.

### 2. Hardware Availability
Only Apple Silicon (macOS arm64) hosts are reachable in the current workspace. The `neoverse-proxy-macos` capture is exported as the interim Linux baseline, but capturing bare-metal Neoverse or x86_64 medians still requires the shared lab hardware tracked under `INFRA-2751`, to be run by the Performance WG once the lab window opens. The remaining capture windows are now booked and tracked in the artefact tree:

- Neoverse N2 bare-metal (Tokyo rack B) booked for 2026-03-12. Operators will reuse the commands from Section 3 and store artefacts under `artifacts/sm_perf/2026-03-lab/neoverse-b01/`.
- x86_64 Xeon (Zurich rack D) booked for 2026-03-19 with SMT disabled to reduce noise; artefacts will land under `artifacts/sm_perf/2026-03-lab/xeon-d01/`.
- After both runs land, promote medians into the baseline JSONs and enable the CI gate in `ci/check_sm_perf.sh` (target switch date: 2026-03-25).

Until those dates, only the macOS arm64 baselines can be refreshed locally.

### 3. Capture Procedure
1. **Sync toolchains**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **Generate capture matrix** (per host)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   The helper now writes `capture_commands.sh` and `capture_plan.json` under the target directory. The script sets up `raw/*.json` capture paths per mode so lab technicians can batch the runs deterministically.
3. **Run captures**  
   Execute each command from `capture_commands.sh` (or run the equivalent manually), ensuring every mode emits a structured JSON blob via `--capture-json`. Always supply a host label via `--cpu-label "<model/bin>"` (or `SM_PERF_CPU_LABEL=<label>`) so the capture metadata and subsequent baselines record the exact hardware that produced the medians. The helper already supplies the appropriate path; for manual runs the pattern is:
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **Validate results**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   Ensure variance stays within ±3% between runs. If not, rerun the affected mode and note the retry in the log.
5. **Promote medians**  
   Use `scripts/sm_perf_aggregate.py` to compute medians and copy them into the baseline JSON files:
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   The helper groups captures by `metadata.mode`, validates that each set shares the
   same `{target_arch, target_os}` triple, and emits a JSON summary with one entry
   per mode. The medians that should land in the baseline files live under
   `modes.<mode>.benchmarks`, while the accompanying `statistics` block records
   the full sample list, min/max, mean, and population stdev for reviewers and CI.
   Once the aggregated file exists, you can auto-write the baseline JSONs (with
   the standard tolerance map) via:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   Override `--mode` to restrict to a subset or `--cpu-label` to pin the
   recorded CPU name when the aggregated source omits it.
   Once both hosts per architecture finish, update:
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (new)

   The `aarch64_unknown_linux_gnu_*` files now reflect the `m3-pro-native`
   capture (cpu label and metadata notes preserved) so `scripts/sm_perf.sh` can
   auto-detect aarch64-unknown-linux-gnu hosts without manual flags. When the
   bare-metal lab run completes, rerun `scripts/sm_perf.sh --mode <mode>
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json`
   with the new captures to overwrite the interim medians and stamp the real
   host label.

   > Reference: the July 2025 Apple Silicon capture (CPU label `m3-pro-local`) is
   > archived under `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`.
   > Mirror that layout when you publish the Neoverse/x86 artefacts so reviewers
   > can diff the raw/aggregated outputs consistently.

### 4. Artefact Layout & Sign-off
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` records the command hash, git revision, operator, and any anomalies.
- Aggregated JSON files feed directly into the baseline updates and are attached to the performance review in `docs/source/crypto/sm_perf_baseline_comparison.md`.
- QA Guild reviews the artefacts before baselines change and signs off in `status.md` under the Performance section.

### 5. CI Gating Timeline
| Date | Milestone | Action |
|------|-----------|--------|
| 2025-07-12 | Neoverse captures complete | Update `sm_perf_baseline_aarch64_*` JSON files, run `ci/check_sm_perf.sh` locally, open PR with artefacts attached. |
| 2025-07-24 | x86_64 captures complete | Add new baseline files + gating in `ci/check_sm_perf.sh`; ensure cross-arch CI lanes consume them. |
| 2025-07-27 | CI enforcement | Enable the `sm-perf-gate` workflow to run on both host classes; merges fail if regressions exceed configured tolerances. |

### 6. Dependencies & Communication
- Coordinate lab access changes via `infra-ops@iroha.tech`.  
- Performance WG posts daily updates in the `#perf-lab` channel while captures run.  
- QA Guild prepares the comparison diff (`scripts/sm_perf_compare.py`) so reviewers can visualise deltas.  
- Once baselines merge, update `roadmap.md` (SM-4c.1a/b, SM-5a.3b) and `status.md` with capture completion notes.

With this plan the SM acceleration work gains reproducible medians, CI gating, and a traceable evidence trail, satisfying the “reserve lab windows & capture medians” action item.

### 7. CI Gate & Local Smoke

- `ci/check_sm_perf.sh` is the canonical CI entrypoint. It shells out to `scripts/sm_perf.sh` for each mode in `SM_PERF_MODES` (defaults to `scalar auto neon-force`) and sets `CARGO_NET_OFFLINE=true` so benches run deterministically on the CI images.  
- `.github/workflows/sm-neon-check.yml` now calls the gate on the macOS arm64 runner so every pull request exercises the scalar/auto/neon-force trio via the same helper used locally; the complementary Linux/Neoverse lane will hook in once the x86_64 captures land and the Neoverse proxy baselines are refreshed with the bare-metal run.  
- Operators can override the mode list locally: `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` trims the run to a single pass for a quick smoke test, while additional arguments (for example `--tolerance 0.20`) are forwarded directly to `scripts/sm_perf.sh`.  
- `make check-sm-perf` now wraps the gate for developer convenience; CI jobs can invoke the script directly while macOS developers piggy-back on the make target.  
- Once the Neoverse/x86_64 baselines land, the same script will pick up the appropriate JSON via the host auto-detection logic already present in `scripts/sm_perf.sh`, so no extra wiring is needed in the workflows beyond setting the desired mode list per host pool.

### 8. Quarterly refresh helper

- Run `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` to mint a quarter-stamped directory such as `artifacts/sm_perf/2026-Q1/<label>/`. The helper wraps `scripts/sm_perf_capture_helper.sh --matrix` and emits `capture_commands.sh`, `capture_plan.json`, and `quarterly_plan.json` (owner + quarter metadata) so lab operators can schedule runs without hand-written plans.
- Execute the generated `capture_commands.sh` on the target host, aggregate the raw outputs with `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json`, and promote the medians into the baseline JSONs via `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite`. Re-run `ci/check_sm_perf.sh` to confirm the tolerances stay green.
- When hardware or toolchains change, refresh comparison tolerances/notes in `docs/source/crypto/sm_perf_baseline_comparison.md`, tighten `ci/check_sm_perf.sh` tolerances if the new medians stabilise, and align any dashboard/alert thresholds with the new baselines so ops alarms stay meaningful.
- Commit `quarterly_plan.json`, `capture_plan.json`, `capture_commands.sh`, and the aggregated JSON alongside the baseline updates; attach the same artefacts to the status/roadmap updates for traceability.
