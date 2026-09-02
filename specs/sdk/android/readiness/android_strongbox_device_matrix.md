# Kagemusha V1 Android hardware qualification matrix

This is the physical-device release gate for an Android Kagemusha V1
hardware profile. A profile remains disabled until every required device row has
fresh, signed evidence from the exact release candidate. Emulator, software-key,
summary-only, or host-simulated results never satisfy the gate.

## Required device families

| Device family | Minimum OS | Required secure provider |
| --- | --- | --- |
| Google Pixel 6 / 6a | Android 14 | StrongBox-backed KeyMint |
| Google Pixel 7 / 7 Pro | Android 14 | StrongBox-backed KeyMint |
| Google Pixel 8 / 8a / 8 Pro | Android 15 | StrongBox-backed KeyMint |
| Google Pixel Fold / Tablet | Android 15 | StrongBox-backed KeyMint |
| Samsung Galaxy S23 | Android 14 | StrongBox-backed KeyMint |
| Samsung Galaxy S24 | Android 15 | StrongBox-backed KeyMint |

One signed slot is required for every row. Slots must not reuse a device
fingerprint, attestation challenge, hardware epoch, or qualification run.

## Evidence contract

Each slot binds the exact source tree, release manifest, paired-Pasta artifacts,
wallet APK and signing certificate, device identity and OS build, attested
hardware policy, and complete raw command/output transcript. It must prove:

- exact-next or one-use-successor state transitions with no software fallback;
- rollback-resistant journal, accepted-credit inbox, and payment outbox;
- trusted commit time and atomic recoverable transition certificates;
- `Bootstrap`, `MintFold`, `SendSplit`, `ReceiveFold`, `RedeemSplit`, and
  offline hardware-epoch `Rotate`;
- 1,000 independently received one-unit credits folded into one aggregate
  balance, followed by one 1,000-unit payment and subsequent full and partial
  redemption;
- at least 1,024 real recursive handoffs with proof/envelope size independent
  of depth;
- shuffled concurrent requests, delayed post-expiry delivery, exact duplicate
  retry, conflicting credit reuse, stale state, forked successors, rollback,
  counter reuse/skip/rollover, forged rotation, overflow, and proof/output
  substitution rejection;
- crash injection at every journal, hardware commit, proof, state persistence,
  inbox, outbox, transport, and acknowledgement boundary, with value
  conservation and byte-identical recovery;
- airplane mode, restart, abrupt power loss, clock rollback, backup/restore,
  thermal, latency, memory, throughput, QR, NFC, and nearby-device operation;
- zero network requests during every device-to-device payment.

Secure state is intentionally non-clonable. Backup/restore qualification must
show that restored application data cannot fork spend authority; it is not a
promise that lost hardware cash can be recovered.

## Release decision

The validator must fail closed for missing rows, stale or substituted artifacts,
untrusted attestation roots, software keys, reused challenges, mutable evidence,
noncanonical values, or any incomplete scenario. Lab signing keys and device
secrets are runtime-only and must never appear in repository files, logs, or
summaries.

No Android hardware profile may be enabled from source-only or simulator
evidence. The signed qualification matrix is a release input, not a compatibility
mode or an optional feature claim.
