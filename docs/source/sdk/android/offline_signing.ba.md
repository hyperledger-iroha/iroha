---
lang: ba
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

## 8. Offline Allowance Inspection & Auditing

- `OfflineToriiClient` reuses the primary transport stack (base URI, observers,
  default headers) so SDKs can query `/v1/offline/allowances`,
  `/v1/offline/transfers` (and `/v1/offline/settlements` alias), `/v1/offline/receipts`,
  and `/v1/offline/state` without duplicating HTTP plumbing. Pass
  `OfflineListParams` to control pagination, filters, and address formatting,
  mirroring the Torii REST surface described in the OA7 roadmap item.
- `OfflineListParams.Builder` exposes the same roadmap filters shipped on the
  REST side — `assetId`, `certificateExpiresBeforeMs/AfterMs`,
  `policyExpiresBeforeMs/AfterMs`, and `verdictIdHex` — along with
  `requireVerdict` / `onlyMissingVerdict`
  toggles so dashboards can target hot certificates without composing JSON
  predicates. Invalid combinations (for example, `verdictIdHex` +
  `onlyMissingVerdict`) are rejected when `build()` is invoked to keep client
  behaviour aligned with Torii validation.
- `OfflineToriiClient.topUpAllowance(...)` issues a certificate and registers it
  on-ledger in one call, and `topUpAllowanceRenewal(...)` does the same for
  renewals. `OfflineWallet.topUpAllowance(...)` / `topUpAllowanceRenewal(...)`
  wrap the flow and (by default) record the issued certificate into the verdict
  journal so refresh warnings work without an extra list call.
- `OfflineWallet.fetchTransfersWithAudit(...)` parses the first receipt from
  each `OfflineToOnlineTransfer` bundle and records `{sender_id, receiver_id,
  asset_id, amount}` using the ledger payload instead of mirroring the
  top-level receiver twice. This keeps regulator exports aligned with the actual
  spend receipts.
- `/v1/offline/summaries` (and the POST-based `.query` variant) returns the
  controller-facing hardware counter checkpoints for each registered allowance.
  Use this feed when merchants or compliance tooling need to verify App Attest
  key counters or Android marker series without downloading every allowance
  record. The SDK models expose the same fields (`certificate_id_hex`,
  `controller_id`, per-platform counter maps, and the deterministic summary
  hash) so mobile code can diff checkpoints locally.
- `OfflineWallet.fetchSummaries(...)` records the same checkpoints into
  `offline_counter_journal.json` so POS clients can enforce monotonic counters
  without storing full allowance payloads. Use
  `recordAppleCounter(...)`, `recordAndroidSeriesCounter(...)`, or
  `recordProvisionedCounter(...)` when assembling receipts to ensure counters
  advance exactly once per spend; the helper throws `OfflineCounterException`
  with deterministic reasons when a jump or hash mismatch is detected.
- `/v1/offline/revocations{,/query}` exposes the attestation verdict deny-list
  maintained on-ledger. `OfflineToriiClient.listRevocations|queryRevocations`
  return `OfflineRevocationList` instances mirroring the REST payload (flattened
  fields plus the raw Norito record), and `OfflineWallet.fetchRevocations(...)`
  passes through the same helpers so POS clients can page revocations using the
  familiar facade. Each item includes the human-readable issuer, display-ready
  account string, canonical reason, optional note/metadata maps, and the full
  record JSON for downstream auditing.
- `/v1/offline/transfers/proof` accepts a transfer payload (`transfer`) and responds with the
  canonical `OfflineProofRequest*` payloads. Use it to build `{sum,counter,replay}` witness JSON
  before admission.
- `/v1/offline/spend-receipts` accepts raw receipts and returns the canonical
  Poseidon `receipts_root` (`OfflineSpendReceiptsSubmitResponse`) so wallets can
  cross-check their local hashing (and surface structured failures) before
  attempting ledger reconciliation.
- `/v1/offline/settlements` submits an offline-to-online bundle for ledger
  admission (submits `SubmitOfflineToOnlineTransfer`). Use this endpoint only
  when operating in ledger-reconcilable mode; offline-only circulation treats
  receipts as final and does not require settlement. When rejection occurs,
  Torii surfaces a stable reason code in the `x-iroha-reject-code` response
  header (for example `certificate_expired`, `counter_conflict`,
  `max_tx_value_exceeded`, `allowance_exceeded`, `invoice_duplicate`) so apps can map failures to
  deterministic UX copy across platforms.
- When OA2 journaling is required, persist envelopes via
  `OfflineJournal` (`offline/OfflineJournal.java`). The helper mirrors the
  Rust/Swift layout: entries are hash-chained with BLAKE2b-256 and authenticated
  via HMAC-SHA256 using a 32-byte key derived from operator seed material so
  regulators can replay the write-ahead log across platforms.
- `OfflineReceiptChallenge.compute(chainId, ...)` wraps the shared native helper
  (`connect_norito_offline_receipt_challenge`) so Android apps can derive the
  canonical Norito payload plus the chain-bound BLAKE2b/SHA-256 hashes that
  KeyMint proofs expect without touching the codec directly. Pass the receipt’s
  `issued_at_ms` (unix ms) into the helper so the platform challenge stays
  aligned with the ledger freshness checks. Receipt `amount` values must use
  the allowance's canonical scale (asset definition scale when specified;
  otherwise the allowance amount scale) to match ledger verification
  rules. Use the overload that accepts `expectedScale` to enforce the scale
  locally.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineReceiptChallenge.java:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/OfflineReceiptChallengeTest.java:1】
- `OfflineBalanceProof.advanceCommitment(...)` generates the new commitment and
  the required v1 proof blob (12,385 bytes: version + delta proof + range proof)
  for offline-to-online settlement. Provide the claimed delta, resulting value,
  and current/next blinding factors to stay aligned with the ledger verifier:

```java
OfflineBalanceProof.Artifacts artifacts =
    OfflineBalanceProof.advanceCommitment(
        chainId,
        claimedDelta,
        resultingValue,
        initialCommitmentHex,
        initialBlindingHex,
        resultingBlindingHex);
```
- `OfflineWallet.CirculationMode` distinguishes between ledger-reconcilable
  allowances and pure offline/bearer pilots. Provide a
  `OfflineWallet.ModeChangeListener` to surface the warning copy from
  `CirculationNotice` whenever operators flip modes, and guard Torii calls when
  `wallet.requiresLedgerReconciliation()` returns `false`. The broader risk
  guidance lives in `docs/source/offline_bearer_mode.md`.
- Verdict metadata (`refresh_at_ms`, policy/certificate expiries, verdict/nonce
  identifiers, remaining allowance value) is now flattened in the REST payloads
  and exposed through `OfflineAllowanceList`. `OfflineWallet` automatically
  persists this information to `offline_verdict_journal.json` (next to the audit
  log) and provides `verdictWarnings(thresholdMs)` plus `verdictMetadata()` so
  apps can render “refresh cached verdict” banners and archive the raw JSON for
  incident response. Each `OfflineVerdictWarning` bundles controller/id/verdict
  labels with pre-formatted `headline`/`details` strings (matching the Swift
  SDK) to keep UI copy aligned across platforms. The warnings honour refresh
  deadlines first, then policy expiry, then certificate expiry; once the
  deadline passes they flip to `State.EXPIRED` and continue to surface until a
  fresh verdict is registered.
- `OfflineWallet.ensureFreshVerdict(...)` refuses stale attestations before a
  bundle is submitted. Pass the certificate id (and optionally the cached
  `attestation_nonce_hex`) to ensure the refresh/policy/certificate deadlines
  are still valid; the helper throws `OfflineVerdictException` with the exact
  `DeadlineKind` or a nonce mismatch reason so UI layers can trigger a refresh
  flow immediately:

```java
try {
  wallet.ensureFreshVerdict("deadbeef", cachedNonceHex);
} catch (OfflineVerdictException verdictError) {
  switch (verdictError.reason()) {
    case NONCE_MISMATCH -> showNonceMismatch(verdictError.expectedNonceHex());
    case DEADLINE_EXPIRED -> promptForRefresh(verdictError.deadlineKind(), verdictError.deadlineMs());
    default -> throw verdictError;
  }
}
```
- `OfflineWallet.verdictMetadata("deadbeef")` returns the cached struct whenever
  you need to display controller-friendly copy or log the stored nonce/verdict
  identifiers alongside audit evidence.
- Typical usage stitches everything together:

```java
OfflineListParams params = OfflineListParams.builder()
    .limit(25L)
    .addressFormat("canonical")
    .build();

OfflineWallet wallet =
    transport.offlineWallet(
        context.getFilesDir().toPath().resolve("offline_audit.json"),
        true /* enable audit logging */);

wallet.fetchTransfersWithAudit(params)
    .thenAccept(list ->
        System.out.println("Logged " + list.items().size() + " offline bundles"));

byte[] auditJson = wallet.exportAuditJson();
// Forward auditJson to compliance or persist the digest locally
```

```java
OfflineQueryEnvelope query = OfflineQueryEnvelope.builder()
    .filterJson("{\"op\":\"eq\",\"args\":[\"receiver_id\",\"ih58...\"]}")
    .setLimit(25L)
    .build();

  transport.offlineToriiClient().queryTransfers(query)
    .thenAccept(list -> System.out.println("Queried " + list.total() + " offline bundles"));
```

### HMS Safety Detect attestation snapshots

Allowances that advertise the `hms_safety_detect` policy now round-trip their
attestation tokens locally so POS devices can attach the snapshot to the
`SubmitOfflineToOnlineTransfer` bundle:

1. Call `wallet.fetchSafetyDetectAttestation(certificateId)` to mint a fresh
   token. The helper enforces the issuer metadata (package name, signer digests,
   nonce, and required evaluations) and caches the JWS alongside the verdict
   journal.
2. When preparing the bundle, call
   `wallet.buildSafetyDetectPlatformTokenSnapshot(certificateId)`. The helper
   loads the cached token, checks `max_token_age_ms`, and returns the
   `{ policy, attestation_jws_b64 }` pair expected by Torii.

The cache persists inside `offline_verdict_journal.json` next to the audit log,
and every snapshot is checked against `max_token_age_ms` before it is attached:

```java
wallet.fetchSafetyDetectAttestation("deadbeef")
    .thenCompose(__ ->
        wallet.buildSafetyDetectPlatformTokenSnapshot("deadbeef")
            .map(snapshot -> submitBundle(snapshot))
            .orElseGet(() -> CompletableFuture.failedFuture(
                new IllegalStateException("Safety Detect token expired"))));
```

`PlatformTokenSnapshot.policy` is always `hms_safety_detect` and the
attestation is returned as base64-encoded JWS bytes. Swift/JS SDKs expose the
same API surface so cross-SDK parity holds when OA10.1a is exercised.

### Play Integrity attestation snapshots

Play Integrity tokens follow the same workflow when `android.integrity.policy`
is `play_integrity`:

1. Call `wallet.fetchPlayIntegrityAttestation(certificateId)` to mint a token.
   The helper validates the issuer metadata (project number, package name,
   signing digest allowlist, and verdict classes) and caches the token in the
   verdict journal.
2. Call `wallet.buildPlayIntegrityPlatformTokenSnapshot(certificateId)` when
   submitting a bundle. The helper checks `max_token_age_ms` and returns
   `{ policy, attestation_jws_b64 }` when the cached token is still valid.

```java
wallet.fetchPlayIntegrityAttestation("deadbeef")
    .thenCompose(__ ->
        wallet.buildPlayIntegrityPlatformTokenSnapshot("deadbeef")
            .map(snapshot -> submitBundle(snapshot))
            .orElseGet(() -> CompletableFuture.failedFuture(
                new IllegalStateException("Play Integrity token expired"))));
```

When writing the bundle to disk (JSON or Norito), include the snapshot under the
top-level `transfer` object or alongside the receipt that generated the token:

```json
{
  "transfer": {
    "bundle_id": "3b6a27bccebfb63b9a...",
    "receiver": "ih58...",
    "deposit_account": "ih58...",
    "receipts": [ /* ... */ ],
    "balance_proof": { /* ... */ },
    "platform_snapshot": {
      "policy": "hms_safety_detect",
      "attestation_jws_b64": "eyJhbGciOiJFUzI1NiIsInR5cCI6IkpXVCJ9..."
    }
  }
}
```

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
        "from": "ih58...",
        "to": "ih58...",
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
