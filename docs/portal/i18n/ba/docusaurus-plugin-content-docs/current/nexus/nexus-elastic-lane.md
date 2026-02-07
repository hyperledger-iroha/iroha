---
id: nexus-elastic-lane
lang: ba
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
---

:::note Canonical Source
This page mirrors `docs/source/nexus_elastic_lane.md`. Keep both copies aligned until the translation sweep lands in the portal.
:::

# Elastic Lane Provisioning Toolkit (NX-7)

> **Roadmap item:** NX-7 — Elastic lane provisioning tooling  
> **Status:** Tooling complete — generates manifests, catalog snippets, Norito payloads, smoke tests,
> and the load-test bundle helper now stitches slot latency gating + evidence manifests so validator
> load runs can be published without bespoke scripting.

This guide walks operators through the new `scripts/nexus_lane_bootstrap.sh` helper that automates
lane manifest generation, lane/dataspace catalog snippets, and rollout evidence. The goal is to make
it easy to spin up new Nexus lanes (public or private) without hand-editing multiple files or
re-deriving the catalog geometry by hand.

## 1. Prerequisites

1. Governance approval for the lane alias, dataspace, validator set, fault tolerance (`f`), and settlement policy.
2. A finalized validator list (account IDs) and protected namespace list.
3. Access to the node configuration repository so you can append the generated snippets.
4. Paths for the lane manifest registry (see `nexus.registry.manifest_directory` and
   `cache_directory`).
5. Telemetry contacts/PagerDuty handles for the lane so alerts can be wired as soon as the lane
   comes online.

## 2. Generate lane artefacts

Run the helper from the repository root:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Key flags:

- `--lane-id` must match the new entry’s index in `nexus.lane_catalog`.
- `--dataspace-alias` and `--dataspace-id/hash` control the dataspace catalog entry (defaults to the
  lane id when omitted).
- `--validator` can be repeated or sourced from `--validators-file`.
- `--route-instruction` / `--route-account` emit ready-to-paste routing rules.
- `--metadata key=value` (or `--telemetry-contact/channel/runbook`) capture runbook contacts so
  dashboards immediately list the right owners.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` add the runtime-upgrade hook to the manifest
  when the lane requires extended operator controls.
- `--encode-space-directory` invokes `cargo xtask space-directory encode` automatically. Pair it with
  `--space-directory-out` when you want the encoded `.to` file somewhere other than the default.

The script produces three artefacts inside the `--output-dir` (defaults to the current directory),
plus an optional fourth when encoding is enabled:

1. `<slug>.manifest.json` — lane manifest containing the validator quorum, protected namespaces, and
   optional runtime-upgrade hook metadata.
2. `<slug>.catalog.toml` — a TOML snippet with `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`,
   and any requested routing rules. Ensure `fault_tolerance` is set on the dataspace entry to size
   the lane-relay committee (`3f+1`).
3. `<slug>.summary.json` — audit summary describing the geometry (slug, segments, metadata) plus the
   required rollout steps and the exact `cargo xtask space-directory encode` command (under
   `space_directory_encode.command`). Attach this JSON to the onboarding ticket for evidence.
4. `<slug>.manifest.to` — emitted when `--encode-space-directory` is set; ready for Torii’s
   `iroha app space-directory manifest publish` flow.

Use `--dry-run` to preview the JSON/ snippets without writing files, and `--force` to overwrite
existing artefacts.

## 3. Apply the changes

1. Copy the manifest JSON into the configured `nexus.registry.manifest_directory` (and into the cache
   directory if the registry mirrors remote bundles). Commit the file if manifests are versioned in
   your configuration repo.
2. Append the catalog snippet to `config/config.toml` (or the appropriate `config.d/*.toml`). Ensure
   `nexus.lane_count` is at least `lane_id + 1`, and update any `nexus.routing_policy.rules` that
   should point at the new lane.
3. Encode (if you skipped `--encode-space-directory`) and publish the manifest to the Space Directory
   using the command captured in the summary (`space_directory_encode.command`). This produces the
   `.manifest.to` payload Torii expects and records the evidence for auditors; submit via
   `iroha app space-directory manifest publish`.
4. Run `irohad --sora --config path/to/config.toml --trace-config` and archive the trace output in
   the rollout ticket. This proves the new geometry matches the generated slug/kura segments.
5. Restart the validators assigned to the lane once the manifest/catalog changes are deployed. Keep
   the summary JSON in the ticket for future audits.

## 4. Build a registry distribution bundle

Package the generated manifest and overlay so operators can distribute lane governance data without
editing configs on every host. The bundler helper copies manifests into the canonical layout,
produces an optional governance catalog overlay for `nexus.registry.cache_directory`, and can emit a
tarball for offline transfers:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Outputs:

1. `manifests/<slug>.manifest.json` — copy these into the configured
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — drop into `nexus.registry.cache_directory`. Every `--module`
   entry becomes a pluggable module definition, enabling governance-module swap-outs (NX-2) by
   updating the cache overlay instead of editing `config.toml`.
3. `summary.json` — includes hashes, overlay metadata, and operator instructions.
4. Optional `registry_bundle.tar.*` — ready for SCP, S3, or artifact trackers.

Sync the entire directory (or the archive) to each validator, extract on air-gapped hosts, and copy
the manifests + cache overlay into their registry paths before restarting Torii.

## 5. Validator smoke tests

After Torii restarts, run the new smoke helper to verify the lane reports `manifest_ready=true`,
metrics expose the expected lane count, and the sealed gauge is clear. Lanes that require manifests
must expose a non-empty `manifest_path`; the helper now fails immediately when the path is missing so
every NX-7 deployment record includes the signed manifest evidence:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Add `--insecure` when testing self-signed environments. The script exits non-zero if the lane is
missing, sealed, or metrics/telemetry drift from the expected values. Use the
`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, and
`--max-headroom-events` knobs to keep per-lane block height/finality/backlog/headroom telemetry
within your operational envelopes, and couple them with `--max-slot-p95` / `--max-slot-p99`
(plus `--min-slot-samples`) to enforce the NX‑18 slot-duration targets without leaving the helper.

For air-gapped validations (or CI) you can replay a captured Torii response instead of hitting a live
endpoint:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

The recorded fixtures under `fixtures/nexus/lanes/` mirror the artefacts produced by the bootstrap
helper so new manifests can be linted without bespoke scripting. CI exercises the same flow via
`ci/check_nexus_lane_smoke.sh` and `ci/check_nexus_lane_registry_bundle.sh`
(alias: `make check-nexus-lanes`) to prove the NX-7 smoke helper stays aligned with the published
payload format and to ensure bundle digests/overlays remain reproducible.

When a lane is renamed, capture the `nexus.lane.topology` telemetry events (for example with
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) and feed them back into
the smoke helper. The `--telemetry-file/--from-telemetry` flag accepts the newline-delimited log and
`--require-alias-migration old:new` asserts that a `alias_migrated` event recorded the rename:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

The `telemetry_alias_migrated.ndjson` fixture bundles the canonical rename sample so CI can verify
the telemetry parsing path without contacting a live node.

## Validator load tests (NX-7 evidence)

Roadmap **NX-7** requires every new lane to ship a reproducible validator load run. Use
`scripts/nexus_lane_load_test.py` to stitch the smoke checks, slot-duration gates, and slot bundle
manifest into a single artefact set that governance can replay:

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

The helper enforces the same DA quorum, oracle, settlement buffer, TEU, and slot-duration gates used
by the smoke helper and writes `smoke.log`, `slot_summary.json`, a slot bundle manifest, and
`load_test_manifest.json` into the chosen `--out-dir` so load runs can be attached directly to
rollout tickets without bespoke scripting.

## 6. Telemetry & governance follow-ups

- Update the lane dashboards (`dashboards/grafana/nexus_lanes.json` and related overlays) with the
  new lane id and metadata. The generated metadata keys (`contact`, `channel`, `runbook`, etc.) make
  it simple to pre-fill labels.
- Wire PagerDuty/Alertmanager rules for the new lane before enabling admission. The `summary.json`
  next-steps array mirrors the checklist in [Nexus operations](./nexus-operations).
- Register the manifest bundle in the Space Directory once the validator set is live. Use the same
  manifest JSON generated by the helper, signed according to the governance runbook.
- Follow [Sora Nexus operator onboarding](./nexus-operator-onboarding) for smoke tests (FindNetworkStatus, Torii
  reachability) and capture the evidence with the artefact set produced above.

## 7. Dry-run example

To preview the artefacts without writing files:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --dry-run
```

The command prints the JSON summary and the TOML snippet to stdout, allowing quick iteration during
planning.

---

For additional context see:

- [Nexus operations](./nexus-operations) — operational checklist and telemetry requirements.
- [Sora Nexus operator onboarding](./nexus-operator-onboarding) — detailed onboarding flow that references the
  new helper.
- [Nexus lane model](./nexus-lane-model) — lane geometry, slugs, and storage layout used by the tool.
