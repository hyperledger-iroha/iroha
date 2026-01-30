---
lang: pt
direction: ltr
source: docs/source/sdk/android/maven_staging_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 643f43f112b33ad5e40a57faf00742e9967462bbe7cffc50416163218c2008c0
source_last_modified: "2026-01-04T11:42:43.530057+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Android Maven Staging Plan (AND5)
summary: Staging environment workflow, validation steps, and evidence capture for Android SDK Maven publishes.
---

# Android Maven Staging Plan

Roadmap milestone **AND5 — Developer Experience & Samples** requires a repeatable
Maven staging environment so Release Engineering can dry-run the publishing flow
before the AND8 pilot window. This document records the staging topology,
pre-flight checks, validation automation, and evidence expectations so the plan
can be cited in governance reviews and `status.md`.

## 1. Environment Overview

| Component | Description | Notes |
|-----------|-------------|-------|
| Local staging repo | `artifacts/android/maven/<version>` populated by `scripts/publish_android_sdk.sh`. | Contains publish artifacts, CycloneDX SBOMs, provenance bundles, `publish_summary.json`, and `checksums.txt`. |
| Remote staging repo | Maven2 repository exposed at `https://maven.stg.sora.dev/releases`. | Only accessible to Release Engineering via the Sigstore-enforced credential pair `ANDROID_MAVEN_STAGING_USERNAME` / `ANDROID_MAVEN_STAGING_TOKEN`. |
| Evidence bucket | `artifacts/releases/android/<version>/` | Stores the staged Maven repo tarball, SBOM bundle, provenance logs, and validation reports for governance packets. |

**Secrets & auth**

- Stage credentials via `eval "$(scripts/release/load_env.sh android-maven-staging)"`.
  The helper loads `ANDROID_MAVEN_STAGING_USERNAME` / `ANDROID_MAVEN_STAGING_TOKEN`
  from `configs/release_env/android-maven-staging.env` (or `~/.config/iroha-release/…`),
  keeping sensitive values out of shell history.
- CI runners reference the same secrets through Buildkite hooks; local dry runs
  rely on the `ANDROID_MAVEN_STAGING_*` environment variables exported in your shell.

## 2. Pre-flight Checklist

1. **Quality gates**
   - Ensure `scripts/publish_android_sdk.sh --version <X.Y.Z>` succeeds without `--skip-sbom`.
   - Confirm `scripts/check_android_samples.sh` and `ci/run_android_tests.sh`
     run inside `publish_android_sdk.sh` (review the log if invoked manually).
2. **Artifact audit**
   - Verify `artifacts/android/maven/<X.Y.Z>/publish_summary.json` and
     `checksums.txt` exist. These files are required for validation.
   - Archive Gradle logs and SBOM/provenance bundles under
     `artifacts/releases/android/<X.Y.Z>/preflight/`.

## 3. Staging Publish Flow

1. **Publish locally** (already handled by `scripts/publish_android_sdk.sh`):

   ```bash
   scripts/publish_android_sdk.sh \
     --version 0.9.0-beta1 \
     --repo-dir artifacts/android/maven/0.9.0-beta1
   ```

2. **Promote to staging repo**:

   ```bash
   scripts/publish_android_sdk.sh \
     --version 0.9.0-beta1 \
     --repo-dir artifacts/android/maven/0.9.0-beta1 \
     --repo-url https://maven.stg.sora.dev/releases \
     --username "${ANDROID_MAVEN_STAGING_USERNAME}" \
     --password "${ANDROID_MAVEN_STAGING_TOKEN}"
   ```

   The command runs the same Gradle publish task, re-targeting the remote repo.
   Preserve the console log (`artifacts/releases/android/<version>/staging_publish.log`)
   for governance evidence.

3. **Package evidence**
   - Tar both the local repository tree and SBOM/provenance bundle into
     `artifacts/releases/android/<version>/staging_repo.tar.gz`.
   - Snapshot the Gradle build scan URL and attach it to the evidence README.

## 4. Validation & Reporting

Use `scripts/check_android_maven_repo.py` to validate the generated repository
before announcing the staging availability. The checker re-hashes every
artifact, cross-references `checksums.txt`, and optionally enforces that
`maven-metadata.xml` lists the recorded version.

```bash
scripts/check_android_maven_repo.py \
  --version 0.9.0-beta1 \
  --require-metadata \
  --json-out artifacts/releases/android/0.9.0-beta1/maven_validation.json \
  --markdown-out artifacts/releases/android/0.9.0-beta1/maven_validation.md
```

- When run without `--repo-dir`, the script expects the standard
  `artifacts/android/maven/<version>` layout.
- The JSON summary includes the artifact count and any validation errors; attach
  it to the staging evidence bundle and reference it in `status.md`. The
  Markdown export is a ready-to-attach evidence note for governance packets and
  should live alongside the JSON report.
- Failing validations block promotion—rerun `publish_android_sdk.sh` after
  fixing the discrepancy and preserve both the failing and passing reports for
  auditors.

## 5. Rollback & Retention

1. Keep the local staging repo under version control by archiving it in
   `artifacts/releases/android/<version>`. Governance reviewers use this when
   comparing the staging drop to the GA cut.
2. If an issue is discovered after pushing to `maven.stg.sora.dev`, run the
   `deploy/android/publish_ga.sh --rollback-only --version <X.Y.Z>` helper to
   remove the remote directory and log the action in `android_release_incidents.md`.
3. Retain staging evidence for at least 180 days so AND8 pilots and compliance
   audits can trace every staging release.

## 6. Status Reporting

- Record the staging publish in `status.md` under AND5 with links to the
  evidence directory and validation report.
- Update `docs/source/sdk/android/developer_experience_plan.md` once the flow is
  rehearsed twice without issues, noting the date of the last successful run.
- During pilots, append the staging validation summary to the partner-facing
  release notes so they can cross-check provenance hashes before consuming the
  Maven artifacts.
