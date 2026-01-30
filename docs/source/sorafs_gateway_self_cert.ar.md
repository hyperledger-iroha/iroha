---
lang: ar
direction: rtl
source: docs/source/sorafs_gateway_self_cert.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dea5a18153f27d4ff7d6f334c915591841900561d37d53ea104986fd0d3c26ef
source_last_modified: "2026-01-03T18:07:57.602009+00:00"
translation_last_reviewed: 2026-01-30
---

title: SoraFS Gateway Self-Certification Kit
summary: Operator workflow for generating signed gateway attestation bundles (SF-5a).
---

# SoraFS Gateway Self-Certification Kit

This guide explains how operators run the self-cert harness, produce a signed
attestation bundle, and archive the results as part of the onboarding checklist.

## Deliverables

- **Harness runner:** `cargo xtask sorafs-gateway-attest` executes the replay + load scenarios, verifies success, and emits artefacts (`report.json`, attestation `.to`, human summary).
- **Wrapper script:** `scripts/sorafs_gateway_self_cert.sh` wraps the xtask command with friendly flags so Ops can call it from CI or shell.
- **Report template:** `docs/source/examples/sorafs_gateway_self_cert_template.json` demonstrates the JSON structure captured in every run (helpful for dashboard ingestion or compliance reviews).

## Prerequisites

- Workspace with `xtask` available (`cargo xtask --help` should list `sorafs-gateway-attest`).
- Config file (key=value) that records the signing key path, signer account, and
  any optional manifest verification inputs. See
  `docs/examples/sorafs_gateway_self_cert.conf` for a template.
- Access to the staging/production gateway endpoint you want to certify.
- Ed25519 signing key in hex (no prefix) tied to the operator’s admission account.
- Optional: custom output directory; defaults to `artifacts/sorafs_gateway_attest`.

## Running the Kit

- Provide options directly or place them in a config file (see
  `docs/examples/sorafs_gateway_self_cert.conf`). Flags override config entries.

```bash
./scripts/sorafs_gateway_self_cert.sh \
  --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest-bundle path/to/updated_manifest.bundle.json
```

- The script forwards arguments to `cargo xtask sorafs-gateway-attest …` and, when
  manifest inputs are present, to `sorafs_cli manifest verify-signature`.
- `--gateway` is optional; omit it (or remove it from the config file) to use the
  harness’ default fixture target.
- Use `--workspace` if the repository root differs from your current directory.

## Output Artefacts

The run creates three files:

| File | Description |
|------|-------------|
| `sorafs_gateway_report.json` | Canonical Norito/JSON run report (matches the template under `docs/source/examples/`). |
| `sorafs_gateway_attestation.to` | Signed Norito envelope containing payload hash, signer metadata, and Ed25519 signature. |
| `sorafs_gateway_attestation.txt` | Human-readable summary suitable for change tickets. |

The JSON report includes:
- Gateway metadata (`gateway.target`, optional `gateway.version` when provided via env/flags).
- Scenario results and metrics (see template).
- Stream-token refusal counters, chunk retry rate, provider reports.
- Payload hash + signature block.

## Verifying the Attestation

1. Inspect the summary: `cat artifacts/.../sorafs_gateway_attestation.txt`.
2. Verify the signature using `norito::decode_from_bytes` or the helper in `xtask`:
   ```bash
   cargo xtask sorafs-verify-attestation \
     --envelope artifacts/.../sorafs_gateway_attestation.to
   ```
3. Archive `report.json` and the summary in the onboarding ticket; submit the `.to` envelope to governance tooling if required.

## Optional Manifest Verification

If no flags or config values are supplied the script falls back to the sample
fixtures under `fixtures/sorafs_manifest/ci_sample/` (including the sample key
`gateway_attestor.hex`), allowing a dry-run out of the box. Provide `--manifest`
(either via the config file or through flags) together with either:

- `--manifest-bundle` (preferred, verifies bundle metadata and signature), or
- `--manifest-signature` plus `--public-key-hex` (detached signature flow).

The wrapper invokes `sorafs_cli manifest verify-signature` after the harness
completes and writes the verification summary to
`<out>/manifest.verify.summary.json`. You can pass `--chunk-plan`,
`--chunk-summary`, or `--chunk-digest-sha3` so the CLI also cross-checks chunk
digests and metadata embedded in the bundle.

## Denylist Diff Evidence (MINFO-6)

When rotating SoraFS gateway denylists, governance expects a before/after trail
highlighting every entry that changed. The self-cert wrapper now wires directly
into the `cargo xtask sorafs-gateway denylist diff` helper:

- Provide `--denylist-old <bundle.json>` and `--denylist-new <bundle.json>`
  (either via flags or config). The script validates both paths exist and then
  executes the diff command. Supply `--denylist-report <path>` to override where
  the JSON report lands; otherwise it defaults to
  `<out>/denylist_diff.json`.
- The command prints the counts of added/removed entries and leaves a JSON
  evidence bundle mirroring the xtask output (MINFO-6 audit format). Attach this
  to the Ministry governance packets alongside the attestation artefacts.
- When only one of the `--denylist-*` flags is present the script skips the
  diff run and emits a warning, preventing partial runs from producing
  misleading evidence.

## Troubleshooting

- If any scenario fails, the xtask command aborts and no attestation is produced. Review the harness output and follow the refusal guidance in `docs/source/sorafs_gateway_refusal_guidance.md` before re-running.
- Persistent 5xx or refusal spikes should be treated as incidents; collect telemetry from the dashboards listed in the deployment handbook.

## Automation Tips

- Integrate the script into CI pipelines (e.g., GitHub Actions) to generate a fresh attestation after each gateway rollout.
- `.github/workflows/sorafs-gateway-self-cert.yml` consumes a config file and
  archives both the attestation outputs and `manifest.verify.summary.json`, keeping
  the verification run reproducible without relying on environment variables.
- Trigger the workflow with:

  ```bash
  gh workflow run sorafs-gateway-self-cert \
    --ref main \
    --field config_path=docs/examples/sorafs_gateway_self_cert.conf
  ```
- Keep the manifest artefacts alongside the gateway outputs so CI can call the
  script with `--manifest`/`--manifest-bundle` and fail fast on signature drift.
- Use `--gateway-manifest-id` / related flags on `sorafs-fetch` (see the deployment handbook) for supplementary smoke tests prior to running the full self-cert suite.

## References

- Deployment & operations handbook: `docs/source/sorafs_gateway_deployment_handbook.md`
- Conformance/load harness: `docs/source/sorafs_gateway_conformance.md`
- Report template: `docs/source/examples/sorafs_gateway_self_cert_template.json`
- Config template: `docs/examples/sorafs_gateway_self_cert.conf`
