---
title: SoraFS Release Rollback and Yank
summary: Preserve release evidence, restore the last verified candidate, and withdraw every affected package channel.
---

# SoraFS Release Rollback and Yank

Use this runbook when a SoraFS CLI/SDK candidate must be stopped before
promotion or withdrawn after publication. Rollback changes deployment pointers;
yank or delist changes package discovery. Neither operation permits deleting or
rewriting the signed candidate, tag, checksums, SBOM, provenance, or audit log.

## 1. Open and bound the incident

1. Record the affected `sorafs-cli-v<version>` tag, commit, target triples,
   package IDs from `release/version-map.toml`, discovery time, reason, incident
   ID, and release-operator/governance approvers.
2. Freeze new promotion, replication, and “latest” pointer changes. Keep the
   current signed artifacts read-only so responders and auditors can reproduce
   the defect.
3. Decide whether the scope is deployment rollback, package-channel withdrawal,
   signing-key revocation, or all three. A compromised signer always requires
   the signer-rotation/revocation procedure in addition to this runbook.

## 2. Select and verify the rollback candidate

Select the most recent previously approved release that is unaffected by the
incident. Before changing any deployment pointer:

1. Download its target-specific archive, `SHA256SUMS`, Sigstore bundle, offline
   provenance bundle, SBOM, and vulnerability report from the immutable release
   record.
2. Verify the exact checksum inventory, Sigstore certificate identity and OIDC
   issuer, provenance subject, release tag/commit binding, and reviewed signer
   fingerprint. Do not fall back to an unsigned local build.
3. Extract the archive into a new empty directory and run
   `sorafs_cli --help`, `sorafs_fetch --help`, and `sorafs-validate --help` from
   that directory. Retain the clean-consumer smoke log.
4. Confirm that its configuration/schema version is accepted by the reference
   deployment. V1 state is reseeded when incompatible; legacy codec or state
   migration paths must not be re-enabled.

## 3. Roll back deployments

1. Drain new work at the affected gateways/providers without discarding durable
   outboxes, cursors, or dead-letter records.
2. Atomically repoint the deployment manifest to the verified prior release.
   Roll the two independently administered gateways separately and retain at
   least one healthy region throughout the change.
3. Reconcile committed ledger state before re-enabling workers. Re-run health,
   readiness, proof, range-fetch, and gateway conformance checks.
4. If rollback checks fail, stop. Keep traffic on the last-known-good region and
   escalate; never skip checksum, provenance, or signature verification.

Taira or Minamoto mutation remains a separately authorized operation. This
runbook does not grant that authority.

## 4. Withdraw package channels

Use the package IDs and exact versions in `release/version-map.toml`. Capture
the authenticated command/API response for every channel; do not claim a yank
until the registry confirms it.

| Ecosystem | Required withdrawal action |
| --- | --- |
| Cargo/crates.io | Run `cargo yank --vers <version> <crate>` for each published SoraFS crate. Do not delete the crate or reuse the version. |
| npm | Run `npm deprecate <package>@<version> "<incident/advisory>"`. Use unpublish only when registry policy explicitly permits it and governance approves the evidence loss. |
| Python/PyPI | Yank the exact release through the authenticated PyPI project release control/API and publish the incident reason. Do not upload replacement files under the same version. |
| C#/NuGet | Unlist the exact package version; do not delete or overwrite its `.nupkg`. Verify normal version search no longer selects it. |
| JVM/Android | Mark the exact Maven/Gradle publication withdrawn in the owning repository. Maven Central releases are immutable, so publish an advisory and a fixed higher version instead of attempting replacement. |
| Swift Package Manager | Preserve the signed Git tag, publish an advisory that rejects the affected version, and cut a fixed higher tag. Do not move or recreate the original tag. |
| GitHub CLI artifacts | Remove the affected release from “latest” discovery and label it withdrawn while retaining artifacts, attestations, signatures, and checksums for verification. Point installation documentation to the verified prior or fixed release. |

If a listed ecosystem was not published for the affected release, record
`not_published` plus the registry query used to establish that fact. Absence of
a package-channel workflow is not evidence that a package was never published.
Never reuse a withdrawn version.

## 5. Close and prove the rollback

Attach a payload-free rollback record to the release ticket with:

- incident ID, affected version/commit, reason, decision timestamp, and
  approvers;
- prior release version plus verified archive/checksum/provenance/signature
  digests;
- per-region deployment result and clean-consumer smoke hashes;
- one `withdrawn`, `not_published`, or `failed` result for every package row in
  `release/version-map.toml`, including registry receipt identifiers;
- signer revocation/rotation receipts when applicable; and
- the corrective release or follow-up owner.

Record only identifiers, timestamps, statuses, and digests. External-signer/KMS handles,
private signing material, bearer tokens, registry credentials, WebAuthn
material, and artifact payloads remain runtime-only and must not enter the
ticket or readiness summaries. Publish the Ed25519 verification key only in
the signed release artifact set; the payload-free ticket carries its reviewed
fingerprint.

Run the production-readiness aggregate gate again against the restored
deployment. A rollback is complete only when monitoring is healthy, ledger
reconciliation is clean, every applicable registry confirms withdrawal, and
the rollback remains independently reversible.
