---
lang: uz
direction: ltr
source: docs/source/sdk/android/strongbox_attestation_harness_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 48476af62858721bd743152b8e3e9daf63268cc7d0011faeab6e8227f8ed20a9
source_last_modified: "2025-12-29T18:16:36.052112+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox Attestation Harness Plan (AND2)

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
| **On-device capture API** | Provision aliases, request attestation chains, and surface challenges through `IrohaKeyManager` + `KeyProvider` helpers. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/IrohaKeyManager.java`; `java/iroha_android/src/main/java/org/hyperledger/iroha/android/crypto/keystore/KeystoreKeyProvider.java`; `docs/source/sdk/android/key_management.md` |
| **Verification library** | Parse the Android Keymaster extension, enforce challenge binding, and classify security levels. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/crypto/keystore/attestation/AttestationVerifier.java` |
| **CLI harness + wrapper script** | Build the verifier, load bundles, inject trust roots, and emit JSON summaries for archives. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/tools/AndroidKeystoreAttestationHarness.java`; `scripts/android_keystore_attestation.sh` |
| **Lab/CI runner** | Walk archived bundles, enforce StrongBox-only policy, create reports, and annotate Buildkite runs. | `scripts/android_strongbox_attestation_ci.sh`; `scripts/android_strongbox_attestation_report.py`; `.buildkite/android-strongbox-attestation.yml` |
| **Readiness artefacts** | Track device matrix, bundle format, and compliance evidence so audits re-run reproductions. | `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`; `docs/source/compliance/android/` |

## 3. Capture API & Bundle Flow

1. **Alias provisioning**
   - `IrohaKeyManager.generateOrLoad(alias, KeySecurityPreference.STRONGBOX_REQUIRED)` creates or loads a StrongBox key.
   - Providers persist alias ownership so downgrades are rejected unless the caller explicitly relaxes the preference. Alias naming and telemetry labels follow the guidance in `docs/source/sdk/android/key_management.md`.
2. **Challenge orchestration**
   - The capture helper app (lab build) derives a 32-byte random challenge, encodes it as uppercase hex, and writes `challenge.hex`. The same value is passed to `IrohaKeyManager.generateAttestation(...)` so the verifier can detect tampering.
3. **Attestation export**
   - `AndroidKeystoreAttestationHarness` ingests either `chain.pem` or individual DER files plus trust roots and runs `AttestationVerifier`.
   - Successful runs print a one-line summary and, when `--output` is supplied, persist `result.json` containing alias, attestation/keymaster level, StrongBox boolean, and chain length.
4. **Bundle layout**
- Device lab operators follow the canonical structure documented in `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` (`chain.pem`, `challenge.hex`, `alias.txt`, `trust_root_*.pem`, optional `notes.md`).
- When OEMs deliver root packs as archives, store them next to the bundle using the
  `trust_root_bundle_<vendor>.zip` naming convention; the harness extracts the PEM/DER files directly
  so labs do not need to keep parallel directories in secure storage.
   - Bundles live under `artifacts/android/attestation/<fleet-tag>/<YYYY-MM-DD>/` so `scripts/android_strongbox_attestation_ci.sh` can auto-discover them.
5. **Verification loop**
   - `scripts/android_keystore_attestation.sh` compiles the verifier with JDK 21+, injects trust roots, enforces `--require-strongbox`, and emits JSON summaries used by CI, the readiness archive, and compliance evidence logs.
- **Lab rehearsal bundles:** Use `scripts/android_generate_mock_attestation_bundles.sh` (backed by `scripts/android_mock_attestation_der.py`) to mint deterministic mock bundles for every fleet tag when physical hardware is unavailable. The script emits a shared mock root (`trust_root_mock.pem`), Norito-aligned challenges, and notes so CI can exercise the harness end-to-end before real captures arrive.

## 4. Alias Lifecycle Hooks

- **Creation:** StrongBox-preferred aliases must be generated through `IrohaKeyManager.withDefaultProviders()` so telemetry and attestation helpers see consistent metadata. When the alias is missing, the StrongBox provider receives the request first and is allowed to fallback only when `KeySecurityPreference` permits downgrades.
- **Rotation:** The harness records `alias` inside every bundle; operators delete/recreate aliases via `KeystoreKeyProvider.deleteKey(alias)` before collecting new chains. Rotation events are logged in `docs/source/android_runbook.md` Section 5 and the readiness archive (`docs/source/sdk/android/readiness/archive/`).
- **Attestation refresh:** `IrohaKeyManager.verifyAttestation(...)` feeds directly into the harness, so apps can surface “last verified” timestamps. The plan requires exposing this metadata in upcoming sample apps (AND5 dependency).
- **Telemetry linkage:** Alias labels map to `telemetry.redaction.alias_labels` so overrides recorded in `docs/source/sdk/android/telemetry_override_log.md` always point to an attested key.

## 5. CI & Device Requirements

- **Device pool re-use:** The Pixel/Samsung fleet tracked in `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` is sufficient; no new procurement is needed before finance approval. Each device must ship an attestation bundle every quarter or after firmware refreshes.
- **Buildkite lane:** `.buildkite/android-strongbox-attestation.yml` triggers two steps:
  1. `scripts/android_strongbox_attestation_ci.sh` finds bundles and runs the harness with `--require-strongbox`.
  2. `scripts/android_strongbox_attestation_report.py --report-path artifacts/android/attestation/report.txt` emits a summary that is attached to the Buildkite annotation and archived in `docs/source/compliance/android/evidence_log.csv`.
- **Gating policy:** The CI job fails if any bundle lacks trust roots, produces a non-STRONGBOX security level, or misses `result.json`. Device lab ops receive Slack notifications (per `docs/source/android_runbook.md` Section 7) when the pipeline fails.
- **Log retention:** Bundle directories store `result.json` plus `trust_root_*.pem` so auditors can re-run the harness offline. Reports reference Buildkite job IDs and are cross-linked from `docs/source/compliance/android/jp/strongbox_attestation.md`.
- **Verification log:** Each manual execution of the harness is recorded in `docs/source/sdk/android/readiness/android_strongbox_attestation_run_log.md` with date, executor, and outcome so AND2 status reviews have auditable evidence between CI runs.

## 6. Implementation Checklist

| Deliverable | Owner | Status | Notes |
|-------------|-------|--------|-------|
| Capture API + alias workflow documented | Android Crypto TL | ✅ | Covered by this plan and the updated key-management guide. |
| Device lab SOP | Hardware Lab Lead | ✅ | Bundle format + matrix updated; reuse existing fleet with health checks. |
| CI pipeline guardrails | Release Engineering | ✅ | Harness + report scripts wired to Buildkite; evidence logged in `docs/source/compliance/android/evidence_log.csv`. |
| Governance artefacts | Docs/Compliance | ✅ | Readiness archive + compliance logs list bundle paths and verification timestamps. |

Outstanding implementation tasks (outside the scope of this plan) are tracked under AND2 in `roadmap.md`: attestation cache rotation, telemetry surfacing in SDK samples, and partner-facing documentation during pilot onboarding.

## 7. Ownership & Next Steps

- Weekly Android crypto syncs review bundle freshness and CI health; escalations go through Hardware Lab Ops before finance is asked to approve additional procurement.
- The same harness is reused by Swift/JS wallets once AND4 networking parity exposes Norito RPC retries; future updates must continue to use Norito JSON for reports so governance tooling ingests them deterministically.
- Once the StrongBox alias cache lands in runtime (AND2 milestone), update this plan with links to the final `iroha_config.android.key_policy` knobs and sample-app flows.
