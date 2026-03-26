---
lang: zh-hant
direction: ltr
source: docs/source/sdk/android/offline_signing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cf3f9e49cea77f1dd57366459a169830a5384404a14a2060f6d22715293dd800
source_last_modified: "2026-01-30T12:29:10.196090+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Offline Signing & Envelope Guide

Roadmap items **AND2** (secure key management) and **AND5** (developer
experience) require deterministic offline signing so operators can stage
transactions on air‑gapped devices, queue retries, or forward signed payloads
between the operator console and retail wallet samples. This guide explains the
envelope format, SDK helpers, attestation bundling, and replay tooling shipped
in `java/iroha_android`.

> **Sample walkthroughs:**  
> - [Retail wallet offline workflows](samples/retail_wallet.md#2-feature-breakdown) – the offline composer, envelope handoff, and connectivity reconciliation rows apply the envelope + queueing guidance from Sections 1–5.  
> - [Operator console walkthrough](samples/operator_console.md#9-step-by-step-walkthrough) – Steps 2–3 illustrate how governance envelopes, attestation bundles, and telemetry evidence map back to the procedures outlined here.

## 1. Envelope Format

`OfflineSigningEnvelope`
(`java/iroha_android/src/main/java/org/hyperledger/iroha/android/tx/offline/OfflineSigningEnvelope.java`)
is a Norito-serialisable container that mirrors the canonical transaction bytes
plus metadata needed for audits and recovery.

| Field | Description |
|-------|-------------|
| `encodedPayload` | Canonical Norito bytes returned by `NoritoJavaCodecAdapter`. |
| `signature` | Raw Ed25519 signature. |
| `publicKey` | 32-byte public key used for verification. |
| `schemaName` | Norito schema identifier (`iroha.android.transaction.Payload.v1`). |
| `keyAlias` | Alias recorded by `IrohaKeyManager` (mirrors telemetry/attestation logs). |
| `issuedAtMs` | UTC timestamp (ms since epoch) when the envelope was generated. |
| `metadata` | Arbitrary key/value pairs (audit IDs, batch labels, etc.). |
| `exportedKeyBundle` | Optional deterministic HKDF/AES-GCM export of the software key. |

`OfflineSigningEnvelopeCodec`
(`java/iroha_android/src/main/java/org/hyperledger/iroha/android/tx/offline/OfflineSigningEnvelopeCodec.java`)
encodes/decodes envelopes using the standard Norito codec so the same bytes can
flow through desktop CLI tools, Android apps, or supporting services.

## 2. Creating Envelopes

`TransactionBuilder` exposes helpers that wrap the normal signing flow:

```java
TransactionBuilder builder =
    new TransactionBuilder(new NoritoJavaCodecAdapter(),
        IrohaKeyManager.withDefaultProviders());

OfflineEnvelopeOptions options = OfflineEnvelopeOptions.builder()
    .putMetadata("batch-id", "governance-q4")
    .setIssuedAtMs(System.currentTimeMillis())
    .build();

OfflineSigningEnvelope envelope = builder.encodeAndSignEnvelope(
    payload,
    "governance-primary",
    KeySecurityPreference.STRONGBOX_PREFERRED,
    options);
```

- `encodeAndSignEnvelope(...)` signs the payload via `IrohaKeyManager` and
  returns an envelope populated with alias/timestamp metadata.
- `encodeAndSignEnvelopeWithAttestation(...)` returns an
  `OfflineTransactionBundle`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/tx/offline/OfflineTransactionBundle.java`)
  combining the envelope plus any StrongBox/TEE attestation produced for the
  alias.
- Use `OfflineEnvelopeOptions.exportDeterministicKey(...)` when the receiving
  system needs the HKDF-derived `KeyExportBundle` (for example, retail wallet
  recovery workflows). The export piggybacks on
  `SoftwareKeyProvider.exportDeterministic(...)` and inherits the AES-GCM
  tamper-detection checks described in `status.md`.

## 3. Attestation Pairing

When hardware-backed providers are available, pair every offline envelope with
attestation material before forwarding it to Torii or a custody service:

1. Call `encodeAndSignEnvelopeWithAttestation(...)` with an optional challenge.
2. Archive the returned `KeyAttestation` alongside the Norito envelope (PEM
   chain + JSON summary as described in
   `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`).
3. Verify the attestation via
   `IrohaKeyManager.verifyAttestation(...)` or
   `scripts/android_keystore_attestation.sh --bundle-dir <dir> --challenge <hex>`.
4. Forward both envelope + attestation to the receiving service; the bundle
   structure already matches the layout referenced in
   `docs/source/android_support_playbook.md#9-compliance--audit-artefacts`.

`OfflineTransactionBundle.attestation()` returns `Optional.empty()` when no
hardware provider produced a bundle; surface this in diagnostics so operators
can detect when flows fell back to software keys.

> **Reminder:** Android Keystore returns the attestation chain that was minted
> when the alias was provisioned. Passing `attestationChallenge` to
> `encodeAndSignEnvelopeWithAttestation(...)` records the value you expect
> `AttestationVerifier` to enforce; the challenge embedded inside the
> certificate chain only changes after deleting/recreating the alias with a new
> `KeyGenParameterSpec.Builder#setAttestationChallenge(...)`.

## 4. Queueing & Replay

The default file-backed queue (`FilePendingTransactionQueue`,
`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/queue/FilePendingTransactionQueue.java`)
stores envelopes instead of ad-hoc CSV records:

- `enqueue(...)` converts a `SignedTransaction` into an envelope, Base64 encodes
  the Norito blob, and appends it to `queueFile`.
- `drain()` decodes each envelope and rehydrates `SignedTransaction` instances
  (export bundles included) so callers can retry submissions deterministically.

For CLI tooling or desktop workflows, reuse `OfflineSigningEnvelopeCodec` to
persist envelopes in any medium (files, QR codes, BLE hand-offs). The codec is
dependency-free and available on every JVM target. When audit requirements call
for authenticated WAL storage, instantiate
`OfflineJournalPendingTransactionQueue` with an `OfflineJournal` key derived from
operator seed material or call
`ClientConfig.Builder.enableOfflineJournalQueue(path, passphraseOrKey)` to wire
the journal queue automatically (overloads accept a raw key, byte[] seed, or
UTF-8 passphrase) or wrap an existing config via
`HttpClientTransport.withOfflineJournalQueue(config, path, passphraseOrSeed)`
before instantiating the transport. Both Android queue implementations share the same
`PendingTransactionQueue` interface so swapping in the journal-backed variant is
transparent to `ClientConfig` consumers.

> **Inspector helper:** The `PendingQueueInspector` CLI
(`java/iroha_android/src/main/java/org/hyperledger/iroha/android/tools/PendingQueueInspector.java`)
decodes queue files and emits canonical hashes/aliases. Run `./run_tests.sh`
once to build the classes, then execute `java -cp build/classes
org.hyperledger.iroha.android.tools.PendingQueueInspector --file
<path> --json` to capture artefacts for AND7 Scenario D.

## 5. Operational Flow

Recommended hand-off steps for offline/mobile scenarios:

1. **Authoring device (air‑gapped):**
   - Build the payload via `TransactionBuilder`.
   - Call `encodeAndSignEnvelopeWithAttestation(...)` with the appropriate alias
     and security preference.
   - Store the resulting envelope (and optional attestation/export) in the
     pending queue or copy to removable media.
2. **Transfer:** Mirror the envelope into the secure support channel. If the
   metadata includes deterministic exports, encrypt the containing file with the
   operator’s passphrase and record the hash in the support ticket.
3. **Submitting device (online):**
   - Decode the envelope using `OfflineSigningEnvelopeCodec`.
   - Convert to `SignedTransaction` via `OfflineSigningEnvelope.toSignedTransaction()`.
   - Submit via `HttpClientTransport.submitTransaction(...)`. The helper preserves
     the original hash (via `SignedTransactionHasher`) so telemetry stays aligned.
   - When attestation is attached, upload the bundle to the operator console so
     compliance reviewers can reconcile it with the envelope metadata.

> **Ledger guardrails:** `RegisterOfflineAllowance` now rejects certificates whose
> operator signature doesn’t match the allowance asset controller, and
> `SubmitOfflineToOnlineTransfer` verifies every spend-key signature plus the
> remaining allowance tracked on-ledger before crediting funds. Keep the
> envelope metadata intact—tampering with the certificate or receipt payloads
> will now cause Torii to reject the bundle during execution.【crates/iroha_core/src/smartcontracts/isi/offline.rs:127】【crates/iroha_core/src/smartcontracts/isi/offline.rs:179】

> **Tip:** The `examples/android/retail-wallet` sample application wires this
> flow end-to-end and prints the demo hash + attestation status on launch.

### Inspector provisioning proofs (OA10.3a)

`AndroidProvisionedProof` loads the `proof.json` artefacts produced by
`cargo xtask offline-provision`, normalises the Norito hash literal + inspector
signature, and re-encodes the manifest deterministically before it is attached
to `RegisterOfflineAllowance` specs or audit bundles:

```java
final AndroidProvisionedProof proof =
    AndroidProvisionedProof.fromPath(
        Paths.get("fixtures/offline_provision/kiosk-demo/proof.json"));

final Map<String, Object> manifest = proof.deviceManifest();
final byte[] challengeHash = proof.challengeHashBytes();
final String canonicalJson = proof.toCanonicalJson();
```

The helper enforces `android.provisioned.device_id`, rejects malformed hash
literals, and keeps the inspector signature in uppercase hex so kiosks and POS
apps do not have to duplicate parsing logic when staging OA10.3a allowances.
【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/AndroidProvisionedProof.java:1】

## 6. Telemetry & Compliance

- Record envelope creation/removal events under the same alias used for signing;
  telemetry redaction (see `docs/source/sdk/android/telemetry_redaction.md`)
  ensures aliases are hashed before export.
- Update `docs/source/android_support_playbook.md#7-operational-checklists` with
  the envelope retention window for each pilot/GA environment.
- Attach deterministic export hashes and attestation digests to the incident or
  release records referenced in Section 9 of the playbook so auditors can verify
  the offline hand-off.

## 7. Testing Checklist

| Scenario | Steps |
|----------|-------|
| CI regression | Run `make android-tests` and `ci/run_android_tests.sh`; the suite exercises `OfflineSigningEnvelope` round trips and queue persistence. |
| Wallet/console integration | Add instrumentation to the AND5 sample apps so Managed Device runs exercise both the envelope creation path and queued replay. |
| Attestation validation | Use `scripts/android_keystore_attestation.sh` against bundles emitted during `encodeAndSignEnvelopeWithAttestation(...)` to ensure the StrongBox flow is wired end-to-end. |
| Override rehearsal | Execute the telemetry chaos scenario #2
  (`docs/source/sdk/android/telemetry_chaos_checklist.md`) to confirm overrides
  correctly reference envelope metadata and exported key hashes. |

Track results in `status.md` (Android section) and archive supporting artefacts
in `docs/source/sdk/android/readiness/` for AND5/AND7 readiness gates.

## 8. Offline reserve flow

The first retail reserve release removed the pre-release allowance/certificate/settlement inspection
flow from the supported Android SDK documentation. Wallets should use the reserve routes directly:

- `POST /v1/offline/cash/setup`
- `POST /v1/offline/cash/load`
- `POST /v1/offline/cash/refresh`
- `POST /v1/offline/cash/sync`
- `POST /v1/offline/cash/redeem`
- `GET /v1/offline/revocations`
- `GET /v1/offline/transfers` and `POST /v1/offline/transfers/query` only when transfer history is required

Offline cash lineage envelopes are authoritative for balance, locked balance, authorization policy, and
revocation freshness. The old allowance/summaries/state/spend-receipts/settlements APIs are not part
of the shipped offline cash wallet path.

When journaling is required, persist the lineage anchor, locked/spendable split, pending receipts,
pending offline cash mutations, seen transfer ids, sender-state replay guards, revocation bundle, and
App Attest key id together so incomplete state freezes the wallet instead of recreating value.

## QR stream handoff

Use the QR stream helpers when passing offline receipts or bundles between devices:

- **Encode frames:** `OfflineQrStream.Encoder.encodeFrames(...)` produces header/data/parity frames.
- **Scan pipeline:** `OfflineQrStreamCameraXScanner` wires CameraX + ZXing to feed raw bytes into
  `OfflineQrStream.ScanSession`.
- **Text fallback:** if QR decoders return text, decode
  `iroha:qr1:<base64(frame_bytes)>` via `OfflineQrStream.TextCodec`.
- **Playback skins:** use `OfflineQrStream.SAKURA_SKIN` for default playback,
  `SAKURA_REDUCED_MOTION_SKIN` for accessibility, and `SAKURA_LOW_POWER_SKIN` for battery-friendly
  animations.

## Petal stream handoff (custom scanner)

Petal stream renders the same QR stream frames as a sakura petal field and requires a custom
scanner:

- **Encode grids:** `OfflinePetalStream.Encoder.encodeGrids(...)` selects a consistent grid size
  for a frame set.
- **CameraX scanner:** `OfflinePetalStreamCameraXScanner` samples luminance and feeds decoded
  frames into `OfflineQrStream.Decoder`.
- **Manual sampling:** use `OfflinePetalStream.Sampler.sampleGridFromLuma(...)` or
  `sampleGridFromRgba(...)` with your own rendering pipeline.

`OfflineTransferPayloads.attachPlatformSnapshot(...)` wires this JSON snippet into either the
in-memory map passed to the Norito encoder or a staged envelope pulled from disk:

```java
var snapshot = wallet.buildSafetyDetectPlatformTokenSnapshot("deadbeef")
    .orElseThrow(() -> new IllegalStateException("Safety Detect token unavailable"));

Map<String, Object> transferPayload = Map.of(
    "bundle_id", bundleHex,
    "receiver", receiverId,
    "deposit_account", depositAccountId);

Map<String, Object> transferWithSnapshot =
    OfflineTransferPayloads.attachPlatformSnapshot(transferPayload, snapshot);
byte[] patchedJson = OfflineTransferPayloads.attachPlatformSnapshot(originalJsonBytes, snapshot);
```

For per-receipt attestations (e.g., OA10 POS imports), embed the snapshot
directly alongside each receipt so validators prefer it over the bundle-level
value:

```json
{
  "transfer": {
    "bundle_id": "3b6a27bccebfb63b9a...",
    "receipts": [
      {
        "tx_id": "00ff...",
        "from": "<katakana-i105-account-id>",
        "to": "<katakana-i105-account-id>",
        "issued_at_ms": 1730314876000,
        "platform_proof": { "...": "..." },
        "platform_snapshot": {
          "policy": "play_integrity",
          "attestation_jws_b64": "eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9..."
        }
      }
    ],
    "...": "..."
  }
}
```

The receipt-scoped variant lets wallets mix Play Integrity and HMS Safety Detect
tokens in the same bundle while keeping governance evidence deterministic.

Use the map variant when constructing the `SubmitOfflineToOnlineTransfer` object graph and the byte
variant when amending canonical JSON envelopes prior to submission.

Validators verify the supplied token directly, so ensure the base64 string is
exactly what HMS/Play returned (no pretty-printing or additional encoding). Use
`policy: "play_integrity"` when attaching Play Integrity tokens; mixing policies
or metadata will cause the bundle to be rejected during admission.

```java
OfflineJournalKey journalKey = OfflineJournalKey.derive("merchant-seed".getBytes(StandardCharsets.UTF_8));
try (OfflineJournal journal =
         new OfflineJournal(context.getFilesDir().toPath().resolve("offline_journal.bin"), journalKey)) {
  OfflineJournalEntry entry = journal.appendPending(txIdBytes, noritoBundleBytes);
  // ... submit bundle to Torii ...
  journal.markCommitted(txIdBytes);
}
```

The audit log mirrors the Swift SDK fields and now carries the true sender and
asset identifiers for every recorded bundle, satisfying the OA5 replay/audit
expectations called out in the roadmap.
