---
lang: ur
direction: rtl
source: docs/source/release_dual_track_automation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b80dcaae8fcb8de805faa6100a5c2131070d2a0c6534ed568dcba83d0dbc1ea3
source_last_modified: "2026-01-04T10:50:53.642758+00:00"
translation_last_reviewed: 2026-01-30
---

# Dual-Track Release Automation Plan

This document outlines the build/release automation work needed to fully support the
Iroha 2 and Iroha 3 dual-track runbook.

## Scope
1. Changelog generation per track
2. Artifact signing & SBOM creation
3. Release artifact publication to S3 buckets
4. Checklist archiving & validation reporting

## Current tooling
- `scripts/release/generate_changelog.sh` – draft script (needs per-track sections)
- `scripts/build_release_bundle.sh` – builds `iroha2`/`iroha3` with `--profile`, emits manifests, hashes, and optional signatures when `--signing-key` is provided
- `scripts/build_release_image.sh` – saves profile-specific Docker images with matching metadata/signature support
- GitHub workflow: `release-artifacts.yml` (builds artifacts on demand)

## Proposed changes
### 1. Changelog automation
- [x] Extend `generate_changelog.sh` to accept `--flavor` flag and update the corresponding section in `CHANGELOG.md`.
- [x] Add CI job `ci/generate-changelog` to run the script and commit updates on branch cut PRs.

### 2. Signing pipeline
- [x] Pass `--signing-key` to `build_release_bundle.sh` from `release-artifacts.yml` so signatures & manifests are generated automatically alongside tarballs.
- [x] Store signing key in Buildkite/GitHub secrets (`RELEASE_SIGNING_KEY`) with rotation policy.

### 3. Publication
- [x] Add step to publish artifacts to `s3://releases/iroha2/` or `s3://releases/iroha3/` based on profile.
- [x] Upload the generated JSON manifest alongside the tarball to preserve hash/signature metadata.

### 4. Reporting
- [x] Extend `release-artifacts.yml` to generate `artifact/releases/{tag}/checklist.json` and attach to GitHub release.
- [ ] Update dashboards (future) to consume release metadata for audit trail.

## Milestones
| Milestone | Due | Owner | Status |
|-----------|-----|-------|--------|
| Changelog script accepts `--flavor` | Feb 15, 2026 | Release Eng | ✅ Complete (script now in `scripts/release/generate_changelog.py`; CI `ci/generate-changelog` job active) |
| Signing + SBOM automated in workflow | Feb 28, 2026 | Release Eng | ✅ Complete (`release-artifacts.yml` passes `--signing-key` and embeds SBOM via Syft) |
| S3 publication step live | Mar 07, 2026 | Release Eng | ✅ Complete (artifacts pushed to `s3://releases/{track}/` with manifests) |
| Checklist artifact published | Mar 15, 2026 | Release Eng | ✅ Complete (`artifact/releases/{tag}/checklist.json` uploaded alongside release) |

## Dependencies
- Access to Release signing key (Rev 2026Q1)
- AWS credentials with write access to releases buckets
- Governance approval for automated changelog updates on release branches
