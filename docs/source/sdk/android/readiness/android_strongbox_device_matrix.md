# Android StrongBox Offline Payments Device Matrix

Last updated: 2026-06-05

This matrix gates production readiness for Android offline-offline payment
flows. A device row is ready only after the lab attaches signed evidence for
StrongBox/KeyMint attestation, one-use key rotation, rollback rejection, ABI-6
recursive spend, and ABI-7 recursive compact-token availability probing.

| Device family | Minimum OS | StrongBox / KeyMint gate | Kagemusha recursive compact gate | Status |
| --- | --- | --- | --- | --- |
| Google Pixel 6 / 6a | Android 14 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Google Pixel 7 / 7 Pro | Android 14 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Google Pixel 8 / 8a / 8 Pro | Android 15 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Google Pixel Fold / Tablet | Android 15 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Samsung Galaxy S23 | Android 14 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |
| Samsung Galaxy S24 | Android 15 | Pending lab attestation export | `recursive_compact_v1` probe must remain unavailable until circuit-backed | Blocked |

Production release criteria:

- ABI 6 recursive spend JNI probes pass on every required device family.
- ABI 7 recursive compact-token JNI probes fail closed with the unavailable
  status until `kagemusha-recursive-compact-v1` is circuit-backed.
- Wallet rollback tests prove that old encrypted wallet state cannot be restored
  after one-use key rotation.
- StrongBox/KeyMint attestation chains bind the app challenge and device
  security level expected by the offline wallet policy.
- Lab reports include raw test commands, device fingerprints, OS build IDs, and
  signed evidence artifact hashes.
