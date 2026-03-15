---
lang: ur
direction: rtl
source: docs/source/offline_allowance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 203c8345299d6940a8a8e1151a6a59eebcffd0737516fff5ca3ed505752a82b0
source_last_modified: "2026-01-30T11:39:36.941648+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Offline Allowance Commitments & Ledger Re-entry
---

Status: 🈴 Completed — OA1–OA5 delivered (commitments + certificates, platform counter verifiers and storage hardening, receiver counter sharing, `OfflineTransfer` admission + aggregate proofs, and WAL/audit tooling + SDK audit toggles).

This note captures the canonical shape of offline allowances, certificates, receipts, and ledger
admission for depositing offline spends back on-chain. It accompanies the OA1 roadmap item and
describes how operators issue commitments, how wallets prove hardware counters, and how the new
tooling under `scripts/offline_topup/` should be used to mint fixtures.

## 1. Goals

- Bind every offline top-up to a deterministic commitment `C_0 = Com(asset, amount, r_0)` and a
  signed certificate so issuers can reason about exposure.
- Require mobile wallets to pair each spend with hardware attestation material and monotonic
  counters so ledgers can reject replayed receipts.
- Provide deterministic encodings (Norito + JSON) so SDKs/shareable fixtures exercise the exact
  payloads that `iroha_core` verifies via `RegisterOfflineAllowance` and
  `SubmitOfflineToOnlineTransfer`.

## 2. Data Model Summary

### 2.1 Allowance commitments & certificates

| Type | Fields | Notes |
|------|--------|-------|
| `OfflineAllowanceCommitment` | `asset: AssetId`,<br>`amount: Numeric`,<br>`commitment: Vec<u8>` | `commitment` contains the Pedersen digest binding the allowance to its asset owner. |
| `OfflineWalletPolicy` | `max_balance`, `max_tx_value`, `expires_at_ms` | Policy bounds enforced independently from allowance expiry. |
| `OfflineWalletCertificate` | `controller`, `allowance`, `spend_public_key`, `attestation_report`, `issued_at_ms`, `expires_at_ms`,<br>`policy`, `operator_signature`, `metadata` | Operators sign the canonical payload returned by `OfflineWalletCertificate::operator_signing_bytes()`. The controller is the on-ledger account that later submits `RegisterOfflineAllowance`. |

Certificates are uniquely identified by `certificate_id = blake2b32(Norito(certificate))` inside
`RegisterOfflineAllowance`. The operator signature **must** be produced with the private key of the
account that controls `allowance.asset.account()` (ledger rejects mismatched key material).

### 2.2 Metadata requirements

Hardware attestation requirements live in the certificate metadata and are enforced during
`SubmitOfflineToOnlineTransfer` when `verify_platform_proof(...)` runs:

| Platform | Required keys | Description |
|----------|---------------|-------------|
| iOS App Attest | `ios.app_attest.team_id`, `ios.app_attest.bundle_id`, `ios.app_attest.environment` (`production`/`development`) | Used to validate the CBOR chain and distinguish prod/dev provisioning profiles. |
| Android marker key / KeyMint | `android.integrity.policy = "marker_key"`, `android.attestation.package_names` (array), `android.attestation.signing_digests_sha256` (array), optional `android.attestation.require_strongbox` (bool, default `false`), optional `android.attestation.require_rollback_resistance` (bool, default `true`) | Scopes the KeyMint attestation to specific package/signing chains and declares whether StrongBox + rollback resistance are mandatory for alias counters. |
| Android Play Integrity | `android.integrity.policy = "play_integrity"`, `android.play_integrity.cloud_project_number` (u64), `android.play_integrity.environment` (`production`/`testing`), `android.play_integrity.package_names`, `android.play_integrity.signing_digests_sha256`, `android.play_integrity.allowed_app_verdicts` (array of `play_recognized`/`licensed`/`unlicensed`), `android.play_integrity.allowed_device_verdicts` (array of `strong`/`device`/`basic`/`virtual`), optional `android.play_integrity.max_token_age_ms` | Declares the Google Cloud project, package/signer allowlist, environment, and acceptable app/device verdict classes so Play Integrity tokens can be verified deterministically. |
| Android HMS Safety Detect | `android.integrity.policy = "hms_safety_detect"`, `android.hms_safety_detect.app_id`, `android.hms_safety_detect.package_names`, `android.hms_safety_detect.signing_digests_sha256`, `android.hms_safety_detect.required_evaluations` (array of `basic_integrity`/`system_integrity`/`strong_integrity`), optional `android.hms_safety_detect.max_token_age_ms` | Captures the Huawei/HarmonyOS App ID, package/signer lists, evaluation classes, and optional verdict TTL enforced by the allowance. |
| Android provisioned path | `android.integrity.policy = "provisioned"`, `android.provisioned.inspector_public_key`, `android.provisioned.manifest_schema`, optional `android.provisioned.manifest_version`, optional `android.provisioned.max_manifest_age_ms`, optional `android.provisioned.manifest_digest` | Used for operator-run provisioning: the inspector key signs manifests, the schema/version/digest declare the manifest layout, and the max-age gate keeps cached manifests fresh. |

Node operators can append extra Android trust anchors (e.g., OEM-issued roots) via the node configuration: add DER file paths to `settlement.offline.android_trust_anchor_files` inside `config.toml`. These anchors supplement the built-in Google roots when verifying KeyMint / marker-key attestations.

Certificates may optionally embed refresh metadata used by SDKs/POS terminals; these fields also drive attestation caching:

- `verdict_id` – unique identifier for the cached attestation verdict returned by the attestation provider.
- `attestation_nonce` – nonce Torii issued (and the provider echoed) during registration so wallets can prove nonce lineage.
- `refresh_at_ms` – unix timestamp (ms) indicating when the cached verdict expires and must be refreshed on-device.

When `android.integrity.policy = "hms_safety_detect"`, issuers **must** populate the `android.hms_safety_detect.*` keys so Torii and governance scripts understand quota windows and nonce behaviour. The same rule applies to `android.play_integrity.*` and `android.provisioned.*`: if the policy slug is present, the associated metadata block must be complete.

#### Android Play Integrity metadata

Play Integrity allowances delegate attestation to Google’s backend. The metadata block captures the project, package/signing allowlists, verdict classes, and allowed token age:

```json
"metadata": {
  "android.integrity.policy": "play_integrity",
  "android.play_integrity.cloud_project_number": 4242424242,
  "android.play_integrity.environment": "production",
  "android.play_integrity.package_names": ["tech.iroha.retail.play"],
  "android.play_integrity.signing_digests_sha256": [
    "778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF00112233445566"
  ],
  "android.play_integrity.allowed_app_verdicts": ["play_recognized", "licensed"],
  "android.play_integrity.allowed_device_verdicts": ["strong", "device"],
  "android.play_integrity.max_token_age_ms": 30000
}
```

- **Cloud project + environment.** The project number routes tokens to the correct Google Cloud project while `environment` chooses production vs. test endpoints.
- **Verdict classes.** The allowed-app/device lists gate which `appIntegrity`/`deviceIntegrity` verdicts can appear; allowances should usually require at least `play_recognized` + `device`.
- **Token freshness.** `max_token_age_ms` keeps cached Integrity verdicts fresh and mirrors the `refresh_at_ms` helper on the certificate.
- **Fixtures.** The `android-play-demo` entry inside `scripts/offline_topup/spec.example.json` exercises the metadata above so CI and SDKs can share canonical fixtures.

#### Huawei Safety Detect metadata & quotas

Safety Detect allowances mirror the marker-key metadata (package + signing digests) and add a Huawei-specific block:

```json
"metadata": {
  "android.integrity.policy": "hms_safety_detect",
  "android.hms_safety_detect.app_id": "103000042",
  "android.hms_safety_detect.package_names": ["tech.iroha.retail.harmony"],
  "android.hms_safety_detect.signing_digests_sha256": [
    "1f2e3d4c5b6a79880796a5b4c3d2e1f0ffeeddccbbaa99887766554433221100"
  ],
  "android.hms_safety_detect.required_evaluations": [
    "strong_integrity",
    "system_integrity"
  ],
  "android.hms_safety_detect.max_token_age_ms": 3600000
}
```

- **App identity.** `android.hms_safety_detect.app_id` records the AppGallery App ID issued by Huawei so Torii can validate the verdict’s audience.
- **Nonce lifecycle.** HMS Core 6.12 enforces a 90 s nonce TTL and rejects reused nonces. Torii issues the nonce (`attestation_nonce_hex` in the spec), mirrors it inside the certificate, and expires unused nonces after 90 s so wallets cannot race stale material.
- **Verdict refresh.** `android.hms_safety_detect.max_token_age_ms` (and the matching `refresh_at_ms` field) inform wallets when to re-fetch a fresh verdict before reconciliation.
- **Quota telemetry.** Operators should still record the quota envelopes published by Huawei (600 evaluations/minute, 200 000/day as of 2025‑02) even though the new metadata block focuses on verification inputs. Torii meters every attestation attempt against these limits and emits back-pressure once 80 % of the window is consumed so operators can throttle offline issuance.
- **Sample data.** `scripts/offline_topup/spec.example.json` now ships the `android-hms-demo` allowance exercising this metadata, and the generated fixtures described in §7 reference the same values so CI/tests cover the Safety Detect policy end-to-end.
- **SDK hooks.** Android’s `OfflineWallet` exposes a Safety Detect policy toggle plus a configurable
  HTTP client (`HttpSafetyDetectService`) so applications can fetch fresh HMS verdicts and log
  telemetry before staging bundles back to Torii (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineWallet.java`).

#### Provisioned inspector metadata

Provisioned allowances skip OEM attestations and rely on an operator-run inspector. The metadata block binds manifests to a known schema and key:

```json
"metadata": {
  "android.integrity.policy": "provisioned",
  "android.provisioned.inspector_public_key": "ed0120C0FFEE...DEADBEEF",
  "android.provisioned.manifest_schema": "offline_provisioning_v1",
  "android.provisioned.manifest_version": 1,
  "android.provisioned.max_manifest_age_ms": 604800000,
  "android.provisioned.manifest_digest": "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
}
```

- **Inspector key.** Must be a multihash-formatted public key; operators rotate the inspector via certificate re-issuance.
- **Schema + digest.** `manifest_schema` labels the manifest layout, while `manifest_digest` (optional) lets operators pin an exact manifest template.
- **Freshness.** `max_manifest_age_ms` mirrors `refresh_at_ms` and prevents stale manifest/counter material from being replayed weeks later.
- **Fixtures.** The `android-provisioned-demo` allowance in `scripts/offline_topup/spec.example.json` exercises this metadata so SDK tooling can round-trip provisioned flows.

Additional metadata entries (audience, jurisdiction flags, regulator notes, etc.) may be included,
but required keys must be present or ledger admission fails.

### 2.3 Verdict revocation registry

Issuers and governance operators can revoke cached attestation verdicts without tearing down the
entire allowance by registering an `OfflineVerdictRevocation`:

| Field | Description |
|-------|-------------|
| `verdict_id` | Identifier returned by the attestation backend when the certificate was provisioned. |
| `issuer` | Account that executed the revocation on-ledger (usually the operator or governance daemon). |
| `revoked_at_ms` | Unix timestamp when the revocation landed on-chain. |
| `reason` | Canonical reason code (`unspecified`, `device_compromised`, `device_lost_or_stolen`, `policy_violation`, `issuer_request`). |
| `note` | Optional human-readable note or incident id. |
| `metadata` | Structured metadata (JSON/Norito) for auditors—attach incident tickets, regulator references, etc. |

`RegisterOfflineVerdictRevocation` stores these records in `offline_verdict_revocations` keyed by
`verdict_id`. Torii exposes them via `/v1/offline/revocations` (JSON list + query envelope), and
`iroha_cli offline revocation list` prints the same data so POS devices or merchant tooling can sync
a deny list even when they only have intermittent connectivity.

## 3. Platform Proofs & Counters

`OfflinePlatformProof` currently supports:

- `AppleAppAttest`: `{ key_id, counter, assertion, challenge_hash }`
- `AndroidMarkerKey`: `{ series, counter, marker_public_key, marker_signature?, attestation }`
- `AndroidProvisioned`: `{ manifest_schema, manifest_version?, manifest_issued_at_ms, challenge_hash, counter, device_manifest, inspector_signature }`

`AppleAppAttest.key_id` is the canonical standard-base64 encoding of the App Attest key id bytes
(no whitespace, with padding as needed). Torii rejects non-canonical encodings to keep counter
scopes stable.

`AndroidMarkerKey.marker_public_key` is the 65-byte uncompressed SEC1 P-256 public key
(`0x04 || X || Y`). `marker_signature` is an optional 64-byte raw signature (`r || s`) over the
receipt challenge hash; when present it must verify against `marker_public_key`. The `series`
value is derived as `marker::<hash>`, where `hash` is the lowercase hex BLAKE2b-256 digest of
`marker_public_key`, and must match the derived value.

`AndroidProvisioned` counters are scoped to `provisioned::<manifest_schema>::<device_id>`, where
`device_id` is read from `device_manifest["android.provisioned.device_id"]`.

Counters are tracked inside `OfflineCounterState` (per-`key_id` for iOS, per-`series`/provisioned
scope for Android). Each bundle must keep receipts within a single counter scope; mixed scopes are
rejected with `mixed_counter_scopes`.
`SubmitOfflineToOnlineTransfer` clones the stored state, stages every receipt via
`stage_receipt_counters`, and enforces strictly monotonic increments (no gaps and no rewinds).

The challenge each platform signs is derived from the receipt payload by encoding the
`OfflineReceiptChallengePreimage` struct with Norito and hashing
`Hash(chain_id) || preimage_bytes`. This binds every receipt to the chain ID and prevents
cross-context replays. The preimage struct lives in `iroha_data_model::offline` and mirrors the
fields every receipt includes:

```text
struct OfflineReceiptChallengePreimage {
    invoice_id: String,
    receiver: AccountId,
    asset: AssetId,
    amount: Numeric,
    issued_at_ms: u64,
    sender_certificate_id: Hash,
    nonce: Hash,    // derived from the receipt's tx_id
}
```

Rust, Swift, and Android SDKs now expose helpers that call the shared native bridge
(`connect_norito_offline_receipt_challenge`, `OfflineReceiptChallenge.encode`, and
`OfflineReceiptChallenge.compute`) so wallets can request the canonical bytes + hashes without
re-implementing the Norito encoder. The `challenge_hash` stored in `AppleAppAttestProof` and the
KeyMint `client_data_hash` are both validated against this derived digest, so wallets **must not**
mutate invoice IDs or amounts once the receipt is signed. Provisioned manifests extend the same
guarantees to inspector-run kiosks: `android.provisioned.device_id` must be present so the ledger
can enforce the strictly increasing inspector counter per `{manifest_schema}::{device_id}` scope,
and the optional `android.provisioned.manifest_digest` declared on the certificate is compared
against the Norito-encoded manifest bytes before verifying the inspector signature. Operators also
bound freshness via `android.provisioned.max_manifest_age_ms`; after that TTL expires Torii rejects
receipts until a new manifest is registered.

## 4. Optical handoff (QR stream / petal stream)

Offline receipts and bundles can be handed off via the QR stream transport (`docs/source/qr_stream.md`)
or the optional Petal Stream transport (`docs/source/petal_stream.md`). Use the animated QR UI for
short-range transfers between POS terminals, operator consoles, and mobile wallets. Petal Stream
requires a custom scanner but offers branded, sakura-styled playback.

### 4.1 Scan timing and progress UI

- **Warm-up frames.** Always show the header frame at least once per second so late scanners can
  join mid-stream.
- **Progress overlay.** Surface `received / total` and a percentage ring based on
  `QrStreamDecodeResult.progress`.
- **Cadence.** 10-12 fps for default playback, 6-8 fps for reduced-motion/low-power modes.
- **Stall fallback.** If no new frames land within 2x the frame interval, prompt the user to
  re-center the camera or switch to the text fallback.

### 4.2 Text fallback flow

Some scanners only expose QR text. Use the text wrapper:

```
iroha:qr1:<base64(frame_bytes)>
```

If the camera API only returns `stringValue` (iOS AVFoundation, browser BarcodeDetector),
decode the prefix and feed the bytes into the QR stream decoder.

### 4.3 Failure recovery

- If progress stalls below 60%, switch to a slower animation profile (reduced motion).
- If progress stalls above 60%, emit the remaining frames as text (copy/paste or NFC fallback).
- Always re-verify `payload_hash` before submitting offline bundles.

Rust data-model callers can now compute these digests directly via
`OfflineReceiptChallengePreimage::hash_with_chain_id()` or
`OfflineSpendReceipt::challenge_hash_with_chain_id()`, avoiding the manual
`to_bytes()`/`Hash::new(...)` plumbing that SDKs previously reimplemented. Use these helpers when
producing host-side attestations or when verifying receipts in tests to stay aligned with the
canonical Norito encoding described above.

## 4. Receipts, Balance Proofs, and Bundles

- `OfflineSpendReceipt` includes the sender/receiver accounts, asset/amount, invoice identifier,
  issuance timestamp (`issued_at_ms`), platform proof, sender certificate id, the sender’s
  spend-key signature, and
  optional receipt-scoped `platform_snapshot` metadata when wallets attach a Play
  Integrity or HMS Safety Detect token for that specific spend.
- `OfflineBalanceProof` carries `{ initial_commitment, resulting_commitment, claimed_delta, zk_proof }`.
- `OfflineToOnlineTransfer` is the bundle submitted to the ledger: `{ bundle_id, receiver,
  deposit_account, receipts[], balance_proof, aggregate_proof?, attachments?, platform_snapshot? }`.
  The optional `aggregate_proof` field carries the Poseidon root plus the three FASTPQ proofs
  described below. `platform_snapshot` lets wallets attach the exact Play Integrity or HMS Safety
  Detect token (`{ policy, attestation_jws_b64 }`) used during settlement; validators prefer the
  receipt-level snapshot (`receipts[].platform_snapshot`) when present and fall back to the bundle
  snapshot or cached certificate token otherwise.
- Receipts inside a bundle must be ordered by `(counter, tx_id)`; Torii rejects unsorted bundles
  with `receipt_order_invalid`.
- `OfflineAllowanceRecord` is the on-ledger mirror stored after registration; it tracks the latest
  commitment, `remaining_amount`, and the most recent counter checkpoints.

### 4.0 Receipt timestamp guardrails

Every receipt includes `issued_at_ms` (unix ms). Torii enforces the following ordering when
admitting bundles:

- `issued_at_ms` must be >= the certificate issuance time.
- `issued_at_ms` must be >= the allowance registration time.
- `issued_at_ms` must be <= the certificate/policy expiry bound.
- `issued_at_ms` must be <= the block timestamp (no future receipts).
- If `settlement.offline.max_receipt_age_ms` is non-zero, receipts older than that window are
  rejected.

Violations map to the `receipt_timestamp_invalid` or `receipt_expired` rejection codes so SDKs can
surface deterministic UX. The default `max_receipt_age_ms` is 86,400,000 (24 hours); set it to `0`
to disable the age gate in private deployments.

### 4.1 Balance-proof proofs (Pedersen representation)

- `zk_proof` is required for settlement: validators reject bundles that omit the proof or supply an
  empty payload, and the proof is checked before any allowance value is credited on-ledger.
  【crates/iroha_core/src/smartcontracts/isi/offline/balance_proof.rs:24】
- Commitments are interpreted as **Pedersen commitments over Ristretto255**, using the global
  basepoint `G` and an independent generator `H = hash_to_group("iroha.offline-balance.generator.H.v1")`.
  A proof shows knowledge of the blinding delta between `C_init` and `C_res` consistent with the
  public `claimed_delta`. Fiat–Shamir is applied with BLAKE2b over the ordered tuple
  `(C_init, C_res, delta_le, context, U, R)` under the label
  `"iroha.offline.balance.v1"`.【crates/iroha_core/src/smartcontracts/isi/offline/balance_proof.rs:52】
- Proof bytes are versioned and contain both the delta proof and a 64-bit range proof that asserts
  the resulting commitment encodes `v_new in [0, 2^64 - 1]`:

  | Offset | Size | Field | Notes |
  |--------|------|-------|-------|
  | 0 | 1 | `version` | Currently `0x01` |
  | 1 | 96 | `delta_proof` | Chaum–Pedersen proof for `C_res - C_init` |
  | 97 | 12288 | `range_proof` | 64 per-bit proofs (192 bytes each) |

  Total size: **12,385 bytes**.

- The delta proof payload uses a fixed 96-byte struct (no envelope, no VK lookup):

  | Offset | Size | Field | Notes |
  |--------|------|-------|-------|
  | 0 | 32 | `R` | Compressed Ristretto point `alpha·G + beta·H` |
  | 32 | 32 | `s_G` | Scalar encoding (`alpha + c·Δ`) |
  | 64 | 32 | `s_H` | Scalar encoding (`beta + c·Δ_blind`) |

  The challenge `c` is derived from `C_init`, `C_res`, the signed `delta_i128_le`, the 32-byte
  network binding (`Hash(chain_id)`), the difference `U = C_res - C_init`, and `R`. The verifier
  checks `s_G·G + s_H·H == R + c·U`.【crates/iroha_core/src/smartcontracts/isi/offline/balance_proof.rs:70】
- The range proof is a per-bit OR proof: each bit commitment is proven to be either `0·G` or `1·G`,
  and the sum of the bit commitments is shown to match `C_res - C_init - Δ·G`.
- `claimed_delta` and the committed values must use the allowance's canonical `Numeric` scale;
  scale mismatches are rejected in proof generation and verification. Receipt amounts and policy
  limits (`max_balance`, `max_tx_value`, allowance `amount`) must use the same scale; mismatches
  are rejected during registration and bundle admission. The canonical scale is the asset
  definition scale when specified; otherwise the allowance amount's scale is used.
- No verifying key is required: the transcript label and generator derivation are hard-coded, so
  nodes do not fetch or rotate any VK material. Proofs are rejected when commitments are not 32-byte
  Ristretto encodings or when `claimed_delta` uses the wrong scale.【crates/iroha_core/src/smartcontracts/isi/offline/balance_proof.rs:318】
- SDK helpers: the Swift SDK exposes `OfflineBalanceProofBuilder` (update commitment + generate
  proof), and the Android SDK exposes `OfflineBalanceProof`. Wallets pass the chain id, claimed
  delta, resulting value, and the current/randomized blinding seeds to obtain both the next
  commitment (`resulting_commitment`) and the versioned proof blob that Torii accepts.

#### Operational guardrails

- Ledger-reconcilable receipts are probabilistic until settled: the on-ledger commitment can move
  between receipt issuance and settlement, so merchants should reconcile quickly and surface
  pending status until Torii confirms. In offline-only circulation, receipts are final between
  participants and are not settled on-ledger.
- Shorten receipt windows by setting `refresh_at_ms` and attestation token TTLs (`max_token_age_ms`);
  bundles submitted after the refresh deadline are rejected.
- Hardware counters are enforced for attested paths; prefer rollback-resistant StrongBox/TEE keys
  for offline allowances and do not export offline spend keys across devices.
- Proof generation occurs in software; SDKs zeroize blinding buffers after proof generation, but
  compromised devices can still leak balance/blinding material. Treat rooted devices as
  privacy-compromised and rotate allowances accordingly.

### 4.2 Poseidon receipt tree & aggregate proofs *(OA13/OA14)*

The optional Chaum–Pedersen proof validates per-bundle balance deltas today. OA13/OA14 expand this
into a deterministic receipt tree and a trio of Poseidon-backed aggregate proofs so bundles can be
verified without replaying every receipt.

#### Poseidon parameters (shared with FASTPQ)

| Parameter | Value | Source |
|-----------|-------|--------|
| Field | Goldilocks `q = 2^64 - 2^32 + 1 = 0xffff_ffff_0000_0001` | `crates/fastpq_isi/src/poseidon.rs` |
| Width (`t`) | 3 | Same profile as FASTPQ Poseidon |
| Rate (`r`) | 2 |
| Capacity (`c`) | 1 |
| S-Box | `x ↦ x^5` |
| Full rounds (`R_F`) | 8 |
| Partial rounds (`R_P`) | 57 |
| Round constants/MDS | Snapshot published under `artifacts/offline_poseidon/constants.ron` |

Implementations must reuse the FASTPQ Poseidon engine (Metal/CUDA/CPU kernels already ship with
these parameters) to avoid duplicate code paths. The constant snapshot is versioned; any update
requires regenerating the golden vectors referenced below.

#### Leaf schema and field encoding

Each receipt contributes a canonical Norito struct:

```text
struct OfflineReceiptLeaf {
    tx_id: Hash,                  // BLAKE2b-256 already used by receipts
    amount: Numeric,              // mantissa + scale
    counter: u64,                 // monotonic hardware counter value
    receiver_hash: Hash,          // blake2b32(Norito(AccountId))
    invoice_hash: Hash,           // blake2b32(invoice bytes)
    platform_proof_hash: Hash,    // BLAKE2b-256 over OfflinePlatformProof bytes
}
```

Conversion rules:

1. Hash fields (`tx_id`, `invoice_id`, `platform_proof_hash`) are interpreted as big-endian integers
   < `q`. If a hash is ≥ `q`, subtract `q` until it falls inside the field (at most once for
   BLAKE2b-256 because `q` ≈ 2²⁵⁵).  
2. `amount` uses the Norito decimal encoding: `(mantissa: i128, scale: u32)`. Both components are
   reduced modulo `q` (two field elements).  
3. `counter` is converted to a field element via the standard little-endian encoding.  
4. `receiver_hash` stores `blake2b32(Norito(AccountId))` before field reduction, so ledger + SDK
   builders never reserialize the account inline.  
5. The Poseidon sponge absorbs the six field elements in the order listed above to produce
   `OfflineReceiptDigest`.

#### Merkle tree layout

- Binary Poseidon Merkle tree with domain tag `b"iroha.offline.receipt.merkle.v1"`.  
- Leaves are the digests described above.  
- Internal nodes hash `(left || right)` using the same Poseidon parameters; an empty subtree is
  represented by the zero field element.  
- The final root is stored as a `PoseidonDigest` (32-byte big-endian field encoding) inside
  `AggregateProofEnvelope`.  
- Deterministic builders must sort receipts by `(counter, tx_id)` before inserting them so wallets
  and nodes derive identical trees. Torii enforces the same ordering on submitted bundles.  
- Golden vectors (leaf inputs, intermediate hashes, tree roots) live under
  `artifacts/offline_poseidon/vectors.json`. Each entry records the leaf hashes and the resulting root
  so SDKs can cross-check their Poseidon implementations without depending on Rust fixtures.

#### Aggregate proof envelope

```text
struct AggregateProofEnvelope {
    version: u16,                       // 1 for the Poseidon/FASTPQ suite
    receipts_root: PoseidonDigest,      // Poseidon Merkle root (32 bytes)
    proof_sum: Option<Vec<u8>>,         // deterministic sum proof (OfflineFastpqSumProof bytes)
    proof_counter: Option<Vec<u8>>,     // deterministic counter proof (OfflineFastpqCounterProof)
    proof_replay: Option<Vec<u8>>,      // deterministic replay proof (OfflineFastpqReplayProof)
    metadata: Map<String, Value>,       // optional: parameter set + circuit identifiers
}
```

Ledger admission looks up the envelope version and dispatches the appropriate verifier set. During
the pilot, proofs remain optional (`settlement.offline.proof_mode = "optional"`). Once OA14
completes and operators flip `settlement.offline.proof_mode = "required"`, Torii will reject
bundles whose proofs are missing (`aggregate_proof_missing`) or invalid.

When available, include the FASTPQ identifiers in `metadata` (string values):

- `fastpq.parameter_set` — parameter-set label (for example, `"fastpq-offline-v1"`).  
- `fastpq.circuit.sum` — sum circuit identifier (for example, `"fastpq/offline_sum/v2"`).  
- `fastpq.circuit.counter` — counter circuit identifier (for example, `"fastpq/offline_counter/v2"`).  
- `fastpq.circuit.replay` — replay circuit identifier (for example, `"fastpq/offline_replay/v2"`).

#### Witness request payloads

`iroha_data_model::offline` now exposes Norito structs for the prover inputs:
`OfflineProofRequestHeader`, `OfflineProofBlindingSeed`, `OfflineProofRequestSum`,
`OfflineProofRequestCounter`, and `OfflineProofRequestReplay`. These payloads include the
Poseidon receipts root, balance commitments/delta, ordered receipt amounts, counter checkpoints,
and deterministic HKDF salts (`OFFLINE_FASTPQ_HKDF_DOMAIN = "iroha.offline.fastpq.v1"`) so SDKs and
host builders can stream identical witness bytes into the deterministic proof builder or the
FASTPQ APIs. Refer to `crates/iroha_data_model/src/offline/mod.rs` for the canonical schema, and use
`iroha offline transfer proof --bundle <PATH> --kind sum` (counter/replay variants supported via
`--kind {counter,replay}`) to dump the canonical Norito JSON for local bundle payloads. Torii
exposes the same payloads over HTTP via `POST /v1/offline/transfers/proof` and accepts a transfer
payload (`transfer`), returning the Norito JSON for the requested proof type.

For replay proofs, `replay_log_head_hex` / `replay_log_tail_hex` come from the receiver/POS
anti-replay log (Torii does not derive or return them). The receiver maintains a local hash chain
over accepted receipt `tx_id` values; use the stored head before this bundle as
`replay_log_head_hex`, then compute `replay_log_tail_hex` by hashing each bundle `tx_id` in receipt
order with `OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN` (`Hash::new(domain || head || tx_id)` iterated).

Example helper (same logic for Swift/Android/JS):

```text
func replayLogTail(head, txIds) -> Hash {
  var current = head
  for txId in txIds { // receipt order
    current = irohaHash(OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN || current || txId)
  }
  return current
}
```

#### Proof statements

- **Sum proof:** Witnesses receipt amounts/commitments and proves
  `Σ amount_i = claimed_delta` and `C_init - C_res = Pedersen(Σ amount_i, Σ blind_i)`, all bound to
  `receipts_root`.  
- **Counter proof:** Witnesses the ordered counter sequence and proves
  `c_0 = checkpoint + 1` and `c_k = c_{k-1} + 1` for every receipt while committing to the same
  `receipts_root`.  
- **Replay proof:** Witnesses the receiver’s anti-replay log and proves that hashing each `tx_id`
  into the previous log head yields the submitted tail value, preventing receipt omission.

Deterministic proofs (`OfflineFastpqSumProof`/`OfflineFastpqCounterProof`/`OfflineFastpqReplayProof`)
cover these statements today. FASTPQ circuits
(`prove_offline_sum`/`prove_offline_counter`/`prove_offline_replay`) can replace them as long as
they emit the same envelope formats. CPU + Metal/CUDA provers must stay within deterministic
resource envelopes defined in OA14.1; SDKs call the host through the FFI/bridge APIs.

#### Test vectors & validation

1. `artifacts/offline_poseidon/README.md` documents the constant snapshot and how to regenerate
   vectors via `cargo xtask offline-poseidon-fixtures`.  
2. `docs/source/offline_allowance.md` (this section) references the artifact version so auditors can
   reproduce hashes offline.  
3. Torii exposes `/v1/offline/bundle/proof_status` in OA14.3 to let operators check whether bundles
   carried proofs, their verification status, and the `receipts_root` they referenced without
   downloading the full payloads.

Ledger invariants enforced during `SubmitOfflineToOnlineTransfer`:

1. All receipts share the same sender certificate id, asset definition, receiver, and controller.
2. Receipts share a single counter scope (canonical App Attest key id, marker series, or provisioned scope).
3. Receipts are ordered by `(counter, tx_id)` to keep counter proofs and aggregates deterministic.
4. Every receipt amount is positive, uses the allowance scale, and stays within
   `policy.max_tx_value`.
5. Aggregate amount equals `balance_proof.claimed_delta`.
6. `record.current_commitment == balance_proof.initial_commitment`.
7. `record.remaining_amount >= claimed_delta`; remaining balance is decremented on success.
8. Platform counters advance exactly by one per receipt and never rewind.
9. If the bundle supplies `platform_snapshot` (either per receipt or at the bundle level), its
   `policy` must match the allowance metadata and `attestation_jws_b64` must be valid base64; those
   bytes are used to verify Play Integrity / HMS Safety Detect tokens instead of the certificate’s
   cached attestation report. Receipt-local snapshots take precedence so POS importers can attach
   distinct proofs per spend.

Ledger-reconcilable receipts remain probabilistic until settlement: device rollbacks, cloned keys,
or a sender racing an online refresh can still invalidate an otherwise well-formed receipt.
Merchants should settle quickly, consume counter summaries, and surface UX warnings for offline
acceptance flows; hardware counters mitigate but do not eliminate state-fork risk. In offline-only
circulation, receipts are final between participants and counter summaries are the primary defense
against duplicate `(certificate_id, counter)` claims.

## 5. Ledger Instructions

### 5.1 `RegisterOfflineAllowance`

1. Controller submits the operator-signed certificate (only the `controller` account may register it).
2. Ledger validates:
   - Positive allowance amount.
   - Certificate expiry > issued_at and policy coverage ≥ certificate lifetime.
   - `policy.max_balance >= allowance.amount` and `policy.max_tx_value <= max_balance`.
   - Operator signature matches the allowance asset owner.
3. Asset definitions intended for offline allowances must set metadata
   `offline.enabled = true`. On registration, the ledger derives a deterministic escrow account in
   the asset definition’s domain (seeded from `Hash("iroha.offline.escrow.v1|<chain_id>|<asset_def_id>")`)
   and ensures the account exists. The derived binding may be cached in
   `settlement.offline.escrow_accounts`, but the ledger will re-derive it from metadata if the map is empty.
4. If `settlement.offline.escrow_required=true`, ledger transfers the allowance amount from
   `allowance.asset.account()` into the escrow account for the asset definition. Missing
   `offline.enabled` (and no configured binding) rejects the registration.
5. Ledger stores an `OfflineAllowanceRecord` keyed by `certificate_id` and seeds
   `remaining_amount = allowance.amount`.

### 5.2 `SubmitOfflineToOnlineTransfer`

1. Deposit account must match the transaction authority.
2. All receipts undergo signature verification (`sender_signature` vs. spend key) and policy checks.
3. Platform proofs are validated (App Attest chain, KeyMint attestation, monotonic counters).
4. The resulting commitment and `remaining_amount` are updated atomically; the aggregate amount is
   deposited into `deposit_account` under the requested asset definition.
5. Bundle IDs are deduplicated to prevent replay (`offline_to_online_transfers` storage).

### 5.3 `RegisterOfflineVerdictRevocation`

1. Operator submits `RegisterOfflineVerdictRevocation { revocation }` where `revocation.verdict_id`
   matches the certificate being revoked.
2. Ledger validates:
   - `verdict_id` exists on an active allowance and has not been revoked already.
   - Caller matches the controller of the allowance (governance can extend this policy later).
3. Ledger writes the revocation entry (`issuer`, `revoked_at_ms`, `reason`, `note`, `metadata`)
   into `offline_verdict_revocations`. Subsequent CLI/HTTP queries will surface it so POS clients
   can refuse cached verdicts immediately.

## 6. Issuer Workflow (OA1)

1. **Select asset & policy.** Choose the operator-owned asset (e.g., `norito:01020304deadbeef`),
   target controller account, allowance amount, and policy limits (max balance / per-spend cap /
   expiry).
2. **Produce a commitment.** Sample `r_0` (32 random bytes) and compute
   `C_0 = Com(asset, amount, r_0)` using the Pedersen generators from §4.1.
   The tooling and ledger both derive `C_0 = amount·G + H(asset_id, r_0)` so
   offline fixtures and on-ledger verification are identical.
3. **Bundle device metadata.** Capture App Attest or KeyMint attestation bundles and record the
   required metadata keys listed in §2.2. Store the attestation report bytes alongside the spec
   (they are embedded in the certificate).
4. **Generate the spend key.** Wallets generate the spend keypair locally and store the private
   spend key on-device. Only the `spend_public_key` is sent to the issuer along with the draft
   certificate payload.
5. **Issue the certificate.** Serialize the `OfflineWalletCertificatePayload` (controller, allowance,
   spend key, attestation bytes, timestamps, policy, metadata) and sign it with the operator’s
   private key whose public key controls the allowance asset. This can be done with
   `POST /v1/offline/certificates/issue` (or the renewal variant) when
   `torii.offline_issuer.operator_private_key` is configured; the endpoint returns the signed
   certificate and its id.
6. **Register on-ledger.** Submit `RegisterOfflineAllowance { certificate }` from the controller’s
   account. After this step, Torii exposes the record via `iroha_cli offline allowance {list,get}`.

## 7. Tooling & Fixtures (`scripts/offline_topup/`)

The OA1 tooling lives under `scripts/offline_topup/`:

- `spec.example.json` — template describing operators, allowances, metadata, and attestation files.
- `run.sh` — thin wrapper around `cargo xtask offline-topup` so CI/scripts can invoke it without
  remembering the Cargo incantation.

### 7.1 Command overview

```
$ scripts/offline_topup/run.sh \
    --spec scripts/offline_topup/spec.example.json \
    --output artifacts/offline_demo \
    --register-config configs/offline-controller.toml
```

Key details:

- `--spec` accepts a JSON file with `{ "operator": { "private_key": "ed25519:…" }, "allowances": [...] }`.
  Each allowance entry supplies `label`, `controller`, `allowance_asset`, and policy fields using the
  canonical I105 account syntax (no `@domain`; runtime parsers reject `@domain` suffixes). Spend keys can be specified as a multihash (`ed0120…`)
  or as `algo:hex`, metadata can be provided inline or via `metadata_file`, attestation bytes can be
  embedded with `attestation_report_{hex,base64}` (or loaded from `attestation_report_file`), and an
  optional `blinding_hex` override keeps regression fixtures deterministic.
- `--output` chooses the root directory for generated artefacts. The tool writes one subdirectory per
  allowance with:
  - `certificate.json` / `certificate.norito`
  - `register_instruction.json` / `register_instruction.norito`
  - `summary.json` (`certificate_id`, commitment hex, asset/controller, delta) for SDK tests.
- `--register-config` is optional; when provided the tool loads the client TOML, instantiates
  `iroha::client::Client`, and submits `RegisterOfflineAllowance` (blocking by default). Use
  `--register-mode immediate` for fire-and-forget submission.

- Use `--operator-key ed25519:<hex>` to override the key embedded in the spec (useful when secrets
  are injected via CI rather than stored on disk).

See `scripts/offline_topup/README.md` for the full CLI and spec schema.

### 7.2 Canonical fixtures

- The repo now keeps deterministic fixtures under `fixtures/offline_allowance/`. Each allowance in
  `scripts/offline_topup/spec.example.json` renders a folder (`ios-demo`, `android-demo`,
  `android-hms-demo`, `android-play-demo`, `android-provisioned-demo`) with
  `certificate.*`, `register_instruction.*`, and `summary.json` outputs.
  (See `fixtures/offline_allowance/README.md` for the generation command and any temporary build
  blockers that still need to be resolved.)
- `fixtures/offline_allowance/allowance_fixtures.manifest.json` lists every fixture plus the relative
  file paths consumed by CI/tests. Regenerate it with:

  ```
  scripts/offline_topup/run.sh \
      --spec scripts/offline_topup/spec.example.json \
      --output fixtures/offline_allowance
  ```

  The example spec pins operator keys, blindings, and attestation placeholders so rerunning the
  command produces identical Norito bytes.
- The `android-hms-demo` sample mirrors the Safety Detect metadata from §2.2 and stores the canonical
  `attestation_nonce`/`refresh_at_ms` pair so downstream tooling can exercise the HMS policy without
  inventing dummy payloads.
- `android-play-demo` captures the Play Integrity metadata described in §2.3 (allowed app/device
  verdicts, project number, token TTL) so SDKs and Torii integrations can validate the new policy
  without crafting dummy JSON.
- `android-provisioned-demo` encodes the provisioned flow from §2.4, including the inspector public
  key, manifest schema/version, and maximum manifest age. Use it as the canonical reference when
  implementing kiosk tooling or verifying operator manifests.
- `crates/iroha_data_model/tests/offline_fixtures.rs` loads the manifest, decodes every
  `certificate.norito`, and asserts that the JSON mirrors the canonical Norito bytes (plus the HMS
  metadata contract). Keep this test green whenever fixtures are regenerated.

### 7.3 Fixtures in SDK tests

- Swift/Android SDKs can import the generated `summary.json` and `certificate.norito` to build
  deterministic regression cases without reimplementing the Norito encoders.
- `iroha_cli offline allowance list --verbose` now matches the JSON the tool emits, so test harnesses
  can diff server responses against the fixture folder.
- Rust clients can persist pending envelopes via `iroha::offline::OfflineJournal`, which implements
  the hash-chained WAL described in OA2 (`R_k = H(R_{k-1} \| tx_id_k)`) and authenticates every
  record with an HMAC key supplied by the operator.
- Swift SDKs share the exact same layout via `IrohaSwift.OfflineJournal`: each record encodes
  `[kind | timestamp | len | tx_id | payload | chain | hmac]` where `chain = BLAKE2b-256(prev_chain ||
  tx_id)` and `hmac = HMAC-SHA256(prev_chain || record_without_hmac)`. Android mirrors the same
  format through `OfflineJournal` so peers can move encrypted journals between devices without
  lossy conversions. Swift enforces streaming reloads with default caps (4096 records / 4 MiB) via
  `OfflineJournalConfiguration`, and the pipeline queue mirrors the guardrails through
  `FilePendingTransactionQueueConfiguration`; oversize or truncated journals fail fast instead of
  being loaded into memory.

### 7.4 POS provisioning bundles (`scripts/offline_pos_provision/`)

- OA12.a adds `cargo xtask offline-pos-provision` (wrapped by
  `scripts/offline_pos_provision/run.sh`) to turn provisioning specs into signed backend manifests.
  Specs declare an operator account/key plus a list of manifests; each manifest lists
  `{ label, manifest_id, sequence, published_at_ms, valid_{from,until}_ms, rotation_hint_ms?, roots[] }`.
- Each entry emits `manifest.json` / `manifest.norito` for the new
  `OfflinePosProvisionManifest` along with a `summary.json` describing the operator, rotation hint,
  and backend roots. The root output folder also gains `pos_provision_fixtures.manifest.json` so CI
  can diff artefacts deterministically.
- Metadata and key material can be provided inline or via external files (`metadata_file`,
  `public_key_file`). Inline metadata is copied straight into the manifest `Metadata` map so POS
  toolchains can surface jurisdiction/channel tags without bespoke wiring.
- Optional `revocation_bundles` entries let operators package deny-lists for USB/QR drops. Each
  bundle references a `revocations` array (or a `revocations_file` such as
  `scripts/offline_pos_provision/revocations.example.json`) and the helper emits a signed
  `OfflineVerdictRevocationBundle` (`revocations.json`, `revocations.norito`, and `summary.json`)
  that POS devices can verify offline. The top-level fixture manifest records these bundles under
  `revocation_bundles` so CI can diff both manifest rotations and revocation exports.
- See `scripts/offline_pos_provision/README.md` and the accompanying `spec.example.json` for the
  schema/CLI overview; manifests decode via
  `OfflinePosProvisionManifest`/`OfflinePosBackendRoot` (`crates/iroha_data_model/src/offline/mod.rs`)
  and the helper keeps Norito + JSON bytes in lockstep.
- POS clients can exercise the same hardening rules via
  `cargo xtask offline-pos-verify`, supplying provisioning manifests, allowance summaries, and a
  policy file (`scripts/offline_pos_provision/policy.example.json`) that pins backend roots,
  enforces verdict refresh TTLs/nonces, and records audit entries whenever manifests rotate. The
  verifier exits with a descriptive error when a policy guard is violated so CI and staging kiosks
  can block regressions before distributing new bundles.
- For key pinning & rotation drills, follow the [Offline POS Key Pinning runbook](offline_pos_key_pinning.md)
  so manifests, deny-lists, and rehearsal evidence stay aligned with OA12 expectations.

### 7.5 Inspector kiosk proofs (`scripts/offline_provision/`)

- OA10.3 kiosks now have a deterministic helper:
  ```
  cargo xtask offline-provision \
      --spec scripts/offline_provision/spec.example.json \
      --output fixtures/offline_provision
  ```
  The command parses receipt challenges + device manifests and emits `AndroidProvisionedProof`
  artefacts (`proof.json`, `proof.norito`) plus a `provision_fixtures.manifest.json` index so
  QA/SDK suites can replay the inspector workflow deterministically.
- Spec fields:
  * `inspector.private_key` – default Ed25519 key used to sign manifests. Override per-proof via
    `inspector_key` or the CLI `--inspector-key` flag.
  * `inspector.manifest_schema` / `.manifest_version` – default values applied when a proof
    omits them.
  * `proofs[].manifest_issued_at_ms` + `proofs[].counter` – tie the kiosk timestamp and
    `{schema}::{device_id}` counter that Torii enforces.
  * `proofs[].device_manifest` (or `_file`) – metadata inserted verbatim into the proof. Must
    include `android.provisioned.device_id`; otherwise the helper aborts.
  * `proofs[].challenge` (or `_file`) – serialized `OfflineReceiptChallengePreimage` describing
    the invoice, receiver, asset, amount, and nonce used when deriving the attestation hash.
  See `scripts/offline_provision/README.md` for the full schema and a working sample.
- The tool serializes the manifest, hashes it (for optional digest comparisons), and signs
  `AndroidProvisionedProof.signing_bytes()` with the inspector key so the output matches the Norito
  layout used by runtime verification.
- `scripts/offline_topup/spec.example.json` wires the demo allowance via the new
  `provisioned_proof_file` field (paths are relative to the spec, hence the `../../` prefix when
  referencing repo-root fixtures). Run `cargo xtask offline-provision` before
  `cargo xtask offline-topup` so the top-up spec can load the inspector proof and embed the
  resulting bytes as the certificate's `attestation_report`.
- **SDK parity:** Swift ships `AndroidProvisionedProof` and Android mirrors it via
  `org.hyperledger.iroha.android.offline.AndroidProvisionedProof`, letting POS/KDS apps load the
  `proof.json` artefacts emitted by `cargo xtask offline-provision`, verify canonical hash/signature
  formatting, and re-encode the manifest deterministically when staging allowances or audit bundles.
  Both helpers surface the parsed device-id, canonical hash literal, and inspector signature to keep
  OA10.3 flows aligned with the Norito schema.【IrohaSwift/Sources/IrohaSwift/AndroidProvisionedProof.swift:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/AndroidProvisionedProof.java:1】

## 8. Torii Offline Query Surfaces (OA7.2)

Torii ships dedicated offline APIs for both inspection and wallet flows. The handlers live under
`/v1/offline/*`, enforce the usual `app_api` gates, and reuse the shared Norito filter DSL so SDKs,
CLI tools, and dashboards stay in sync with the roadmap requirements.

| Method | Path | Purpose |
|--------|------|---------|
| `GET`  | `/v1/offline/allowances` | Lightweight pagination over registered allowances. |
| `POST` | `/v1/offline/allowances` | Register an allowance certificate (submits `RegisterOfflineAllowance`). |
| `GET`  | `/v1/offline/allowances/{certificate_id_hex}` | Fetch a single allowance record. |
| `POST` | `/v1/offline/allowances/{certificate_id_hex}/renew` | Renew an allowance certificate. |
| `POST` | `/v1/offline/allowances/query` | Full `QueryEnvelope` (filter + sort + selector + pagination). |
| `GET`  | `/v1/offline/certificates` | Alias for `/v1/offline/allowances`. |
| `POST` | `/v1/offline/certificates` | Alias for `/v1/offline/allowances` issuance. |
| `POST` | `/v1/offline/certificates/issue` | Issue an operator-signed certificate without registering it on-ledger. |
| `POST` | `/v1/offline/certificates/query` | Alias for `/v1/offline/allowances/query`. |
| `GET`  | `/v1/offline/certificates/{certificate_id_hex}` | Alias for `/v1/offline/allowances/{certificate_id_hex}`. |
| `POST` | `/v1/offline/certificates/{certificate_id_hex}/renew` | Alias for allowance renewal. |
| `POST` | `/v1/offline/certificates/{certificate_id_hex}/renew/issue` | Issue a renewed certificate after confirming the allowance exists. |
| `POST` | `/v1/offline/certificates/revoke` | Register a verdict revocation for a certificate. |
| `GET`  | `/v1/offline/transfers` | Paginate bundles awaiting ledger admission. |
| `GET`  | `/v1/offline/transfers/{bundle_id_hex}` | Fetch a bundle summary by id. |
| `POST` | `/v1/offline/transfers/query` | Envelope-based queries for bundles. |
| `GET`  | `/v1/offline/settlements` | Alias for `/v1/offline/transfers`. |
| `POST` | `/v1/offline/settlements` | Submit a bundle for on-ledger settlement (submits `SubmitOfflineToOnlineTransfer`). |
| `GET`  | `/v1/offline/settlements/{bundle_id_hex}` | Alias bundle detail endpoint. |
| `POST` | `/v1/offline/settlements/query` | Alias bundle query endpoint. |
| `POST` | `/v1/offline/transfers/proof` | Build `{sum,counter,replay}` witness payloads from a transfer payload. |
| `GET`  | `/v1/offline/receipts` | Audit list of flattened receipts extracted from settled bundles. |
| `POST` | `/v1/offline/receipts/query` | Envelope-based queries for receipts. |
| `POST` | `/v1/offline/spend-receipts` | Validate receipts and return their Poseidon `receipts_root`. |
| `GET`  | `/v1/offline/state` | Full offline state snapshot for wallet sync. |
| `GET`  | `/v1/offline/summaries` | List hardware counter summaries. |
| `POST` | `/v1/offline/summaries/query` | Envelope-based queries for counter summaries. |
| `GET`  | `/v1/offline/revocations` | List registered verdict revocations. |
| `POST` | `/v1/offline/revocations/query` | Envelope-based queries for revocations. |
| `GET`  | `/v1/offline/bundle/proof_status` | Lightweight proof status for a bundle. |
| `GET`  | `/v1/offline/rejections` | Aggregated per-platform rejection counters (requires telemetry). |

Top-ups are a two-step flow: issue a certificate with
`/v1/offline/certificates/issue` (or the renewal variant) and then register it
via `/v1/offline/allowances` (or `/v1/offline/allowances/{certificate_id_hex}/renew`).
There is no single “top-up” endpoint; SDK helpers simply chain the two calls
and verify the certificate ids match.

When build-claim verification is enabled and `torii.offline_issuer` is
configured, `/v1/offline/settlements` auto-issues missing receipt build claims
before submitting the transaction. Use `build_claim_overrides[]` on the
settlement request for per-receipt overrides (`app_id`, `build_number`, or
claim window), and set `repair_existing_build_claims=true` to re-issue already
present claims.

Android certificates may whitelist multiple package names (for example Play
Integrity or marker-key metadata with more than one entry). In that case Torii
cannot infer a single `app_id`; callers must provide `build_claim_overrides[]`
for the affected receipt `tx_id_hex`.

Issuer endpoints (`/v1/offline/certificates/*/issue`) are only enabled when
`torii.offline_issuer` is configured. Use `torii.offline_issuer.allowed_controllers` to restrict
which controller accounts may request new certificates.

When a request fails due to an offline validation rejection, Torii surfaces a stable error code in
the `x-iroha-reject-code` response header (for example `certificate_expired`, `counter_conflict`,
`max_tx_value_exceeded`, `allowance_exceeded`, `invoice_duplicate`).

`/v1/offline/transfers/proof` accepts a Norito/JSON body with a `transfer` payload plus proof
parameters. The `kind` field selects the FASTPQ proof (`sum`, `counter`, or `replay`), optional
`counter_checkpoint` overrides the inferred checkpoint (default = first receipt counter `- 1`), and
replay proofs must provide both log hashes. The response mirrors the `OfflineProofRequest*` structs
so SDKs can pass the payloads directly to the prover.

Example request/response:

```json
POST /v1/offline/transfers/proof
{
  "transfer": {
    "bundle_id": "11F7...C0DE",
    "receiver": "i105...",
    "deposit_account": "i105...",
    "receipts": [/* OfflineSpendReceipt */],
    "balance_proof": { /* OfflineBalanceProof */ }
  },
  "kind": "counter",
  "counter_checkpoint": 4095
}
```

```json
{
  "header": {
    "version": 1,
    "bundle_id": "11F7...C0DE",
    "certificate_id": "AA7E...42B9",
    "receipts_root": "0000000000000000000000000000000000000000000000001E4538501EEF1488"
  },
  "counter_checkpoint": 4095,
  "counters": [4096, 4097, 4098]
}
```

CLI equivalent:

```bash
iroha offline transfer proof \
  --bundle ./bundle.json \
  --kind counter \
  --counter-checkpoint 4095
```

### 8.1 GET parameters (`ListFilterParams`)

All GET surfaces accept the compact `ListFilterParams` struct via query parameters:

- `filter` — JSON-encoded `FilterExpr`. URL-encode the JSON blob; the parser accepts the same
  `{"op":"eq","args":["field","value"]}` form documented in
  `docs/source/torii/sumeragi_evidence_app_api.md`.
- `limit` / `offset` — pagination knobs (default `offset = 0`; `limit` falls back to the handler cap).
- `sort` — comma-separated `field[:asc|:desc]` entries (default `registered_at_ms:desc` for
  allowances and `bundle_id_hex:asc` for transfers).
- Account literals are rendered as canonical I105; Torii no longer accepts an `canonical I105 rendering` selector on this endpoint.
  response accordingly.

`/v1/offline/allowances` additionally exposes roadmap-driven convenience filters so dashboards and
SDKs can avoid building JSON predicates for the common expiry/verdict workflows:

- `controller_id` — filter allowances by controller account. Torii accepts only canonical I105 account literals or
  raw public-key literals and canonicalises them to the Global form before evaluating the query, so
  operators can feed whichever encoding their tooling surfaces.
- `asset_id` — filter allowances by asset identifier.
- `certificate_expires_before_ms` / `certificate_expires_after_ms` — constrain the certificate
  expiry timestamp (`<=`/`>=` respectively).
- `policy_expires_before_ms` / `policy_expires_after_ms` — constrain the policy expiry timestamp.
- `refresh_before_ms` / `refresh_after_ms` — filter by the attestation refresh deadline (`refresh_at_ms`);
  allowances missing refresh metadata are excluded when these filters are supplied.
- `verdict_id_hex` — case-insensitive verdict identifier match.
- `attestation_nonce_hex` — case-insensitive attestation nonce match.
- `require_verdict` / `only_missing_verdict` — include only allowances that do or do not carry
  verdict metadata (mutually exclusive; specifying `verdict_id_hex` implicitly requires a verdict).
- `include_expired=true` — return expired allowances (including those whose `refresh_at_ms` has
  elapsed). By default the handler mirrors `iroha_cli offline allowance list` and skips allowances
  whose certificate, policy, or refresh deadlines have passed.

`/v1/offline/transfers` exposes a parallel set of filters so bundle reviewers can scope results by
the same certificate metadata without composing JSON:

- `controller_id` / `receiver_id` / `deposit_account_id` — filter bundles by the originating
  controller, receiver, or deposit account. Each parameter accepts only canonical I105 account literals or raw
  public-key literals and Torii normalises the value to the canonical on-ledger form before matching.
- `asset_id` — filter bundles by the asset identifier recorded in the transfer commitment.
- `certificate_id_hex` — case-insensitive certificate identifier match.
- `certificate_expires_before_ms` / `certificate_expires_after_ms` — filter by the underlying
  certificate expiry window.
- `policy_expires_before_ms` / `policy_expires_after_ms` — filter by the wallet policy expiry.
- `refresh_before_ms` / `refresh_after_ms` — constrain the attestation refresh deadline.
- `verdict_id_hex` / `attestation_nonce_hex` — match verdict IDs or nonces (case-insensitive).
- `require_verdict` / `only_missing_verdict` — include only bundles that do/do not carry verdict
  metadata (mutually exclusive; supplying `verdict_id_hex` implies `require_verdict`).
- These certificate/verdict controls are exposed via query parameters on the GET endpoint; the JSON
  filter DSL remains limited to the base bundle metadata (controller/receiver/status/asset).

Allowed filter keys differ by resource:

| Resource | Filter keys | Operators |
|----------|-------------|-----------|
| Allowances | `certificate_id_hex`, `controller_id`, `asset_id` | `eq`, `ne`, `in`, `nin`, `exists` |
| Allowances | `registered_at_ms`, `certificate_expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms` | `eq`, `ne`, `lt`, `lte`, `gt`, `gte`, `exists` |
| Allowances | `verdict_id_hex`, `attestation_nonce_hex` | `eq`, `ne`, `in`, `nin`, `exists` |
| Transfers | `bundle_id_hex`, `controller_id`, `receiver_id`, `deposit_account_id`, `asset_id`, `status` | `eq`, `ne`, `in`, `nin`, `exists` |

Unsupported fields/operators return `422` (`ValidationFail::TooComplex`). Sort keys are limited to
`certificate_id_hex`, `controller_id`, `asset_id`, `registered_at_ms`, and `allowance_amount` for
allowances and `bundle_id_hex`, `receiver_id`, `deposit_account_id`, `asset_id`, `receipt_count`,
and `total_amount` for transfers.

Allowance responses now surface all attestation metadata inline so SDKs and dashboards do not have to
drill into the embedded record: `verdict_id_hex`, `attestation_nonce_hex`, `refresh_at_ms`,
`expires_at_ms`, `policy_expires_at_ms`, and `remaining_amount` sit alongside the existing
`certificate_id_hex`/`controller_id` columns. Transfer payloads now add the same certificate
metadata—`certificate_id_hex`, `certificate_expires_at_ms`, `policy_expires_at_ms`,
`refresh_at_ms`, `verdict_id_hex`, and `attestation_nonce_hex`—on top of the existing
`controller_id`, `controller_display`, `status`, and `recorded_at_{ms,height}` +
`archived_at_height` telemetry so operators can correlate bundles with ledger heights directly.
Each allowance entry now also advertises countdown metadata:

- `deadline_kind` — which deadline is currently tracked (`refresh`, `policy`, or `certificate`).
- `deadline_state` — `ok` when outside the alert window, `warning` when the deadline is within
  24 hours, and `expired` once the timestamp has passed.
- `deadline_ms` — the unix millisecond timestamp for the selected deadline.
- `deadline_ms_remaining` — signed milliseconds remaining (negative when overdue).

Clients can use these fields to drive countdown badges and UX prompts without recomputing the
deadline precedence (refresh → policy → certificate). The 24-hour warning window mirrors
`iroha_cli offline allowance list --summary` so dashboards, SDKs, and scripts surface consistent
notices.

Example GET query filtering allowances by controller:

```sh
FILTER=$(python3 - <<'PY'
import json, urllib.parse
expr = {"op": "eq", "args": ["controller_id", "i105..."]}
print(urllib.parse.quote(json.dumps(expr)))
PY
)
curl -s "$TORII/v1/offline/allowances?limit=25&sort=registered_at_ms:desc&filter=$FILTER" \
  -H "Authorization: Bearer $TOKEN" | jq .
```

### 8.2 POST envelopes (`QueryEnvelope`)

`POST /v1/offline/*/query` accepts a Norito/JSON `QueryEnvelope` for collection endpoints
(`allowances`/`certificates`, `transfers`/`settlements`, `receipts`, `summaries`, `revocations`) with
richer controls:

- `query` — optional label for audit logs.
- `filter` — full boolean tree (`and`/`or`/`not`) using the keys above.
- `select` — projection list (e.g., `["record.controller","record.policy"]`).
- `sort` — array of `{ "key": "registered_at_ms", "order": "desc" }` objects.
- `pagination` — `{ "limit": 100, "offset": 0 }`.
- `fetch_size` — optional iterator batch size (`null` = server default).
- Request envelopes no longer accept an `canonical I105 rendering` field; canonical I105 output is always returned.

Sample envelope and invocation:

```json5
{
  "query": "merchant-allowances",
  "filter": {
    "op": "and",
    "args": [
      {"op": "eq", "args": ["controller_id", "i105..."]},
      {"op": "gte", "args": ["registered_at_ms", 1733500000000]}
    ]
  },
  "sort": [{"key": "registered_at_ms", "order": "desc"}],
  "pagination": {"limit": 100, "offset": 0},
}
```

Save the envelope above as `envelope.json` and call:

```sh
curl -s -X POST "$TORII/v1/offline/allowances/query" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  --data @envelope.json | jq '.items[0]'
```

Responses always follow `{ "items": [...], "total": <u64> }`. Allowance items expose
`certificate_id_hex`, `controller`, `asset_id`, `registered_at_ms`, and the canonical
`OfflineAllowanceRecord` under `record`. Transfer items include `bundle_id_hex`, `receiver`,
`deposit_account`, `asset_id`, `receipt_count`, `total_amount`, and the embedded
`OfflineToOnlineTransfer`.

### 8.3 SDK & CLI hooks

- `OfflineToriiClient` and `OfflineListParams` (Android) plus the equivalent Swift/Rust helpers call
  these endpoints directly, so OA7 operators can page allowances/bundles without reimplementing HTTP
  plumbing.
- `iroha_cli offline allowance|transfer list` mirrors the same filters/sorts via the gRPC query
  builders, letting operators compare CLI output with the REST response body.
- `iroha_cli offline allowance list` now accepts the same verdict/expiry knobs exposed over HTTP
  (`--certificate-expires-before-ms`, `--policy-expires-after-ms`, `--verdict-id`, etc.), so
  operators can script audits without composing JSON predicates by hand.
- `iroha_cli offline transfer list` gained matching certificate/verdict filters
  (`--certificate-id`, `--certificate-expires-before-ms`, `--verdict-id`, `--require-verdict`,
  `--only-missing-verdict`, etc.) plus a `--summary` mode that prints the derived metadata (`bundle`,
  `status`, certificate expiry/policy deadlines, verdict identifiers) alongside the underlying
  ledger timestamps.
- `iroha_cli offline bundle inspect <bundle.json> [--proofs]` now loads offline bundle fixtures in
  either Norito (`.norito`/`.to`) or JSON form, recomputes the Poseidon receipts root, compares it
  against any embedded `AggregateProofEnvelope`, and optionally (`--proofs`) prints proof byte
  counts and metadata keys. This gives auditors a deterministic way to validate bundle evidence
  before it is staged back to Torii or attached to regulator reports. Example:

  ```sh
  iroha offline bundle inspect artifacts/offline_bundle/sample.json --proofs
  ```
- `scripts/offline_bundle/run.sh --spec <path>` feeds `cargo xtask offline-bundle` and emits
  fully-populated `OfflineToOnlineTransfer` fixtures (both JSON + Norito) alongside the three FASTPQ
  witness requests. Specs live next to the helper (`scripts/offline_bundle/spec.example.json`)
  and accept either canonical `hash:` literals or raw hex for identifiers. Each bundle is written
  into `artifacts/offline_bundle/<label>/` with a `bundle_summary.json` manifest so automation can
  capture the Poseidon root, witness filenames, and aggregate-proof metadata in a single place.
- `iroha offline rejection stats [--telemetry-profile=<label>]` hits
  `/v1/offline/rejections`, normalises the `{platform,reason,count}` tuples, and prints the JSON
  response so operators and CI jobs can consume the same telemetry that powers dashboards. Passing a
  telemetry profile forwards the `X-Torii-Telemetry-Profile` header to target a specific tenant; the
  command exits with a friendly message when the profile does not expose rejection metrics.
- HTTP clients calling `/v1/offline/transfers{,/query}` can supply `platform_policy=play_integrity`
  or `platform_policy=hms_safety_detect` to receive only the bundles backed by those attestation
  providers, mirroring the CLI/SDK convenience flag.【crates/iroha_torii/src/openapi.rs:540】【crates/iroha_torii/src/routing.rs:19321】
- `iroha_cli offline transfer list` also accepts `--platform-policy {play-integrity,hms-safety-detect}`
  so operators can filter summary/event dumps down to the attestation provider backing each bundle
  without post-processing the output.【crates/iroha_cli/src/offline.rs:330】【crates/iroha_cli/src/offline.rs:780】
- Transfer responses now embed `verdict_snapshot` (certificate id, verdict id/nonce, refresh/certificate/policy expiries) so SDKs/POS devices can cache the attestation metadata that was in effect when the bundle settled, even if the certificate later rotates.【crates/iroha_data_model/src/offline/mod.rs:1691】【crates/iroha_cli/src/offline.rs:592】【crates/iroha_torii/src/routing.rs:25398】
- `platform_token_snapshot` captures the Play Integrity / HMS Safety Detect attestation token (base64 JWS) used during settlement, giving auditors and POS devices an immutable copy of the token bytes the ledger verified.【crates/iroha_data_model/src/offline/mod.rs:1691】【crates/iroha_cli/src/offline.rs:592】【crates/iroha_torii/src/routing.rs:25398】
- Torii event subscribers can now filter directly on the attestation provider by calling `OfflineTransferEventFilter::for_platform_policy(...)`, which only emits `OfflineTransferEvent::Settled` payloads whose `platform_token_snapshot` matches the requested `AndroidIntegrityPolicy`. The `/v1/events/sse` endpoint exposes the same selector via `?filter={"op":"eq","args":["platform_policy","play_integrity"]}` so webhooks, SDKs, and CLI watchers can stick to a single filter grammar.【crates/iroha_data_model/src/events/data/filters.rs:580】【crates/iroha_torii/src/routing.rs:18735】
- Validators reject bundles whose cached verdicts are stale: `SubmitOfflineToOnlineTransfer` now enforces the `refresh_at_ms` deadline from either the allowance record or certificate, returning `VerdictExpired` when wallets fail to fetch fresh attestations before reconciling.【crates/iroha_core/src/smartcontracts/isi/offline.rs:767】【crates/iroha_data_model/src/offline/mod.rs:1643】
- The new `iroha_offline_attestation_policy_total{policy}` metric records how often each Android integrity policy is exercised so dashboards/alerts can spot Safety Detect or Play Integrity drift before it affects settlement.【crates/iroha_telemetry/src/metrics.rs:4794】
- Metrics labelled `torii.offline_allowances.*`/`torii.offline_transfers.*` record per-endpoint
  access along with the canonical I105 rendering, giving observability parity across SDKs.
- Every transfer record now surfaces a `status_transitions` array derived from `OfflineTransferRecord.history`, so CLI/REST callers can see precisely when a bundle settled, hit the hot-retention cutoff, and was eventually pruned from storage.【crates/iroha_data_model/src/offline/mod.rs:1684】【crates/iroha_cli/src/offline.rs:579】【crates/iroha_torii/src/routing.rs:25398】
- Each `status_transitions` entry now carries the same `verdict_snapshot` that Torii captured at settlement, giving POS importers and SDKs the certificate/verdict metadata aligned to each lifecycle event without scraping the top-level record.【crates/iroha_data_model/src/offline/mod.rs:1684】【crates/iroha_torii/src/routing.rs:26370】【crates/iroha_cli/src/offline.rs:862】
- Retention is governed by `settlement.offline.hot_retention_blocks` (archive window) and the new `settlement.offline.cold_retention_blocks` + `prune_batch_size` knobs; once the cold window elapses validators remove the archived bundle, increment `iroha_offline_transfer_pruned_total`, and emit an `OfflineTransferEvent::Pruned` payload so operators can archive the record before deletion.【crates/iroha_config/src/parameters/user.rs:3997】【crates/iroha_core/src/smartcontracts/isi/offline.rs:1111】【crates/iroha_telemetry/src/metrics.rs:4794】

## 9. Audit Logging & Jurisdictional Guidance (OA5)

OA5 calls for deterministic audit artefacts whenever allowances circulate, even before `OA4`
admission lands. Swift and Android already expose `OfflineAuditLogger`/`OfflineWallet` toggles, but
operators still need a single playbook describing when to enable logging, how to export bundles,
what privacy copy to display, and how long to retain data in each jurisdiction.

### 9.1 Platform toggles

| Platform | Toggle entry points | Default storage path | Notes |
|----------|--------------------|----------------------|-------|
| Swift | `OfflineWallet(auditLoggingEnabled:auditStorageURL:)`, `setAuditLoggingEnabled(_:)`, `fetchTransfersWithAudit` and `recordTransferAudit` (`IrohaSwift/Sources/IrohaSwift/OfflineWallet.swift`) | `Documents/offline_audit_log.json` (`OfflineAuditLogger.defaultStorageURL()`) | Files inherit the app’s NSFileProtection policy; point `auditStorageURL` at an `Application Support` subdirectory when devices require `NSFileProtectionCompleteUnlessOpen`. |
| Android | `OfflineWallet` factory on `OfflineToriiClient`, `setAuditLoggingEnabled(boolean)`, `fetchTransfersWithAudit`, `recordTransferAudit` (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineWallet.java`) | `Context.getFilesDir()/offline_audit.json` in the sample; pass an `EncryptedFile`-backed path when the jurisdiction mandates encryption at rest. | The logger writes a canonical JSON array; use `OfflineAuditLogger.exportJson()` to stream bytes directly into evidence bundles. |
| Headless/CI | Reuse the SDKs via Swift Package / JVM bindings or call `iroha_cli offline transfer list --json --audit-log <path>` to emit the same canonical log without extra tooling. | Operator-chosen | Keep the exported JSON alongside the CLI summary so `cargo xtask sorafs-adoption-check` and auditor notebooks can diff hashes. |

### 9.2 Export & escrow workflow

1. **Capture transfers with audit enabled.** Call `fetchTransfersWithAudit` (Swift/Android) or loop
   over the REST response and invoke `recordTransferAudit` manually when a custom sorter is needed.
2. **Export deterministic JSON.** Swift returns `[OfflineAuditEntry]` via `exportEntries()` or a
   pretty-printed `Data` blob via `exportJSON()`. Android exposes `wallet.exportAuditJson()`. Both
   emit `{tx_id,sender_id,receiver_id,asset_id,amount,timestamp_ms}` in the same field order.
3. **Attach digests to the release ticket.** Run `blake3 offline_audit_log.json >
   offline_audit_log.json.b3` (or `shasum -a 256`) and stash the digest next to the JSON. The digest
   plus the raw file must accompany any regulator request or OA5 compliance review.
4. **Escrow sensitive exports.** Upload the JSON/digest pair to the organisation’s encrypted
   evidence bucket (for example, the `artifacts/offline_audit/<YYYY-MM>` hierarchy referenced in the
   treasury runbook) and record the URI inside the change ticket.

### 9.3 Retention defaults & purge checklist

| Circulation mode | Default retention window | Required actions before purge |
|------------------|-------------------------|-------------------------------|
| Ledger reconcilable (`CirculationMode.LEDGER_RECONCILABLE`) | ≥13 months or 90 days after the final deposit tied to the bundle (whichever is longer). | Confirm the allowance has been reconciled on-ledger, attach the settlement height to the ticket, export one final digest, and document the purge in the compliance log. |
| Offline-only (`CirculationMode.OFFLINE_ONLY`) | Lifetime of the circulating float + 5 years (matches most AML programmes). | Verify treasury sign-off for the bearer window, export the JSON/digest pair to cold storage, reference the approval ID, then call `clear()` / `clearAuditLog()` to wipe local caches. |

Never disable logging while outstanding allowances exist. If a jurisdiction forbids storage, record
the legal citation, the incident/change ticket, and the compensating controls (for example, a Torii
query replay cadence) before flipping the toggle.

### 9.4 Privacy notices & change control

- **Ledger reconcilable copy:** “Offline receipts are logged locally and reconciled on-ledger within
  the policy window. Logs contain sender/receiver/account/amount/timestamp and remain on this device
  for at least 13 months for regulator reviews.”
- **Offline-only copy:** “This wallet treats allowances as bearer value. Recovery depends on your
  local audit journal; if you delete it, the funds cannot re-enter the ledger.”

Swift exposes `OfflineWalletCirculationMode` notices and Android mirrors them via
`OfflineWallet.CirculationMode`/`ModeChangeListener`. Surface the appropriate notice whenever the
user or operator flips the toggle, and require a change ticket ID + approval before calling
`setAuditLoggingEnabled(false)`.

### 9.5 Operational checklist

- Enable audit logging before minting OA1 allowances; store the toggle state in the change record.
- Schedule a weekly (or per-jurisdiction) `fetchTransfersWithAudit` run and ship the new digest to
  the compliance bucket.
- When exporting evidence for regulators, bundle the Torii `/v1/offline/transfers` JSON,
  the audit-log JSON, and the digest in the same artefact.
- Automate drift detection by hashing the local log daily and alert when the digest diverges from
  the escrow copy (protects against silent truncation).

### 9.6 Deterministic WAL & SDK export toggles

- **Deterministic WAL:** `iroha::offline::OfflineJournal` provides the hash-chained WAL required by
  OA2/OA5, exposes append + commit APIs, and enforces integrity/HMAC verification on every open so
  auditors can re-run the chain check before accepting exported logs.【crates/iroha/src/offline/mod.rs:1】
- **Swift SDK:** `OfflineAuditLogger` persists `{sender, receiver, asset, amount, timestamp}` records
  under `Documents/offline_audit_log.json`, exposes enable/disable toggles, and supports
  export/clear flows so jurisdictions that forbid audit logging can flip the knob at runtime while
  still producing deterministic JSON when enabled.【IrohaSwift/Sources/IrohaSwift/OfflineAuditLogger.swift:31】
- **Android SDK:** `OfflineAuditLogger` and `OfflineWallet` mirror the same toggles, keep the logs in
  JSON, and integrate with the Torii helpers so deposit submissions automatically include the
  relevant audit context.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineAuditLogger.java:19】
- **Operator workflow:** Support teams can harvest the WAL/audit exports by copying the journal +
  logger files off the device (or by using the SDK-provided export helpers), then attach them to the
  Torii transcripts for regulators. Because the fields are ordered deterministically, diffs survive
  air-gapped review.

## 10. Allowance Registry & Telemetry (OA7)

- **On-ledger registry.** `OfflineAllowanceRecord` and `OfflineTransferRecord` live under the world
  state registries (`world.offline_allowances()`, `world.offline_to_online_transfers()`), and the
  queries `FindOfflineAllowances`, `FindOfflineAllowanceByCertificateId`,
  `FindOfflineToOnlineTransfers`, and `FindOfflineCounterSummaries` are wired through both Torii and
  CLI so operators can inspect commitments, remaining balances, and counter checkpoints without
  bespoke scripts.【crates/iroha_core/src/smartcontracts/isi/offline.rs:888】
- **CLI workflow.** `iroha offline allowance list|get`, `iroha offline transfer list|get`, and the
  new `iroha offline summary list|export` subcommands reuse the standard pagination/sorting flags and
  emit either IDs (non-verbose) or full records, keeping operator runbooks aligned with OA7.1.【crates/iroha_cli/src/offline.rs:29】
  - The `--summary` flag on `iroha offline allowance list` now emits per-certificate rows with
    `expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
    `attestation_nonce_hex`, `remaining_amount`, and the countdown metadata
    (`deadline_kind`, `deadline_state`, `deadline_ms`, `deadline_ms_remaining`) so auditors can
    inspect attestation health without toggling verbose mode.
  - Expiry-aware flags (`--certificate-expires-before-ms/--certificate-expires-after-ms` and
    `--policy-expires-before-ms/--policy-expires-after-ms`) complement the existing `--verdict-id`,
    `--attestation-nonce`, and `--refresh-*-ms` switches so support teams can zero in on allowances
    that are about to lapse or already outside the acceptable policy window during audits.
- **REST + queries.** Torii exposes `/v1/offline/*`, including list + `/query` endpoints for
  `allowances`/`certificates`, `transfers`/`settlements`, `receipts`, `summaries`, and `revocations`,
  so SDKs and support tools can page/filter over the registry via HTTP using the same filter spec as
  other collections.【crates/iroha_torii/src/routing.rs:23200】【crates/iroha_torii/src/routing.rs:23442】
- **Telemetry.** `find_offline_allowances` and `find_offline_to_online_transfers` counters (plus the
  Torii request metrics) increment on every query, so dashboards already reflect allowance registry
  usage across OA7 deployments.
- **Bundle filters.** The ledger now maintains sender/receiver/status indexes for every
  `OfflineTransferRecord`, enabling the new
  `FindOfflineToOnlineTransfersBy{Controller,Receiver,Status}` queries and corresponding CLI flags
  (`iroha offline transfer list --controller ... --receiver ... --status settled|archived`). Torii’s
  `/v1/offline/transfers{,/query}` endpoints now accept the same certificate/verdict filters
  (`certificate_id_hex`, `certificate_expires_before_ms`, `refresh_before_ms`, `verdict_id_hex`,
  etc.) and export `controller_id`, `controller_display`, `status`, `recorded_at_{ms,height}`,
  `archived_at_height`, plus the certificate metadata (`certificate_id_hex`,
  `certificate_expires_at_ms`, `policy_expires_at_ms`, `refresh_at_ms`, `verdict_id_hex`,
  `attestation_nonce_hex`) in the JSON response so compliance teams can page across hot vs. archived
  bundles without custom SQL.【crates/iroha_core/src/smartcontracts/isi/offline.rs:1009】【crates/iroha_cli/src/offline.rs:135】【crates/iroha_torii/src/routing.rs:23869】

## 11. Commitment Issuance & Certificate Tooling (OA1)

- **Data model + Norito schema.** `OfflineAllowanceCommitment`, `OfflineWalletPolicy`, and
  `OfflineWalletCertificate` implement the OA1 schema in `iroha_data_model`, derive Norito codec
  traits, and drive the canonical payload operators sign when issuing certificates.【crates/iroha_data_model/src/offline/mod.rs:170】【crates/iroha_data_model/src/offline/mod.rs:200】
  `OfflineWalletCertificate::operator_signing_bytes()` serializes the documented struct so issuers
  cannot diverge from the published commitment format.【crates/iroha_data_model/src/offline/mod.rs:235】
- **Issuance tooling.** `scripts/offline_topup/run.sh` (and the underlying `cargo xtask offline-topup`)
  consumes the JSON spec described earlier, mints certificates plus `RegisterOfflineAllowance`
  instructions, and optionally submits them to Torii so operators have a deterministic workflow that
  mirrors the spec.【scripts/offline_topup/README.md:1】
- **Ledger admission + inspection.** `RegisterOfflineAllowance` enforces policy metadata, operator
  signatures, and allowance bounds when inserting records, while `iroha offline allowance list|get`
  exposes the registered certificate IDs for operators and auditors.【crates/iroha_data_model/src/isi/offline.rs:4】【crates/iroha_core/src/smartcontracts/isi/offline.rs:36】【crates/iroha_cli/src/offline.rs:47】

## 12. Platform Counters, Journals, and Storage Hardening (OA2)

- **Counter checkpoints.** Each `OfflineAllowanceRecord` carries an `OfflineCounterState` that records
  the latest Apple App Attest key counter or Android series counter, and `OfflineCounterSummary`
  exposes that state over Torii so receivers mirror exactly what the ledger enforces.【crates/iroha_data_model/src/offline/mod.rs:400】【crates/iroha_data_model/src/offline/mod.rs:424】
- **Ledger enforcement.** `SubmitOfflineToOnlineTransfer` stages platform proofs, calls
  `enforce_monotonic_entry` for every receipt, and merges the staged counters back into storage so
  counter windows remain contiguous even after crashes.【crates/iroha_core/src/smartcontracts/isi/offline.rs:788】【crates/iroha_core/src/smartcontracts/isi/offline.rs:820】
- **Sealed WAL/journal.** `iroha::offline::OfflineJournal`, `IrohaSwift.OfflineJournal`, and the
  Android `OfflineJournal` keep pending/committed entries in a hash-chained, HMAC-authenticated log
  so devices can recover and prove counter state even when offline for long periods, satisfying the
  OA2 storage-hardening goal.【crates/iroha/src/offline/mod.rs:1】【IrohaSwift/Sources/IrohaSwift/OfflineJournal.swift:19】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineWallet.java:17】

## 13. Receiver Counter Summaries & Replay Digests (OA3)

`OfflineCounterSummary` captures the latest counter checkpoints per certificate, including the
const deterministic `summary_hash` that receivers can sign or gossip when auditing merchants.【crates/iroha_data_model/src/offline/mod.rs:424】
For every allowance the node derives a summary lazily and exposes it through both the gRPC query
surface and the `/v1/offline/summaries{,/query}` Torii endpoints so merchants do not need extra
indexes.【crates/iroha_torii/src/routing.rs:10204】

- **Publishing digests:** `iroha offline summary export --output counters.json --pretty` now streams
  the summaries through the query API, applies the same filtering/sorting flags as the other offline
  subcommands, and emits a JSON envelope (`generated_at_ms`, `summary_count`, entries with
  `certificate_id_hex`, `summary_hash_hex`, and the per-platform counter maps).【crates/iroha_cli/src/offline.rs:110】
  The file is intentionally small (counter maps + hex identifiers) so wallets can hand the digest to
  BLE/QR relays or post it to their reconciliation services without leaking receipt payloads.
- **Receiver-facing format:** Each summary entry matches the on-ledger checkpoint and includes the
  hash of the underlying counter maps, which means merchants can sign only the 32-byte hash when
  exchanging snapshots; peers can rehydrate the full map from the digest if they need to inspect the
  last seen counter for a specific App Attest key or Android series.
- **Replay log & gossip cadence:** Wallets persist their pending receipts inside
  `iroha::offline::OfflineJournal` (Rust), `IrohaSwift.OfflineJournal`, and the Android
  `OfflineJournal` helpers. The journals store `[kind | timestamp | len | tx_id | payload | chain |
  hmac]` entries and keep the hash chain/HMAC key alive even across device restores, so the exported
  summary plus the journal history is enough to prove a counter rollback attempt.【crates/iroha/src/offline/mod.rs:1】【IrohaSwift/Sources/IrohaSwift/OfflineJournal.swift:19】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineWallet.java:19】
- **Gossip tooling:** Operators can schedule `iroha offline summary export` on the controller side
  and push the resulting JSON to their support tooling or to a lightweight "summary relay" so
  merchants operating intermittently-online devices can fetch the latest counter digests before
  accepting receipts.

## 14. Commitment → Bundle Reconciliation Path (OA4)

The ledger path is fully wired: `RegisterOfflineAllowance` persists the operator certificate and the
initial commitment, while `SubmitOfflineToOnlineTransfer` validates every receipt and ties the
claimed delta to `C_0 → C_1`.【crates/iroha_data_model/src/isi/offline.rs:4】

1. **Certificate validation:** `iroha_core` enforces operator signatures, policy envelopes, and
   metadata requirements (App Attest and KeyMint bindings) before inserting the allowance
   record.【crates/iroha_core/src/smartcontracts/isi/offline.rs:36】
2. **Bundle admission:** During `SubmitOfflineToOnlineTransfer`, the validator verifies spend
   signatures, stages the per-platform counters, enforces monotonicity via
   `enforce_monotonic_entry`, and rejects any bundle where the balance proof does not satisfy
   `record.current_commitment == balance_proof.initial_commitment` or where the claimed delta does
   not match the sum of receipts.【crates/iroha_core/src/smartcontracts/isi/offline.rs:752】
3. **State updates:** Once a bundle settles, `merge_counter_state` merges the staged counters back
   into the allowance record so the next `OfflineCounterSummary` reflects the new checkpoint, and
   `OfflineTransferRecord` stores the full submission for telemetry/audit queries.【crates/iroha_core/src/smartcontracts/isi/offline.rs:788】
4. **Operator inspection:** CLI helpers (`iroha offline allowance list|get`, `iroha offline transfer
   list|get`, `iroha offline summary list/export`) plus the `/v1/offline/*` REST endpoints provide a
   reconciliation dashboard without shipping custom database scripts.【crates/iroha_cli/src/offline.rs:29】【crates/iroha_torii/src/routing.rs:23200】

SDKs already build the exact payloads required by OA4: `OfflineWallet` on Swift/Android batches
receipts, calculates the balance proof delta, and posts the bundle via the Torii helpers while
recording the attempt in their journals.【IrohaSwift/Sources/IrohaSwift/OfflineWallet.swift:6】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineWallet.java:17】

## 15. Platform Proof Verification & Telemetry (OA8)

- **App Attest verification.** `verify_apple_attestation` parses the operator-provided attestation
  report, pins the Apple trust anchors, recomputes the nonce derived from the receipt payload, and
  enforces that the monotonic counter advances exactly once per spend before accepting the
  proof.【crates/iroha_core/src/smartcontracts/isi/offline.rs:1034】
- **Android KeyMint verification.** `verify_android_attestation` decodes the X.509 chain, validates
  the KeyDescription (parsing `attestationApplicationId`, StrongBox/rollback flags, alias metadata),
  and optionally enforces marker-key signatures so marker series + counters remain bound to the
  certificate metadata.【crates/iroha_core/src/smartcontracts/isi/offline.rs:1068】
- **Canonical challenge + metadata binding.** `derive_receipt_challenge` hashes
  `Hash(chain_id) || {invoice, receiver, asset, amount, issued_at_ms, tx_id}` and both verifiers compare their
  inputs against the metadata recorded in `OfflineWalletCertificate`, ensuring servers recompute the
  exact chain-bound challenge described in the spec.【crates/iroha_core/src/smartcontracts/isi/offline.rs:562】
- **Telemetry.** `iroha_core` records settlement/archival metrics via `iroha_telemetry::metrics`
  (`record_offline_transfer_settlement`, `inc_offline_transfer_archived`) and now annotates every
  rejection path with `record_offline_transfer_rejection`, which feeds the
  `iroha_offline_transfer_rejections_total{platform,reason}` counter so operators can see which
  App Attest / KeyMint checks are failing. Torii exposes both the historical records
  (`/v1/offline/*`) and the aggregated rejection stats via
  `/v1/offline/rejections`, letting dashboards surface per-platform/per-reason trends without
  scraping Prometheus directly. The JavaScript SDK mirrors this surface through
  `ToriiClient.getOfflineRejectionStats`, so browser dashboards and CI agents can read the same
  counters without bespoke HTTP plumbing.【crates/iroha_core/src/smartcontracts/isi/offline.rs:681】【crates/iroha_telemetry/src/metrics.rs:6001】【crates/iroha_torii/src/routing.rs:23427】【javascript/iroha_js/src/toriiClient.js:395】【javascript/iroha_js/index.d.ts:1950】

## 16. Offline-Only Bearer Mode Guidance (OA6)

Some deployments prefer a bearer-mode experience where offline receipts circulate without ever
depositing them back on-ledger. `docs/source/offline_bearer_mode.md` captures the governance and UX
guidance for that posture: risk disclosures, treasury accounting implications, how wallet settings
should label the "skip reconciliation" toggle, and how to communicate the missing commitment delta
on user interfaces.【docs/source/offline_bearer_mode.md:1】 Wallets still record the
hardware counters/WAL entries so they can share counter summaries with receivers or reconcile later
if operators opt back into ledger-reconcilable mode. The doc spells out when skipping ledger
reconciliation is acceptable and how to re-enable deposits without losing state.

## 17. Next Steps

- OA2–OA4 extend this foundation with WAL/journal crates, SDK counter-sharing APIs, and the ZK/FASTPQ
  verifier referenced in the roadmap.
- When the Pedersen circuit is merged, update the commitment helper in `scripts/offline_topup/` and
  add the proof generator hook inside the same xtask command.
