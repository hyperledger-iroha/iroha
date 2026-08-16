<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sumeragi Randomness & Evidence Runbook

This guide satisfies the Milestone A6 roadmap item that required refreshed
operator procedures for VRF randomness and slashing evidence. Use it alongside
{doc}`sumeragi` and {doc}`sumeragi_chaos_performance_runbook` whenever you stage
a new validator build or capture readiness artefacts for governance.


For the first release, VRF penalties jail offenders after the governed activation
lag. `SumeragiNposParameters.reconfig.slashing_delay_blocks` delays consensus
slashing so governance can cancel it with `CancelConsensusEvidencePenalty`
before it applies; this is governed chain state, not local `[sumeragi]` config.

## Scope & prerequisites

- `iroha_cli` configured for the target cluster (see `specs/cli.md`).
- An allow-listed operator key in an absolute runtime-only mode-`0600` file.
  For brevity, every `iroha ... ops sumeragi ...` command below assumes the
  global `--operator-private-key-file /absolute/runtime/operator.key` option;
  there is no account-key, token, environment, or TOML fallback.
- `curl`/`jq` for scraping the Torii `/status` payload when preparing inputs.
- Prometheus access (or snapshot exports) for the `sumeragi_vrf_*` metrics.
- Awareness of the current epoch and roster so you can match CLI output to the
  staking snapshot or governance manifest.

## 1. Confirm mode selection and epoch context

1. Run `iroha --output-format text ops sumeragi status` to prove that the signed
   revision-4 height context selected NPoS and to record its context fingerprint,
   committee, and epoch. Use `ops sumeragi params` only to inspect governed NPoS
   election/reconfiguration records; it is not a mutable local mode selector.
2. Inspect the runtime view:

   ```bash
   iroha --output-format text ops sumeragi status
   iroha --output-format text ops sumeragi telemetry
   ```

   The status output records authoritative leader/view and durable consensus
   state. Collector/RBC-named fields in the legacy aggregate telemetry endpoint
   are compatibility diagnostics and may remain zero on revision 4.
3. Capture the epoch number you intend to audit:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   Store the value (decimal or `0x` prefixed) for the VRF commands below.

## 2. Snapshot VRF epochs and penalties

Use the dedicated CLI subcommands to pull the persisted VRF records from each
validator:

```bash
iroha --output-format text ops sumeragi vrf-epoch --epoch "$EPOCH"
iroha ops sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha --output-format text ops sumeragi vrf-penalties --epoch "$EPOCH"
iroha ops sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

The summaries show whether the epoch is finalized, how many participants
submitted commits/reveals, the roster length, and the derived seed. The JSON
captures the participant list, per-signer penalty status, and the `seed_hex`
value used for deterministic election. Compare the participant count against the staking
roster, and verify that the penalty arrays reflect the alerts triggered during
chaos testing (late reveals should appear under `late_reveals`, forfeited
validators under `no_participation`).

## 3. Monitor VRF telemetry and alerts

Prometheus exposes the counters required by the roadmap:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

Example PromQL for the weekly report:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

During readiness drills confirm that:

- `sumeragi_vrf_commits_emitted_total` and `..._reveals_emitted_total` increase
  for every block inside the commit/reveal windows.
- Late-reveal scenarios trigger `sumeragi_vrf_reveals_late_total` and clear the
  matching entry in the `vrf_penalties` JSON.
- `sumeragi_vrf_no_participation_total` spikes only when you intentionally
  withhold commits during chaos testing.

The Grafana overview (`specs/grafana_sumeragi_overview.json`) includes
panels for each counter; capture screenshots after every run and attach them to
the artefact bundle referenced in {doc}`sumeragi_chaos_performance_runbook`.

## 4. Evidence observation and streaming

Slashing evidence is admitted through the authenticated consensus peer path and
exposed read-only by Torii. Use the CLI helpers to demonstrate parity with the HTTP endpoints documented in
{doc}`torii/sumeragi_evidence_app_api`:

```bash
# Count and list persisted evidence
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 5

# Show JSON for audits
iroha ops sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

Verify that the reported `total` matches the Grafana widget fed by
`sumeragi_evidence_records_total`, and confirm that records older than
`SumeragiNposParameters.reconfig.evidence_horizon_blocks` are rejected. Alert drills must
produce evidence through the authenticated peer protocol; Torii and the CLI do
not provide an evidence-injection path.

Monitor `/v1/events/sse` with a filtered stream to prove SDKs see the same data:
reuse the Python one-liner from {doc}`torii/sumeragi_evidence_app_api` to build
the filter and capture the raw `data:` frames. The SSE payloads should echo the
evidence kind and signer that appeared in the CLI output.

## 5. Evidence packaging and reporting

For every rehearsal or release candidate:

1. Store the CLI JSON files (`vrf_epoch_*.json`, `vrf_penalties_*.json`,
   `evidence_snapshot.json`) under the run’s artifact directory (the same root
   used by the chaos/performance scripts).
2. Record the Prometheus query results or snapshot exports for the counters
   listed above.
3. Attach the SSE capture and alert acknowledgements to the artefact README.
4. Update `status.md` and
   `specs/project_tracker/npos_sumeragi_phase_a.md` with the artifact
   paths plus the epoch number you inspected.

Following this checklist keeps VRF randomness proofs and slashing evidence
auditable during the NPoS rollout and gives governance reviewers a deterministic
trail back to the captured metrics and CLI snapshots.

## 6. Troubleshooting signals

- **Mode or roster mismatch** — Compare the authoritative Sumeragi status and
  signed genesis/governed height context with the intended consensus mode,
  revision-4 `3f + 1` roster, and VRF seed. Correct the signed chain input (or
  governed chain state), restart the validator, and re-run the validation flow
  described in {doc}`sumeragi`. Legacy `consensus_mode` or collector fields in
  compatibility output are not runtime authority.
- **Missing commits or reveals** — A flat `sumeragi_vrf_commits_emitted_total`
  or `sumeragi_vrf_reveals_emitted_total` time series means the authenticated
  peer path is not broadcasting VRF frames. Check the validator logs for
  `handle_vrf_*` errors, restore peer connectivity, and let the validator
  rebroadcast the frame through consensus.
- **Unexpected penalties** — When `sumeragi_vrf_no_participation_total` spikes,
  cross-check the `vrf_penalties_<epoch>.json` file to confirm the signer ID and
  compare it with the staking roster. Penalties that do not align with chaos
  drills indicate either a validator clock skew or Torii replay protection; fix
  the offending peer before rerunning the test.
- **Evidence ingestion stalls** — When `sumeragi_evidence_records_total`
  plateaus while chaos tests emit faults, run `iroha ops sumeragi evidence count`
  on multiple validators and confirm `/v1/sumeragi/evidence/count` matches the
  CLI output. Any divergence means SSE/webhook consumers may also be stale, so
  compare authenticated peer-ingress logs and repair the diverging validator.
  Evidence cannot be injected through Torii.
