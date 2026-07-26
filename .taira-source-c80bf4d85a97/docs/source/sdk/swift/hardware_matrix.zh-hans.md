---
lang: zh-hans
direction: ltr
source: docs/source/sdk/swift/hardware_matrix.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05a571bb335a56bd8a71992930d423f49d9442b77f1dbf4cdb554824b92d0415
source_last_modified: "2025-12-29T18:16:36.070267+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Swift Hardware Matrix (IOS4 Pre-read)

This matrix summarizes the Secure Enclave, simulator, and macOS Catalyst
coverage that the IOS4 hardware review will validate. Use it as a pre-read for
the 2026-05-15 session described in
`docs/source/sdk/swift/hardware_requirements_review.md`.

## Device Coverage

| Tier | Hardware / OS | Storage Backend | Primary Use | Notes |
|------|---------------|-----------------|-------------|-------|
| P0 | iPhone 15 Pro / iOS 18 | Secure Enclave | Production wallets, multisig hosts | Baseline device with A17 Pro; attestation sample captured (see below). |
| P0 | iPad Pro (M4) / iPadOS 18 | Secure Enclave | Treasury dashboards, operational tooling | Shares attestation cadence with iPhone P0 but exposes additional multitasking metrics. |
| P1 | Apple Silicon Mac (M3) / macOS 15 | Secure Enclave | Desktop wallet + CLI parity | Acts as the CI reference for Catalyst builds; record attestation evidence per release. |
| P2 | Simulator (x86_64) | In-memory deterministic seed | CI smoke, integration testing | Lacks Secure Enclave; relies on deterministic HKDF seeds documented in the IOS4 RFC. |
| P2 | CI-hosted macOS runners | Secure Enclave (shared) | Nightly multisig smoke harness | Device pool booking tracked alongside Android AND2 share. |

## Key Points

- Secure Enclave tiers must export attestation bundles using the helper that
  will ship with `SigningKey` once IOS4 lands. Pre-read sample:
  `docs/source/sdk/swift/readiness/archive/2026-05/secure_enclave_attestation_sample.json`.
- Simulator / CI fallbacks must only rely on deterministic seed derivation as
  defined in `docs/source/sdk/swift/ios4_crypto_api_rfc.md`.
- Device bookings and rotation cadence mirror
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` so the
  shared hardware lab can schedule AND2/IOS4 coverage without collisions.

## Action Items Before Review

1. Capture fresh attestation samples for each Secure Enclave tier and drop them
   into `docs/source/sdk/swift/readiness/archive/2026-05/`.
2. Confirm CI runner availability for nightly multisig smoke tests.
3. Prepare a short demo of the forthcoming `SigningKey` Secure Enclave wrapper
   (even if a stub) so the review can lock implementation checkpoints.
