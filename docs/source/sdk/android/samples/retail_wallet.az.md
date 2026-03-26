---
lang: az
direction: ltr
source: docs/source/sdk/android/samples/retail_wallet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 78bc3b022f78e95f310fa3d6b1bb4170f64008b94380e86734ffd61ee4d0b518
source_last_modified: "2026-01-28T17:11:30.749362+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Retail Wallet Sample — Design Notes

**Roadmap Link:** AND5 developer experience & docs  
**Owners:** Android DX TL (sample implementation), Wallet PM (requirements), Docs/DevRel (content), QA Guild (Managed Device coverage)  
**Status:** Completed 2026-05-02 — sample wired to SDK + POS assets

## 1. Purpose & Personas

This sample demonstrates retail-friendly flows that rely on the Android SDK’s offline signing, recovery, and Norito RPC retry support. It targets wallet vendors evaluating Nexus lanes plus auditors who need deterministic evidence of how offline envelopes are composed, transferred, and reconciled once connectivity returns.

Key scenarios:

- Prepare and sign transfer/mint/burn instructions offline using StrongBox or the configured secure element.
- Stage envelopes for device-to-device handoff (QR/NFC) with replay protection and Gosper-coded chunking for large payloads.
- Sync queued envelopes with the Torii `/v1/pipeline` endpoint once connectivity resumes, detecting conflicts and displaying receipts.
- Run recovery/rotation workflows (seed + device binding) with deterministic logging for compliance checks.

## 2. Feature Breakdown

| Area | Requirements | Notes |
|------|--------------|-------|
| Offline composer | `WalletPreviewViewModel` builds a sample `TransactionPayload`, signs it with attestation enabled, and surfaces the envelope hash/status for demos | Uses `TransactionBuilder.encodeAndSignEnvelopeWithAttestation` + `SignedTransactionHasher` to keep preview hashes deterministic |
| Secure storage | Prefer StrongBox when present and surface attestation availability in the UI; retain preview hash even when falling back to software keys | `IrohaKeyManager` drives key generation; preview shows `attestationAvailable` and alias used |
| Envelope handoff | Display the preferred i105 QR + copy buttons (canonical Katakana i105 only) and expose the mock hand-off inbox/endpoint so OA12 drills can cite the exported values | `scripts/android_sample_env.sh --handoff` seeds the inbox/endpoint; audit events flow through `PosAuditLogger` |
| Connectivity reconciliation | Load revocations/policy from bundled assets, hydrate the verdict journal file, and refresh snapshots via `OfflineToriiClient` when the mock endpoint is available | Revocation UI mirrors `offline_revocations.json`; verdict journal stored under app files and logged for audits |
| Recovery & rotation | Surface policy overrides (grace profiles/overrides) and pinned-root status alongside the POS manifest rotation window | `PolicyOverrideStore` + `SecurityPolicyLoader` thread overrides into the view model; `ManifestStatus` reports operator, window, and dual-key state |
| POS security policy | `security_policy.json` + Gradle knobs guard pinned roots, verdict TTL/nonce, and grace overrides | Sample enforces certificate/nonce checkpoints, optional `grace_period_profiles`, and logs rotations via `PosAuditLogger` |

## 3. Project Layout

- Source lives under `examples/android/retail-wallet` with bundled assets
  (`pos_manifest.json`, `security_policy.json`, `offline_revocations.json`,
  `offline_verdict_journal.json`, `pinned_root.pem`).
- `WalletPreviewViewModel` drives the preview hash/attestation, revocation feed,
  policy overrides (`PolicyOverrideStore`), POS manifest status (`PosManifestLoader`),
  and audit logging (`PosAuditLogger`).
- BuildConfig derives the Torii endpoint + verdict-grace overrides from Gradle
  properties (`retailWalletToriiEndpoint`, `retailWalletVerdictGracePeriodMs`,
  `retailWalletVerdictGraceProfile`) so CI and `android_sample_env.sh --wallet`
  runs produce deterministic outputs.
- `generateSampleManifest` captures the SDK version, Torii endpoint, feature flags,
  policy overrides, and SHA-256 fingerprints for the bundled assets so release/
  readiness bundles can cite the exact POS artefacts shipped with each APK.

## 4. Build, Test, Release

| Stage | Command | Output |
|-------|---------|--------|
| Lint/tests | `./gradlew :examples:retail-wallet:lintRelease :examples:retail-wallet:testRelease` | Kotlin + instrumentation tests covering composer, queue reconciliation, and handoff codec |
| Managed Device matrix | `./gradlew :examples:retail-wallet:managedDeviceMatrix` | Pixel 8/7/6 + Android 15/14/13 + Android Automotive emulator for kiosk mode smoke |
| Offline sync replay | `scripts/check_retail_wallet_offline_sync.sh` | Spins up mock Torii harness, replays sample envelopes, and asserts deterministic reconciliations |
| Bundle/SBOM | `./gradlew :examples:retail-wallet:bundleRelease :examples:retail-wallet:cyclonedxBom` | Artefacts stored under `artifacts/android/samples/retail-wallet/` with `sample_manifest.json` for provenance |

Release automation (`scripts/publish_android_sdk.sh --sample retail-wallet`) uploads bundles, SBOM, and attestations. CI must verify that `sample_manifest.json` aligns with the committed fixture hashes.

## 5. Telemetry & Observability

- Emits metrics: `android.retail_wallet.offline_queue_depth`, `android.retail_wallet.handoff_receive_total`, `android.retail_wallet.sync_conflict_total`.
- Logs include anonymized customer ids (`uaid` hash) and envelope digests only, satisfying `telemetry_redaction.md` constraints.
- Dashboard alignment: share panels with AND7 telemetry pack; publish JSON under `docs/source/sdk/android/readiness/dashboard_parity/`.

## 6. Documentation Requirements

Publish walkthrough docs under `docs/source/sdk/android/samples/retail_wallet.md` covering:

- environment bootstrap (`scripts/android_sample_env.sh --wallet`);
- offline composer usage, secure storage policy, and draft inspection checklist;
- QR/NFC handoff tutorial with troubleshooting for checksum mismatches;
- connectivity recovery steps and how to interpret pipeline receipts;
- recovery/rotation workflows referencing `docs/source/sdk/android/offline_signing.md`.

Localization: Japanese translation at beta, Hebrew at GA. Track translation tasks in `docs/source/sdk/android/i18n_plan.md`.

## 7. POS Security & Audit Controls

The OA11.1b controls live entirely inside the sample:

- **`security_policy.json`:** declares pinned backend roots, the enforced certificate id/nonce, a base `verdict_grace_period_ms`, and optional `grace_period_profiles` for common overrides. Operators can switch to a named profile via `./gradlew :retail-wallet:assembleDebug -PretailWalletVerdictGraceProfile=demo-short` or supply an explicit override window with `-PretailWalletVerdictGracePeriodMs=<millis>`.
- **Pinned root verifier:** `PinnedCertificateVerifier` fingerprints `assets/pinned_root.pem`, matches it against _all_ configured aliases, and logs `PIN_VERIFY`/`PINSET_ROTATION` entries inside `filesDir/pos_security_audit.log`.
- **Verdict enforcement:** `OfflineWallet.ensureFreshVerdict(...)` enforces certificate + nonce freshness and threads the selected grace window into the warning pipeline. Every pass/fail emits a `VERDICT_STATUS` record tagged with the source of the grace window plus `VERDICT_ROTATION` entries whenever Torii issues a new certificate/nonce pair.
- **Runtime policy knobs:** The **Configure grace window** button feeds `PolicyOverrideStore` so operators can toggle the policy default, pick any published profile, or enter a custom override without rebuilding the sample. `WalletPreviewViewModel` reloads the security policy through `SecurityPolicyLoader` on every change, refreshes the pinned-root + verdict displays, and logs a new audit entry whenever overrides change, keeping OA11.1b’s policy controls auditable at runtime.【examples/android/retail-wallet/src/main/java/org/hyperledger/iroha/samples/wallet/MainActivity.kt:1】【examples/android/retail-wallet/src/main/java/org/hyperledger/iroha/samples/wallet/WalletPreviewViewModel.kt:1】
- **OA12 provisioning manifest:** `PosManifestLoader` reads the bundled `pos_manifest.json`, enforces dual-signature coverage, warns when rotation hints or validity windows approach, and records the manifest sequence/health inside `pos_security_audit.log`. The new manifest card sits beneath the policy controls so operators can screenshot OA12 drills alongside the existing OA11 evidence.【examples/android/retail-wallet/src/main/java/org/hyperledger/iroha/samples/wallet/PosManifest.kt:1】【examples/android/retail-wallet/src/main/java/org/hyperledger/iroha/samples/wallet/MainActivity.kt:1】

Ship the audit log alongside sample artefacts when rehearsing rotation drills so ops/regulators can confirm that grace overrides and pinset changes were authorised.

## 8. Outstanding Questions & Next Steps

1. Confirm whether kiosk mode / Android Automotive support is necessary for GA or deferred to LTS.
2. Decide on the default chunking strategy for envelope QR encoding (base45 vs. compressed JSON) to keep parity with Torii CLI.
3. Align offline queue persistence format with the Norito fixture exporter so CLI and Android outputs stay interoperable.
4. Schedule Managed Device time for the automotive emulator once the Gradle modules land.

Update this file whenever scope shifts or dependencies are resolved. Link tickets back here for traceability.
