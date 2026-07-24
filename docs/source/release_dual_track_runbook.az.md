---
lang: az
direction: ltr
source: docs/source/release_dual_track_runbook.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: ad5fbbe8c942c16fcfc88991025e264d8aaccd813e6ba8cd7d85d1cef268b9cc
source_last_modified: "2026-01-05T09:28:12.043743+00:00"
translation_last_reviewed: 2026-02-07
---

//! Dual-track (Iroha 2 / Iroha 3) release runbook.

# Dual-Track Release Runbook (Iroha 2 / Iroha 3)

This runbook captures the branching, build, validation, and publication flow required to ship simultaneous Iroha 2 (self-hosted) and Iroha 3 (Sora Nexus) releases. It complements `docs/source/release_procedure.org` and `docs/source/release_artifact_selection.md`, focusing on the dual-artifact specifics that surfaced in the roadmap Milestone R3.

## Scope & Roles
- **Release Manager** — coordinates the schedule, drives the checklist, owns branching/tagging.
- **Core Engineering Lead** — approves code freeze and validates consensus/perf gates.
- **Ops / DevRel** — verify packaging, update operator docs, announce availability.
- **Security Review** — approves the Ed25519 public-key fingerprint, HSM
  ceremony, signer rotation/revocation, and artifact-signature results.

## Branching & Tagging Strategy
| Step | Branch/Tag | Purpose |
|------|------------|---------|
| D‑7 | `release/iroha2/vX.Y.Z-rc` & `release/iroha3/vX.Y.Z-rc` branched from `main` | Freeze candidate commits for each track. |
| D‑5 | `release/iroha2/vX.Y.Z` & `release/iroha3/vX.Y.Z` (fast-forward) | Promote RC branch after smoke fixes, keep RC branch for hotfixes. |
| D‑2 | `stable/iroha2` & `stable/iroha3` (fast-forward) | Update long-lived stable tips; use for hotfix cherry-picks. |
| Release | Tags `iroha2-vX.Y.Z` & `iroha3-vX.Y.Z` on respective release branches | Immutable snapshot referenced by manifests and SDKs. |
| Post | Merge `release/*` back to `main` via PR | Ensure fixes land on trunk; resolve conflicts immediately. |

**Notes**
- Keep branch protection in place (CI + review) even for release branches.
- Tag ordering: create `iroha2-v…` first, validate Sora Nexus gating before pushing `iroha3-v…`.
- If an emergency fix is required, cherry-pick onto both release branches and regenerate tags (`git tag -f` + `git push --force --tags`) only after stakeholder approval.

## Release Timeline (Relative to Target Release Date)
| Day | Event | Owner | Checks |
|-----|-------|-------|--------|
| D‑7 | Code freeze + release planning call | Release Manager | Outstanding PR triage, roadmap review. |
| D‑6 | Branch cut (see above) | Release Manager | CI green on `main`, feature flags documented. |
| D‑5 | Profile build smoke (`ci/dual_profile_smoke.sh`) | Core Lead | Build logs archived in `artifacts/smoke/`. |
| D‑3 | Chaos/perf deltas (NPoS, Nexus) | SRE / Core | Attach metrics to release ticket. |
| D‑2 | Final validation matrix (see below) | Release Manager | All rows pass; blockers escalated. |
| D‑1 | HSM-sign artifacts, stage images | Security / Core | Checksums, Ed25519 signatures, and reviewed fingerprint recorded. |
| R0 | Publish artefacts, cut GitHub releases, send announcements | Ops / DevRel | Release notes merged. |
| R+1 | Post-release retrospective + tracker updates | All | Update `status.md`, roadmap, incident log. |

### Release ticket template

Attach the following artefacts to every release ticket (per track) before seeking approval:

- **Validation matrix & manifests** — `dual_profile_matrix.json`, `artifacts/release_manifest.json`, and any profile diffs noted in `artifacts/network_profiles.json`. Run `ci/dual_profile_matrix.sh --output artifacts/dual_profile_matrix.json dist/iroha2-<ver>-linux.tar.zst dist/iroha3-<ver>-linux.tar.zst` once the tarballs are produced. The helper unwraps each bundle, verifies the executables/configuration set, records SHA256 + size metadata, and captures the stdout from `irohad --version` / `kagami --help` so reviewers can diff the evidence without manually unpacking artifacts. Attach the JSON output together with the manifest files in every approval ticket.
- **FASTPQ Stage 6/7 evidence** — the latest `fastpq_metal_bench_*.json` capture (20 k rows padded to 32,768) showing `gpu_mean_ms ≤ 950` for the LDE entry, the paired stdout/trace log, and the signed `fastpq_bench_manifest.{json,sig}` bundle produced by `cargo xtask fastpq-bench-manifest`. Stage 7 adds the operator artefacts captured in `docs/source/fastpq_rollout_playbook.md`: the Grafana `fastpq-acceleration` export with rollout annotations, the alert pack snapshot, and the rollback drill logs/metrics. Bundle everything under `artifacts/fastpq_rollouts/<stamp>/` and link the directory in the ticket so reviewers can replay the evidence. `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (token via `GRAFANA_TOKEN`) writes the Grafana export + alert pack into the bundle automatically, and `ci/check_fastpq_rollout.sh` validates the bundle before attaching it to the ticket.【docs/source/fastpq_plan.md:270】【docs/source/fastpq_rollout_playbook.md:1】
- **Security review + compliance artefacts** — latest SM/SoraFS memos, approval hashes, and any export filings referenced in the rollout ticket (see `docs/source/crypto/sm_operator_rollout.md` and `docs/source/sorafs/developer/releases.md`).
- **Android SDK Maven/SBOM bundle** — run `scripts/run_release_pipeline.py --publish-android-sdk [--android-sdk-repo-url …]` so the release output includes `android/maven/`, `android/sbom/`, `android/README.txt`, `publish_summary.json`, and `checksums.txt`. Attach the README and summary to the ticket and reference any remote Maven promotion URL used for publication.

### SM Feature Gating & Readiness Reviews
| Stage | Milestone | Required artefacts |
|-------|-----------|--------------------|
| SM-RR1 (Verify-only) | Prior to enabling SM signing (adding `sm2` to `crypto.allowed_signing`) on mainnet validators | Approved checklist in rollout ticket referencing `docs/source/crypto/sm_operator_rollout.md`, compliance/export brief updated in current cycle, logs from `scripts/sm_openssl_smoke.sh` and `scripts/sm_interop_matrix.sh`, manifests/config snapshots showing `allowed_signing = ["ed25519"]` and `default_hash = "sm3-256"`. |
| SM-RR2 (Signing pilot) | Before adding `Sm2` to `allowed_signing` for the pilot cohort | Closed external-audit finding or compensating control, pilot manifest allowlist, operator rollback playbook, telemetry baseline diff from verify-only stage. |
| SM-RR3 (GA signing) | Before GA release notes advertise SM signing support | Positive pilot report, updated jurisdiction filings, Release Eng + Crypto WG + Ops/Legal sign-off recorded in release ticket, manifests/configs for all lanes refreshed, SDK parity review completed. |

Record the outcome of each review in the release tracker and refuse promotion to the next stage until every artefact is attached. This gating replaces ad-hoc approvals for SM enablement.

## Cadence & Ownership
- **Monthly release heartbeat:** Continue the existing cadence (week containing the 28th). Calendar invite covers Core Engineering, Nexus Program, SDK leads, Security, and DevRel. Agenda: freeze status, validation progress, SDK blockers, and sign-off readiness.
- **Bi-weekly readiness huddle:** Optional 30‑minute sync during crunch (D‑14, D‑7) to burn down checklist; chaired by the Release Manager, alternating focus on Iroha 2 vs. Iroha 3 risk items.

| Track | Primary Sign-off Owner | Backup | Responsibilities |
|-------|-----------------------|--------|------------------|
| Iroha 2 (self-hosted) | Core Release TL (Core Engineering) | On-call Core Engineer | Approve validation matrix, ensure single-lane smoke/chaos tests pass, sign configuration manifests. |
| Iroha 3 (Sora Nexus) | Nexus Program Ops Lead | Nexus Reliability Engineer | Approve Nexus lane tests, DA manifests, Connect/ISO bridge readiness, coordinate Sora council acknowledgements. |
| Cross-cutting | Security Review Board Rep | Release Manager | Sign hash manifests, audit key custody, approve publication timing. |
| Communications | DevRel Lead | Product Marketing | Publish release notes, stakeholder updates, and SDK parity status. |

- **Escalation path:** Any blocker discovered during validation escalates to Release Manager → Core Lead → Exec sponsor within 24 hours. Document decisions in the release ticket.
- **Post-release review:** Schedule at R+7 with all owners to confirm metrics, log follow-up actions, and reassess role assignments after the first combined release. The inaugural dual-track review (2026-03-08, release ticket RLS-102) confirmed the frozen validation matrix, reassigned the SDK parity checklist to DevRel, and closed the telemetry dashboard backlog; repeat the same agenda for future cycles.

## Build Matrix & Commands

Set the non-secret signing inputs from the reviewed release ticket:

```bash
EXTERNAL_SIGNER=/opt/iroha/bin/pkcs11-ed25519-sign
SIGNING_PUBLIC_KEY=/run/iroha-release/ed25519-public.raw
TRUSTED_SIGNING_FINGERPRINT=<reviewed-lowercase-sha256>
RELEASE_MANIFEST_VERIFIER=/opt/iroha/bin/sorafs-validate
TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256=<reviewed-lowercase-sha256>
```

The signer executable obtains its private-key handle and authentication from
the runtime PKCS#11/HSM environment. Never pass private key bytes, a PIN, or a
bearer token on the command line. The verifier path must identify the packaged
candidate reviewed for this release, and its digest must be approved through an
independent channel.

Build the binary bundles:

```bash
scripts/build_release_bundle.sh \
  --profile iroha2 --config single --artifacts-dir dist \
  --external-signer "$EXTERNAL_SIGNER" \
  --signing-public-key "$SIGNING_PUBLIC_KEY" \
  --trusted-signing-fingerprint "$TRUSTED_SIGNING_FINGERPRINT"

scripts/build_release_bundle.sh \
  --profile iroha3 --config nexus --artifacts-dir dist \
  --external-signer "$EXTERNAL_SIGNER" \
  --signing-public-key "$SIGNING_PUBLIC_KEY" \
  --trusted-signing-fingerprint "$TRUSTED_SIGNING_FINGERPRINT"
```

Use the same three signing options with `scripts/build_release_image.sh` for
the two container-image archives.

To run the complete local coordinator, including `git-cliff`, bundles, images,
aggregated checksums, manifest generation, and SoraFS publication-plan
validation:

```bash
scripts/run_release_pipeline.py \
  --version <X.Y.Z> \
  --previous-tag <prior-tag> \
  --external-signer "$EXTERNAL_SIGNER" \
  --signing-public-key "$SIGNING_PUBLIC_KEY" \
  --trusted-signing-fingerprint "$TRUSTED_SIGNING_FINGERPRINT" \
  --release-manifest-verifier "$RELEASE_MANIFEST_VERIFIER" \
  --trusted-release-manifest-verifier-sha256 \
    "$TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256" \
  --publish-target iroha2=sorafs://staging/iroha2/v<X.Y.Z> \
  --publish-target iroha3=sorafs://staging/iroha3/v<X.Y.Z>
```

Outputs land under `artifacts/releases/<X.Y.Z>/`. No checked-in workflow
currently invokes this generic coordinator. `.github/workflows/workspace_release.yml`
is the workspace release gate, while `.github/workflows/sorafs-cli-release.yml`
is the separate canonical SoraFS CLI/reference-validator workflow.

### Packaging Outputs & Determinism
- **Tarballs:** `iroha{2,3}-<version>-<os>.tar.zst` produced via deploy profile binaries + profile configs. Bundles always include `PROFILE.toml` (metadata), `config/` tree, and `bin/` executables. Compression is fixed (`zstd -19 --long=31`) for deterministic bytes.
- **Container images:** `iroha{2,3}-<version>-<os>-image.tar` generated from the Dockerfile using the same deploy binaries. Naming does not embed the registry tag, ensuring reproducible tarball names.
- **Hashes:** Each artifact emits `<name>.sha256`; `scripts/run_release_pipeline.py` collates them into `SHA256SUMS` and `release_manifest.json`, which downstream signing/publication systems use verbatim.
- **Signatures:** The complete external-signer option set makes both builders
  invoke a reviewed PKCS#11/HSM wrapper, require exactly 64 raw Ed25519
  signature bytes, and verify them before and after exclusive installation.
  Each per-artifact `.sig` contains the raw signature and `.pub` is generated
  Ed25519 SPKI PEM; the per-artifact manifest records the exact raw-key SHA-256
  fingerprint and format. After the final evidence update, the pipeline signs
  the deterministic aggregate `release_manifest.json` through the same
  external signer. Its `release_manifest.json.sig` is exactly 64 raw bytes and
  `release_manifest.json.pub` is exactly 32 raw Ed25519 public-key bytes.
- **Manifests:** `generate_release_manifest.py` records profile, format, path,
  and SHA256 for every bundle/AppImage encountered using stable key ordering.
  `scripts/run_release_pipeline.py` may then append an `evidence` block for
  archived rollout artefacts (for example FASTPQ rollout bundles, their
  reviewer-facing summaries, Grafana exports, and CBDC rollout validation
  paths). It signs only after that update. `scripts/publish_plan.py` rejects a
  production plan unless the aggregate Ed25519 signature, raw public key,
  reviewed signing fingerprint, and independently pinned native verifier path
  and SHA-256 all verify through `sorafs-validate release-manifest`. Keep the
  JSON, `.sig`, `.pub`, verifier digest, and candidate identity attached to the
  release checklist and status updates.
- **Deterministic directories:** All artifacts live under `artifacts/releases/<version>/artifacts/`; rerunning the pipeline after a clean build should overwrite the same filenames, enabling reproducibility checks (diff of tarball hashes, manifest comparison).

**Build prerequisites**
- Ensure `cargo xtask gen-version --write` has been run so version metadata matches the target tag.
- Provision the reviewed external signer executable, raw 32-byte Ed25519 public
  key, and independently approved lowercase SHA-256 fingerprint. Signing
  credentials remain runtime-only in the PKCS#11/HSM session.
- Provision the packaged `sorafs-validate` candidate by direct path and obtain
  independent approval of the exact executable's lowercase SHA-256 digest.
- Use the same toolchain/container across both builds to maintain deterministic binaries.

## Validation Matrix
| Category | Iroha 2 Checks | Iroha 3 Checks |
|----------|----------------|----------------|
| Core CI | `cargo test --workspace --locked`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`. | Same. |
| Integration | `cargo test -p integration_tests -- --ignored --nocapture`; `ci/dual_profile_smoke.sh`. | `ci/dual_profile_smoke.sh`; `ci/check_nexus_lane_smoke.sh`; Nexus lane replay tests. |
| Torii | `cargo test -p iroha_torii --features "app_api,transparent_api"` | Same plus Nexus Connect smoke (`IROHA_NEXUS_PROFILE=1`). |
| Config | `scripts/select_release_profile.py --network self-hosted --emit-manifest artifacts/network_profiles.json` | `scripts/select_release_profile.py --network sora-nexus --emit-manifest artifacts/network_profiles_nexus.json`. |
| SDK Parity | Attach the workspace SDK test results and clean-consumer smoke record for the exact bundle commit. | Attach the same results plus Nexus configuration/query parity evidence. |
| Telemetry | Attach the deployment `/metrics` and `/status` verification record. | Include Nexus lane-count and DA telemetry evidence. |

Record pass/fail in the release ticket. Any ❌ requires engineering sign-off before release proceeds.

## Configuration & Manifest Checks
- Run `scripts/select_release_profile.py --list` to confirm network profile mappings.
- Verify `release/network_profiles.toml` includes the new version numbers, lane counts, and default artifact names.
- Attach the generated `artifacts/release_manifest.json`,
  `release_manifest.json.sig`, `release_manifest.json.pub`, and
  `artifacts/network_profiles.json` to the release PR checklist. When FASTPQ or
  CBDC rollout bundles are archived as part of the same run, verify that
  `release_manifest.json.evidence` points at the copied rollout bundle roots
  and any FASTPQ `fastpq_rollout_summary.{json,md}` files before verifying the
  aggregate signature.
- Ensure `defaults/` templates in the bundles match their target (single vs. nexus). If not, regenerate with `cargo xtask gen-config --profile <...>`.

## Approvals & Sign-off
| Gate | Required Approvers | Evidence |
|------|-------------------|----------|
| Freeze Confirmation | Release Manager + Core Lead | Meeting notes in tracker ticket. |
| Validation Matrix | Release Manager + relevant domain owners | Checklist in release PR/checklist. |
| Security/Signing | Security Review + Core Lead | Verified Ed25519 signatures, reviewed raw-key fingerprint, HSM ceremony and rotation/revocation record. |
| Publication | Release Manager + Ops | Bucket upload log, GitHub release draft. |

Document approvals in the release ticket or the `release/YYYY-MM/notes.md` record.

## Publication Flow
1. Generate tarballs/images (Build Matrix).
2. Upload artefacts to the staging buckets (`s3://releases-staging/iroha2/`, `s3://releases-staging/iroha3/`) and container registries (`registry.sora.org/iroha2`, `registry.sora.org/iroha3`).
3. Re-run the complete verification procedure in
   `docs/source/release_artifact_selection.md`: check each checksum, require the
   Ed25519 manifest fields, derive the SHA-256 fingerprint of the raw 32-byte
   key from the downloaded SPKI PEM, compare both manifest and derived values
   with the independently reviewed runtime fingerprint, and run
   `openssl pkeyutl -verify -pubin -rawin` for each per-artifact signature.
   Verify the aggregate raw-key signature separately through the pinned
   `sorafs-validate release-manifest` contract.
4. Promote from staging to the production buckets (`s3://releases/iroha2/`, `s3://releases/iroha3/`) once approval is logged.
5. Create GitHub releases `iroha2-vX.Y.Z` / `iroha3-vX.Y.Z`, attaching:
   - Tarballs, manifests, signatures.
   - SBOM and a vulnerability report with no critical/high findings.
   - OIDC/cosign provenance bundle and verification receipt.
   - Release notes sections linking to status/roadmap updates.
6. Update `docs/source/release_artifact_selection.md` if artifact layout changes.

## SoraFS/SoraNet endpoints, layout, and validation

| Track | Staging root (example) | Production root (example) | Object layout |
|-------|------------------------|---------------------------|---------------|
| Iroha 2 | `sorafs://staging/iroha2/v<ver>` | `sorafs://prod/iroha2/v<ver>` | Payloads at `<root>/iroha2-<ver>-<os>.tar.zst`, `<root>/iroha2-<ver>-<arch>.AppImage`, `<root>/SHA256SUMS`, `<root>/release_manifest.json`, `<root>/release_manifest.json.sig`, `<root>/release_manifest.json.pub`, and `<root>/dual_profile_matrix.json`. |
| Iroha 3 (Nexus) | `sorafs://staging/iroha3/v<ver>` | `sorafs://prod/iroha3/v<ver>` | Same layout as Iroha 2 with `iroha3-*` artefacts. |

Generate and validate publish plans before any upload/promotion:

1. Create the plan and shell wrapper from the release output (supply the SoraFS/SoraNet roots you intend to publish to):

   ```bash
   scripts/publish_plan.py generate \
     --manifest artifacts/releases/<ver>/release_manifest.json \
     --manifest-signature artifacts/releases/<ver>/release_manifest.json.sig \
     --manifest-public-key artifacts/releases/<ver>/release_manifest.json.pub \
     --trusted-signing-fingerprint "$TRUSTED_SIGNING_FINGERPRINT" \
     --release-manifest-verifier "$RELEASE_MANIFEST_VERIFIER" \
     --trusted-release-manifest-verifier-sha256 \
       "$TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256" \
     --artifacts-dir artifacts/releases/<ver>/artifacts \
     --target iroha2=sorafs://staging/iroha2/v<ver> \
     --target iroha3=sorafs://staging/iroha3/v<ver> \
     --output-dir artifacts/releases/<ver>
   ```

2. Validate the plan locally (size/sha parity) and optionally probe staged HTTP(S) gateways after upload. Attach both `publish_plan.json` and `publish_plan_report.json` to the release ticket:

   ```bash
   scripts/publish_plan.py validate \
     --plan artifacts/releases/<ver>/publish_plan.json \
     --trusted-signing-fingerprint "$TRUSTED_SIGNING_FINGERPRINT" \
     --release-manifest-verifier "$RELEASE_MANIFEST_VERIFIER" \
     --trusted-release-manifest-verifier-sha256 \
       "$TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256" \
     --previous-plan artifacts/releases/<prev>/publish_plan.json \
     --probe-remote \
     --probe-command "sorafs_cli head --json {destination}" \
     --output artifacts/releases/<ver>/publish_plan_report.json
   ```

3. Both generation and validation reverify the aggregate Ed25519 signature and
   bind `publish_plan.json` to the exact manifest SHA-256. Both require the
   independently reviewed signing fingerprint and native-verifier path/digest;
   values copied only from the plan are not trust anchors. An unsigned plan is
   rejected unless the explicit `--development-allow-unsigned-manifest` flag
   is supplied to both commands; that escape hatch is test/development-only
   and is never valid for promotion.
4. Any change to the SoraFS/SoraNet roots or object layout requires an explicit approval note in the release ticket. Include the diff emitted by `--previous-plan` when proposing a change; the generator/validator fail fast when required paths are missing or hashes deviate from the manifest.

## Post-Release Actions
- Merge release branches back into `main`, resolve conflicts promptly.
- Update `status.md` with validation highlights and artefact pointers.
- Close roadmap items (Milestone R3 tasks) and move completed work to `status.md`.
- Schedule retrospective; capture lessons learned and backlog action items for the next cycle.
- Archive artefacts (logs, manifests) under `artifacts/releases/vX.Y.Z/`.

## Outstanding actions
- **Validation matrix automation** — Owner: Release Engineering. Target: 2026-04-30.
  - Local state: `ci/dual_profile_matrix.sh` emits the required JSON.
  - External state: no checked-in workflow invokes the generic coordinator;
    hosted build/upload evidence is still required.
- **Signing and provenance evidence** — Owner: Security Review board liaison.
  - Local state: the builders accept no private key, enforce the external
    Ed25519 signer contract, and verify the signature and reviewed fingerprint.
  - External state: attach the PKCS#11/HSM ceremony, rotation/revocation record,
    OIDC/cosign provenance, registry/publication receipts, and rollback/yank
    rehearsal before promotion.
