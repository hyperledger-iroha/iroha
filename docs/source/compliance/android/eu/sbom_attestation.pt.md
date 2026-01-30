---
lang: pt
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM & Provenance Attestation — Android SDK

| Field | Value |
|-------|-------|
| Scope | Android SDK (`java/iroha_android`) + sample apps (`examples/android/*`) |
| Workflow Owner | Release Engineering (Alexei Morozov) |
| Last Verified | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. Generation Workflow

Run the helper script (added for AND6 automation):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

The script performs the following:

1. Executes `ci/run_android_tests.sh` and `scripts/check_android_samples.sh`.
2. Invokes the Gradle wrapper under `examples/android/` to build CycloneDX SBOMs for
   `:android-sdk`, `:operator-console`, and `:retail-wallet` with the supplied
   `-PversionName`.
3. Copies each SBOM into `artifacts/android/sbom/<sdk-version>/` with canonical names
   (`iroha-android.cyclonedx.json`, etc.).

## 2. Provenance & Signing

The same script signs every SBOM with `cosign sign-blob --bundle <file>.sigstore --yes`
and emits `checksums.txt` (SHA-256) in the destination directory. Set the `COSIGN`
environment variable if the binary lives outside `$PATH`. After the script finishes,
record the bundle/checksum paths plus Buildkite run id in
`docs/source/compliance/android/evidence_log.csv`.

## 3. Verification

To verify a published SBOM:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

Compare the output SHA to the value listed in `checksums.txt`. Reviewers also diff the SBOM against the previous release to ensure dependency deltas are intentional.

## 4. Evidence Snapshot (2026-02-11)

| Component | SBOM | SHA-256 | Sigstore Bundle |
|-----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` bundle stored beside SBOM |
| Operator console sample | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Retail wallet sample | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Hashes captured from Buildkite run `android-sdk-release#4821`; reproduce via the verification command above.)*

## 5. Outstanding Work

- Automate SBOM + cosign steps inside the release pipeline before GA.
- Mirror SBOMs to the public artefact bucket once AND6 marks the checklist complete.
- Coordinate with Docs to link SBOM download locations from partner-facing release notes.
