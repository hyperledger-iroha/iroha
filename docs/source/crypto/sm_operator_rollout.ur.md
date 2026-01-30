---
lang: ur
direction: rtl
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2026-01-03T18:07:57.089544+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM Feature Rollout & Telemetry Checklist

This checklist helps SRE and operator teams enable the SM (SM2/SM3/SM4) feature
set safely once the audit and compliance gates are cleared. Follow this document
alongside the configuration brief in `docs/source/crypto/sm_program.md` and the
legal/export guidance in `docs/source/crypto/sm_compliance_brief.md`.

## 1. Pre-flight Readiness
- [ ] Confirm the workspace release notes show `sm` as verify-only or signing,
      depending on the rollout stage.
- [ ] Verify the fleet is running binaries built from a commit that includes the
      SM telemetry counters and configuration knobs. (Target release TBD; track
      in the rollout ticket.)
- [ ] Run `scripts/sm_perf.sh --tolerance 0.25` on a staging node (per target
      architecture) and archive the summary output. The script now auto-selects
      the scalar baseline as a comparison target for acceleration modes
      (`--compare-tolerance` defaults to 5.25 while the SM3 NEON work lands);
      investigate or block the rollout if either the primary or comparison
      guard fails. When capturing on Linux/aarch64 Neoverse hardware, pass
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      to overwrite the exported `m3-pro-native` medians with the host’s capture
      before shipping.
- [ ] Ensure `status.md` and the rollout ticket record the compliance filings for
      any nodes operating in jurisdictions that require them (see compliance brief).
- [ ] Prepare KMS/HSM updates if validators will store SM signing keys in
      hardware modules.

## 2. Configuration Changes
1. Run the xtask helper to generate the SM2 key inventory and ready-to-paste snippet:
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   Use `--snippet-out -` (and optionally `--json-out -`) to stream the outputs to stdout when you just need to inspect them.
   If you prefer to drive the lower-level CLI commands manually, the equivalent flow is:
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   If `jq` is unavailable, open `sm2-key.json`, copy the `private_key_hex` value, and pass it directly to the export command.
2. Add the resulting snippet to each node’s configuration (values shown for the
   verify-only stage; adjust per environment and keep the keys sorted as shown):
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. Restart the node and confirm `crypto.sm_helpers_available` and (if you enabled the preview backend) `crypto.sm_openssl_preview_enabled` surface as expected in:
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`).
   - The rendered `config.toml` for each node.
4. Update manifests/genesis entries to add SM algorithms to the allow-list if
   signing is enabled later in the rollout. When using `--genesis-manifest-json`
   without a pre-signed genesis block, `irohad` now seeds the runtime crypto
   snapshot directly from the manifest’s `crypto` block—ensure the manifest is
   checked into your change plan before rolling forward.

## 3. Telemetry & Monitoring
- Scrape Prometheus endpoints and ensure the following counters/gauges appear:
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview` (0/1 gauge reporting the preview toggle state)
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- Hook signing path once SM2 signing is enabled; add counters for
  `iroha_sm_sign_total` and `iroha_sm_sign_failures_total`.
- Create Grafana dashboards/alerts for:
  - Spikes in failure counters (window 5m).
  - Sudden drops in SM syscall throughput.
  - Differences between nodes (e.g., mismatched enablement).

## 4. Rollout Steps
| Phase | Actions | Notes |
|-------|---------|-------|
| Verify-only | Update `crypto.default_hash` to `sm3-256`, leave `allowed_signing` without `sm2`, monitor verification counters. | Goal: exercise SM verification paths without risking consensus divergence. |
| Mixed Signing Pilot | Allow limited SM signing (subset of validators); monitor signing counters and latency. | Ensure fallback to Ed25519 remains available; halt if telemetry shows mismatches. |
| GA Signing | Extend `allowed_signing` to include `sm2`, update manifests/SDKs, and publish final runbook. | Requires closed audit findings, updated compliance filings, and stable telemetry. |

### Readiness Reviews
- **Verify-only readiness (SM-RR1).** Convene Release Eng, Crypto WG, Ops, and Legal. Require:
  - `status.md` notes compliance filing status + OpenSSL provenance.
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / this checklist updated within the last release window.
  - `defaults/genesis` or the environment-specific manifest shows `crypto.allowed_signing = ["ed25519","sm2"]` and `crypto.default_hash = "sm3-256"` (or the verify-only variant without `sm2` if still in stage one).
  - `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` logs attached to the rollout ticket.
  - Telemetry dashboard (`iroha_sm_*`) reviewed for steady-state behaviour.
- **Signing pilot readiness (SM-RR2).** Additional gates:
  - Audit report for RustCrypto SM stack closed or RFC for compensating controls signed by Security.
  - Operator runbooks (facility-specific) updated with signing fallback/rollback steps.
  - Genesis manifests for the pilot cohort include `allowed_signing = ["ed25519","sm2"]` and the allow-list is mirrored in each node configuration.
  - Exit/rollback plan documented (switch `allowed_signing` back to Ed25519, restore manifests, reset dashboards).
- **GA readiness (SM-RR3).** Requires positive pilot report, updated compliance filings for all validator jurisdictions, signed telemetry baselines, and release ticket approval from Release Eng + Crypto WG + Ops/Legal triad.

## 5. Packaging & Compliance Checklist
- **Bundle OpenSSL/Tongsuo artifacts.** Ship OpenSSL/Tongsuo 3.0+ shared libraries (`libcrypto`/`libssl`) with every validator package or document the exact system dependency. Record the version, build flags, and SHA256 checksums in the release manifest so auditors can trace the supplier build.
- **Verify during CI.** Add a CI step that executes `scripts/sm_openssl_smoke.sh` against the packaged artifacts on each target platform. The job must fail if the preview flag is enabled but the provider cannot be initialised (missing headers, unsupported algorithm, etc.).
- **Publish compliance notes.** Update release notes / `status.md` with the bundled provider version, export-control references (GM/T, GB/T), and any jurisdiction-specific filings required for SM algorithms.
- **Operator runbook updates.** Document the upgrade flow: stage the new shared objects, restart peers with `crypto.enable_sm_openssl_preview = true`, confirm the `/status` field and `iroha_sm_openssl_preview` gauge flip to `true`, and keep a rollback plan (flip the config flag or revert the package) if preview telemetry deviates across the fleet.
- **Evidence retention.** Archive the build logs and signing attestations for the OpenSSL/Tongsuo packages alongside the validator release artefacts so future audits can reproduce the provenance chain.

## 6. Incident Response
- **Verification failure spikes:** Roll back to a build without SM support or remove `sm2`
  from `allowed_signing` (reverting `default_hash` as needed) and fail over to the previous
  release while investigating. Capture failed payloads, comparative hashes, and node logs.
- **Performance regressions:** Compare SM metrics with Ed25519/SHA2 baselines.
  If ARM intrinsic path causes divergence, set `crypto.sm_intrinsics = "force-disable"`
  (feature toggle pending implementation) and report findings.
- **Telemetry gaps:** If counters are missing or not updating, file an issue
  against Release Engineering; do not proceed with wider rollout until the gap
  is resolved.

## 7. Checklist Template
- [ ] Configuration staged and peer restarted.
- [ ] Telemetry counters visible and dashboards configured.
- [ ] Compliance/legal steps recorded.
- [ ] Rollout phase approved by Crypto WG / Release TL.
- [ ] Post-rollout review completed and findings documented.

Maintain this checklist in the rollout ticket and update `status.md` when the
fleet transitions between phases.
