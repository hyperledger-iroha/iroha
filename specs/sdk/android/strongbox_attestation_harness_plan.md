<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox Attestation Harness Plan (AND2)

This is a strict qualification harness for deployments that explicitly select
the optional StrongBox integration. It is not a general Android SDK build,
test, tag, or release prerequisite; software-backed signing remains valid
without a physical-device bundle.

Status: Authored 2026-02-14  
Owners: Android Crypto TL, Hardware Lab Lead, Device Lab Ops  
Related roadmap item: AND2 — Plan StrongBox attestation harness

## 1. Goals & Scope

- Describe the capture API and automation layers that verify Android StrongBox attestation bundles before finance signs off on lab spend.
- Define how aliases, challenges, and bundle layouts integrate with `IrohaKeyManager` so SDK callers get deterministic StrongBox evidence.
- Capture CI/device requirements so the existing Pixel/Samsung pool can satisfy AND2 without new hardware purchases.

## 2. Architecture Overview

| Component | Responsibility | Sources |
|-----------|----------------|---------|
| **On-device capture API** | Provision aliases, request attestation chains, and surface challenges through `IrohaKeyManager` + `KeyProvider` helpers. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/IrohaKeyManager.java`; `java/iroha_android/src/main/java/org/hyperledger/iroha/android/crypto/keystore/KeystoreKeyProvider.java`; `specs/sdk/android/key_management.md` |
| **Verification library** | Parse the Android Keymaster extension, enforce mandatory challenge binding, evaluate the chain at an explicit time, reject governed revoked serials/TBS hashes, and classify security levels. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/crypto/keystore/attestation/AttestationVerifier.java` |
| **CLI harness + wrapper script** | Build the verifier, load bundles, inject trust roots, and emit JSON summaries for archives. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/tools/AndroidKeystoreAttestationHarness.java`; `scripts/android_keystore_attestation.sh` |
| **Lab/CI runner** | Walk archived bundles, enforce StrongBox-only policy, create reports, and annotate Buildkite runs. | `scripts/android_strongbox_attestation_ci.sh`; `scripts/android_strongbox_attestation_report.py`; `.buildkite/android-strongbox-attestation.yml` |
| **Readiness artefacts** | Track device matrix, bundle format, and compliance evidence so audits re-run reproductions. | `specs/sdk/android/readiness/android_strongbox_device_matrix.md`; this plan; `specs/compliance/android/` |

## 3. Capture API & Bundle Flow

1. **Alias provisioning**
   - Generate an externally issued random challenge, set it in `KeyGenParameters`, and call `IrohaKeyManager.generateOrLoad(newUniqueAlias, KeySecurityPreference.STRONGBOX_REQUIRED)`. Android cannot attach a new challenge to an existing alias.
   - Providers persist alias ownership so downgrades are rejected unless the caller explicitly relaxes the preference. Alias naming and telemetry labels follow the guidance in `specs/sdk/android/key_management.md`.
2. **Challenge orchestration**
   - The lab authority records the issued challenge, alias, and generated key's leaf-SPKI SHA-256 in an authenticated expectations tree. Copies returned by the device are evidence only and never become verifier expectations.
3. **Attestation export**
   - `AndroidKeystoreAttestationHarness` ingests either `chain.pem` or individual DER files plus separately trusted roots, alias, non-empty challenge, expected leaf-SPKI digest, explicit evaluation time, and the current governed revocation snapshot before running `AttestationVerifier`.
   - Successful runs print a one-line summary and, when `--output` is supplied, persist `result.json` containing alias, attestation/keymaster level, StrongBox boolean, and chain length.
4. **Evidence and expectations layout**
   - Untrusted bundles live under `artifacts/android/attestation/<fleet-tag>/<YYYY-MM-DD>/` and contain the submitted chain plus output/notes.
   - A separately authenticated tree mirrors each relative path under `artifacts/android/attestation-expectations/` and contains `alias.txt`, `challenge.hex`, and `leaf_spki_sha256.hex`.
   - Vendor roots and root archives remain in a governed authority directory outside both trees. Bundle-local roots, identity files, and challenges are never authoritative.
5. **Verification loop**
   - `scripts/android_keystore_attestation.sh` compiles the verifier with JDK 21+, injects trust roots, requires the governed snapshot digest/date/max-age and evaluation time, enforces `--require-strongbox`, and emits JSON summaries used by CI, the readiness archive, and compliance evidence logs.
- **Lab rehearsal bundles:** Use `scripts/android_generate_mock_attestation_bundles.sh` (backed by `scripts/android_mock_attestation_der.py`) to mint mock bundles and a separate expectations tree. Supply its mock root from `artifacts/android/attestation-authority/`; never copy it into a submitted bundle.

## 4. Alias Lifecycle Hooks

- **Creation:** StrongBox-preferred aliases must be generated through `IrohaKeyManager.withDefaultProviders()` so telemetry and attestation helpers see consistent metadata. When the alias is missing, the StrongBox provider receives the request first and is allowed to fallback only when `KeySecurityPreference` permits downgrades.
- **Rotation:** Provision a new unique alias with the new challenge, verify its recorded chain, then switch application ownership deliberately. Never silently overwrite the active signing alias. Rotation events are logged in `specs/android_runbook.md` Section 5 and the readiness archive (`specs/sdk/android/readiness/archive/`).
- **Attestation verification:** `IrohaKeyManager.verifyAttestation(...)` rereads provisioning-time evidence and feeds directly into the harness, so apps can surface “last verified” timestamps without claiming a new certificate was minted.
- **Telemetry linkage:** Alias labels map to `telemetry.redaction.alias_labels` so overrides recorded in `specs/sdk/android/telemetry_override_log.md` always point to an attested key.

## 5. CI & Device Requirements

- **Device pool re-use:** The Pixel/Samsung fleet tracked in `specs/sdk/android/readiness/android_strongbox_device_matrix.md` is sufficient; no new procurement is needed before finance approval. Each device must ship an attestation bundle every quarter or after firmware refreshes.
- **Buildkite lane:** `.buildkite/android-strongbox-attestation.yml` triggers two steps:
  1. `scripts/android_strongbox_attestation_ci.sh` finds bundles and runs the harness with a separately trusted expectations tree, roots, and governed snapshot.
  2. `scripts/android_strongbox_attestation_report.py --report-path artifacts/android/attestation/report.txt` emits a summary that is attached to the Buildkite annotation and archived in `specs/compliance/android/evidence_log.csv`.
- **Gating policy:** Once the optional hardware-qualification job is explicitly
  selected, it fails for zero bundles, missing evidence, missing trusted
  expectations, bundle/authority path overlap, stale governed status, revoked
  certificates or anchors, a leaf-SPKI mismatch, non-StrongBox evidence, or a
  missing `result.json`. This result does not gate the software-backed SDK
  release path.
- **Log retention:** Store `result.json` with evidence, but retain expectations and trust anchors in the separately governed inventory. Reports reference Buildkite job IDs and are cross-linked from `specs/compliance/android/jp/strongbox_attestation.md`.
- **Verification log:** Each manual execution of the harness is recorded in `specs/sdk/android/readiness/android_strongbox_attestation_run_log.md` with date, executor, and outcome so AND2 status reviews have auditable evidence between CI runs.

## 6. Implementation Checklist

| Deliverable | Owner | Status | Notes |
|-------------|-------|--------|-------|
| Capture API + alias workflow documented | Android Crypto TL | ✅ | Covered by this plan and the updated key-management guide. |
| Device lab SOP | Hardware Lab Lead | ✅ | Bundle format + matrix updated; reuse existing fleet with health checks. |
| CI pipeline guardrails | Release Engineering | ✅ | Harness + report scripts wired to Buildkite; evidence logged in `specs/compliance/android/evidence_log.csv`. |
| Governance artefacts | Docs/Compliance | ✅ | Readiness archive + compliance logs list bundle paths and verification timestamps. |

Outstanding implementation tasks (outside the scope of this plan) are tracked under AND2 in `roadmap.md`: attestation cache rotation, telemetry surfacing in SDK samples, and partner-facing documentation during pilot onboarding.

## 7. Ownership & Next Steps

- Weekly Android crypto syncs review bundle freshness and CI health; escalations go through Hardware Lab Ops before finance is asked to approve additional procurement.
- The same harness is reused by Swift/JS wallets once AND4 networking parity exposes Norito RPC retries; future updates must continue to use Norito JSON for reports so governance tooling ingests them deterministically.
- Once the StrongBox alias cache lands in runtime (AND2 milestone), update this plan with links to the final `iroha_config.android.key_policy` knobs and sample-app flows.
