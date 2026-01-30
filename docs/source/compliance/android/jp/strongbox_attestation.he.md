---
lang: he
direction: rtl
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox Attestation Evidence — Japan Deployments

| Field | Value |
|-------|-------|
| Assessment Window | 2026-02-10 – 2026-02-12 |
| Artefact Location | `artifacts/android/attestation/<device-tag>/<date>/` (bundle format per `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| Capture Tooling | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Reviewers | Hardware Lab Lead, Compliance & Legal (JP) |

## 1. Capture Procedure

1. On each device listed in the StrongBox matrix, generate a challenge and capture the attestation bundle:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Commit bundle metadata (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) to the evidence tree.
3. Run the CI helper to re-verify all bundles offline:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Device Summary (2026-02-12)

| Device Tag | Model / StrongBox | Bundle Path | Result | Notes |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Passed (hardware-backed) | Challenge bound, OS patch 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Passed | Primary CI lane candidate; temperature within spec. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Passed (retest) | USB-C hub replaced; Buildkite `android-strongbox-attestation#221` captured the passing bundle. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Passed | Knox attestation profile imported 2026-02-09. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Passed | Knox attestation profile imported; CI lane now green. |

Device tags map to `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. Reviewer Checklist

- [x] Verify `result.json` shows `strongbox_attestation: true` and certificates chain to trusted root.
- [x] Confirm challenge bytes match Buildkite runs `android-strongbox-attestation#219` (initial sweep) and `#221` (Pixel 8 Pro retest + S24 capture).
- [x] Re-run Pixel 8 Pro capture after hardware fix (owner: Hardware Lab Lead, completed 2026-02-13).
- [x] Complete Galaxy S24 capture once Knox profile approval arrives (owner: Device Lab Ops, completed 2026-02-13).

## 4. Distribution

- Attach this summary plus the latest report text file to partner compliance packets (FISC checklist §Data residency).
- Reference bundle paths when responding to regulator audits; do not transmit raw certificates outside encrypted channels.

## 5. Change Log

| Date | Change | Author |
|------|--------|--------|
| 2026-02-12 | Initial JP bundle capture + report. | Device Lab Ops |
