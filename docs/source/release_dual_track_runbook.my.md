---
lang: my
direction: ltr
source: docs/source/release_dual_track_runbook.md
status: complete
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
- **Core Engineering Lead** — approves code freeze, validates consensus/perf gates, signs binaries.
- **Ops / DevRel** — verify packaging, update operator docs, announce availability.
- **Security Review** — sign-off on manifest hashes and signing key custody.

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
| D‑1 | Sign binaries, stage images | Security / Core | Hash/sign manifests generated. |
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
| Profile | Bundle Command | Image Command | Outputs |
|---------|----------------|---------------|---------|
| Iroha 2 | `scripts/build_release_bundle.sh --profile iroha2 --config single --artifacts-dir dist --signing-key <key>` | `scripts/build_release_image.sh --profile iroha2 --config single --artifacts-dir dist --signing-key <key>` | Tarballs (`iroha2-*-linux.tar.zst`), manifests, signatures. |
| Iroha 3 | `scripts/build_release_bundle.sh --profile iroha3 --config nexus --artifacts-dir dist --signing-key <key>` | `scripts/build_release_image.sh --profile iroha3 --config nexus --artifacts-dir dist --signing-key <key>` | Tarballs (`iroha3-*-linux.tar.zst`), manifests, signatures. |

> Tip: to run the full sequence (git-cliff changelog, bundles, images, aggregated checksums, manifest, Android SDK Maven bundle, and an AWS publish plan) invoke `scripts/run_release_pipeline.py --version <X.Y.Z> --previous-tag <prior-tag> --signing-key <key> --publish-bucket s3://bucket/path --publish-android-sdk`. Optional flags `--android-sdk-repo-url/--android-sdk-username/--android-sdk-password` pass remote repository credentials to the Android publisher. Outputs land under `artifacts/releases/<X.Y.Z>/`, with Android artefacts stored in the `android/` subdirectory (Maven repo, SBOM/provenance copy, README, and publish summary).

> Automation: the GitHub Actions workflow `.github/workflows/release-pipeline.yml`
> wraps `scripts/run_release_pipeline.py`, installing `papermill`/`jupyter` so
> `scripts/telemetry/run_privacy_dp_notebook.sh` refreshes the SoraNet privacy
> artefacts on every promotion.

### Packaging Outputs & Determinism
- **Tarballs:** `iroha{2,3}-<version>-<os>.tar.zst` produced via deploy profile binaries + profile configs. Bundles always include `PROFILE.toml` (metadata), `config/` tree, and `bin/` executables. Compression is fixed (`zstd -19 --long=31`) for deterministic bytes.
- **Container images:** `iroha{2,3}-<version>-<os>-image.tar` generated from the Dockerfile using the same deploy binaries. Naming does not embed the registry tag, ensuring reproducible tarball names.
- **Hashes:** Each artifact emits `<name>.sha256`; `scripts/run_release_pipeline.py` collates them into `SHA256SUMS` and `release_manifest.json`, which downstream signing/publication systems use verbatim.
- **Signatures:** When `--signing-key` is supplied, both bundle and image scripts create matching `.sig` and `.pub` files (PEM-encoded public key). The pipeline preserves these alongside the artifacts so publication jobs only need to upload.
- **Manifests:** `generate_release_manifest.py` records profile, format, path, and SHA256 for every bundle/AppImage encountered. Keep this JSON attached to the release checklist and status updates.
- **Deterministic directories:** All artifacts live under `artifacts/releases/<version>/artifacts/`; rerunning the pipeline after a clean build should overwrite the same filenames, enabling reproducibility checks (diff of tarball hashes, manifest comparison).

**Build prerequisites**
- Ensure `cargo xtask gen-version --write` has been run so version metadata matches the target tag.
- Provide `--signing-key`/`--signing-cert` when signing is required; stash public key in the publication bucket.
- Use the same toolchain/container across both builds to maintain deterministic binaries.

## Validation Matrix
| Category | Iroha 2 Checks | Iroha 3 Checks |
|----------|----------------|----------------|
| Core CI | `cargo test --workspace --locked`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`. | Same. |
| Integration | `cargo test -p integration_tests -- --ignored --nocapture`; `scripts/run_sumeragi_smoke.sh` single-lane. | `scripts/run_sumeragi_smoke.sh --profile nexus`; Nexus lane replay tests. |
| Torii | `cargo test -p iroha_torii --features "app_api,transparent_api"` | Same plus Nexus Connect smoke (`IROHA_NEXUS_PROFILE=1`). |
| Config | `scripts/select_release_profile.py --network self-hosted --emit-manifest artifacts/network_profiles.json` | `scripts/select_release_profile.py --network sora-nexus --emit-manifest artifacts/network_profiles_nexus.json`. |
| SDK Parity | Rust/Python/JS CLI smoke against bundle (`scripts/sdk_release_smoke.sh --profile iroha2`). | Same script with `--profile iroha3` and Nexus config. |
| Telemetry | Verify `/metrics` and `/status` endpoints using `ci/release_metrics_check.sh`. | Ensure Nexus-specific gauges (lane counts, Nexus DA) present. |

Record pass/fail in the release ticket. Any ❌ requires engineering sign-off before release proceeds.

## Configuration & Manifest Checks
- Run `scripts/select_release_profile.py --list` to confirm network profile mappings.
- Verify `release/network_profiles.toml` includes the new version numbers, lane counts, and default artifact names.
- Attach the generated `artifacts/release_manifest.json` and `artifacts/network_profiles.json` to the release PR checklist.
- Ensure `defaults/` templates in the bundles match their target (single vs. nexus). If not, regenerate with `cargo xtask gen-config --profile <...>`.

## Approvals & Sign-off
| Gate | Required Approvers | Evidence |
|------|-------------------|----------|
| Freeze Confirmation | Release Manager + Core Lead | Meeting notes in tracker ticket. |
| Validation Matrix | Release Manager + relevant domain owners | Checklist in release PR/checklist. |
| Security/Signing | Security Review + Core Lead | Signed hash manifests, key custody log. |
| Publication | Release Manager + Ops | Bucket upload log, GitHub release draft. |

Document approvals in the release ticket or the `release/YYYY-MM/notes.md` record.

## Publication Flow
1. Generate tarballs/images (Build Matrix).
2. Upload artefacts to the staging buckets (`s3://releases-staging/iroha2/`, `s3://releases-staging/iroha3/`) and container registries (`registry.sora.org/iroha2`, `registry.sora.org/iroha3`).
3. Run `ci/verify_release_assets.sh --path artifacts/<profile>` to sanity-check hashes and signatures.
4. Promote from staging to the production buckets (`s3://releases/iroha2/`, `s3://releases/iroha3/`) once approval is logged.
5. Create GitHub releases `iroha2-vX.Y.Z` / `iroha3-vX.Y.Z`, attaching:
   - Tarballs, manifests, signatures.
   - SBOM if available (`cargo auditable` output).
   - Release notes sections linking to status/roadmap updates.
6. Update `docs/source/release_artifact_selection.md` if artifact layout changes.

## SoraFS/SoraNet endpoints, layout, and validation

| Track | Staging root (example) | Production root (example) | Object layout |
|-------|------------------------|---------------------------|---------------|
| Iroha 2 | `sorafs://staging/iroha2/v<ver>` | `sorafs://prod/iroha2/v<ver>` | Payloads at `<root>/iroha2-<ver>-<os>.tar.zst`, `<root>/iroha2-<ver>-<arch>.AppImage`, `<root>/SHA256SUMS`, `<root>/release_manifest.json`, and `<root>/dual_profile_matrix.json`. |
| Iroha 3 (Nexus) | `sorafs://staging/iroha3/v<ver>` | `sorafs://prod/iroha3/v<ver>` | Same layout as Iroha 2 with `iroha3-*` artefacts. |

Generate and validate publish plans before any upload/promotion:

1. Create the plan and shell wrapper from the release output (supply the SoraFS/SoraNet roots you intend to publish to):

   ```bash
   scripts/publish_plan.py generate \
     --manifest artifacts/releases/<ver>/release_manifest.json \
     --artifacts-dir artifacts/releases/<ver>/artifacts \
     --target iroha2=sorafs://staging/iroha2/v<ver> \
     --target iroha3=sorafs://staging/iroha3/v<ver> \
     --output-dir artifacts/releases/<ver>
   ```

2. Validate the plan locally (size/sha parity) and optionally probe staged HTTP(S) gateways after upload. Attach both `publish_plan.json` and `publish_plan_report.json` to the release ticket:

   ```bash
   scripts/publish_plan.py validate \
     --plan artifacts/releases/<ver>/publish_plan.json \
     --previous-plan artifacts/releases/<prev>/publish_plan.json \
     --probe-remote \
     --probe-command "sorafs_cli head --json {destination}" \
     --output artifacts/releases/<ver>/publish_plan_report.json
   ```

3. Any change to the SoraFS/SoraNet roots or object layout requires an explicit approval note in the release ticket. Include the diff emitted by `--previous-plan` when proposing a change; the generator/validator fail fast when required paths are missing or hashes deviate from the manifest.

## Post-Release Actions
- Merge release branches back into `main`, resolve conflicts promptly.
- Update `status.md` with validation highlights and artefact pointers.
- Close roadmap items (Milestone R3 tasks) and move completed work to `status.md`.
- Schedule retrospective; capture lessons learned and backlog action items for the next cycle.
- Archive artefacts (logs, manifests) under `artifacts/releases/vX.Y.Z/`.

## Outstanding actions
- **Validation matrix automation** — Owner: Release Engineering. Target: 2026-04-30.
  - Status: `ci/dual_profile_matrix.sh` now emits the required JSON locally; pipeline wiring still needs a Buildkite secret for the artefact upload.
  - Next step: open PR wiring `ci/dual_profile_matrix.sh` into `release-artifacts.yml` and add artefact upload alongside `checklist.json`.
- **Signing key rotation policy alignment** — Owner: Security Review board liaison (`@sec-review`). Target: before GA code freeze.
  - Status: waiting on `SEC-541` scoping meeting (scheduled 2026-03-08).
  - Next step: draft policy doc covering HSM ceremony, emergency rotation, and release checklist updates; circulate to Release Eng for sign-off.
