---
lang: dz
direction: ltr
source: docs/source/references/address_curve_registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b10d01e0a893a4df2e64f80b038778bfa8f706c0b0f574b9081d129c571a28da
source_last_modified: "2026-01-22T16:26:46.583380+00:00"
translation_last_reviewed: 2026-02-07
title: Account Curve Registry
description: Canonical mapping between account controller curve identifiers and signing algorithms.
---

# Account Curve Registry

Account addresses encode their controllers as a tagged payload that begins with
an 8-bit curve identifier. Validators, SDKs, and tooling rely on a shared
registry so that curve identifiers remain stable across releases and enable
deterministic decoding across implementations.

The table below is the normative reference for every assigned `curve_id`. A
machine-readable copy ships alongside this document at
[`address_curve_registry.json`](address_curve_registry.json); automated tooling
SHOULD consume the JSON version and pin its `version` field when generating
fixtures.

## Registered Curves

| ID (`curve_id`) | Algorithm | Feature Gate | Status | Public Key Encoding | Notes |
|-----------------|-----------|--------------|--------|---------------------|-------|
| `0x01` (1) | `ed25519` | — | Production | 32-byte compressed Ed25519 key | Canonical curve for V1. All SDK builds MUST support this identifier. |
| `0x02` (2) | `ml-dsa` | — | Production (config-gated) | Dilithium3 public key (1952 bytes) | Available in all builds. Enable in `crypto.allowed_signing` + `crypto.curves.allowed_curve_ids` before emitting controller payloads. |
| `0x03` (3) | `bls_normal` | `bls` | Production (feature-gated) | 48-byte compressed G1 public key | Required for consensus validators. Admission allows BLS controllers even when `allowed_signing`/`allowed_curve_ids` omit them. |
| `0x04` (4) | `secp256k1` | — | Production | 33-byte SEC1-compressed key | Deterministic ECDSA over SHA-256; signatures use the canonical 64-byte `r∥s` layout. |
| `0x05` (5) | `bls_small` | `bls` | Production (feature-gated) | 96-byte compressed G2 public key | Compact-signature BLS profile (smaller signatures, larger public keys). |
| `0x0A` (10) | `gost3410-2012-256-paramset-a` | `gost` | Reserved | 64-byte little-endian TC26 param set A point | Unlocks with the `gost` feature once governance approves the rollout. |
| `0x0B` (11) | `gost3410-2012-256-paramset-b` | `gost` | Reserved | 64-byte little-endian TC26 param set B point | Mirrors the TC26 B parameter set; blocked behind the `gost` feature gate. |
| `0x0C` (12) | `gost3410-2012-256-paramset-c` | `gost` | Reserved | 64-byte little-endian TC26 param set C point | Reserved for future governance approval. |
| `0x0D` (13) | `gost3410-2012-512-paramset-a` | `gost` | Reserved | 128-byte little-endian TC26 param set A point | Reserved pending demand for 512-bit GOST curves. |
| `0x0E` (14) | `gost3410-2012-512-paramset-b` | `gost` | Reserved | 128-byte little-endian TC26 param set B point | Reserved pending demand for 512-bit GOST curves. |
| `0x0F` (15) | `sm2` | `sm` | Reserved | DistID length (u16 BE) + DistID bytes + 65-byte SEC1 uncompressed SM2 key | Becomes available when the `sm` feature graduates from preview. |

### Usage Guidelines

- **Fail closed:** Encoders MUST reject unsupported algorithms with
  `ERR_UNSUPPORTED_ALGORITHM`. Decoders MUST raise `ERR_UNKNOWN_CURVE` for any
  identifier not listed in this registry.
- **Feature gating:** BLS/GOST/SM2 remain behind the listed build-time feature
  gates. Operators must enable matching `iroha_config.crypto.allowed_signing`
  entries and build-time features before emitting addresses with those curves.
- **Admission exceptions:** BLS controllers are allowed for consensus
  validators even when `allowed_signing`/`allowed_curve_ids` do not list them.
- **Config + manifest parity:** Use `iroha_config.crypto.allowed_curve_ids`
  (and the matching `ManifestCrypto.allowed_curve_ids`) to publish which curve
  identifiers the cluster accepts for controllers; admission now enforces this
  list alongside `allowed_signing`.
- **Deterministic encoding:** Public keys are encoded exactly as returned by
  the signing implementation (Ed25519 compressed bytes, ML‑DSA public key
  bytes, BLS compressed points, etc.). SDKs should surface validation errors
  before submitting malformed payloads.
- **Manifest parity:** Genesis manifests and controller manifests MUST use the
  same identifiers so admission can reject controllers that exceed cluster
  capabilities.

## Capability Bitmask Advert

`GET /v2/node/capabilities` now exposes both the `allowed_curve_ids` list and
the packed `allowed_curve_bitmap` array under `crypto.curves`. The bitmap is
little-endian across 64-bit lanes (up to four values to cover the 0–255 `u8`
identifier space). Bit `i` being set means curve identifier `i` is permitted by
the cluster’s admission policy.

- Example: `{ allowed_curve_ids: [1, 15] }` ⇒ `allowed_curve_bitmap: [32770]`
  because `(1 << 1) | (1 << 15) = 32770`.
- Curves above `63` set bits in later lanes. Trailing zero lanes are omitted to
  keep payloads short, so a configuration that also enables `curve_id = 130`
  would emit `allowed_curve_bitmap = [32768, 0, 4]` (bits 15 and 130 set).

Prefer the bitmap for dashboards and health checks: a single bit test answers
capability questions without scanning the full array, while tooling that needs
ordered identifiers can continue using `allowed_curve_ids`. Surfacing both
views satisfies roadmap item **ADDR-3**’s requirement to publish deterministic
capability bitmasks for operators and SDKs.

## Validation Checklist

Every component that ingests controllers (Torii, admission, SDK encoders,
offline tooling) must apply the same deterministic checks before accepting a
payload. The steps below should be treated as mandatory validation logic:

1. **Resolve cluster policy:** Parse the leading `curve_id` byte from the
   account payload and reject the controller if the identifier is not present
   in `iroha_config.crypto.allowed_curve_ids` (and the mirrored
   `ManifestCrypto.allowed_curve_ids`). BLS controllers are the exception: when
   compiled in, admission allows them regardless of the allowlists so consensus
   validator keys keep working. This prevents clusters from accepting preview
   curves that operators have not explicitly enabled.
2. **Enforce encoding length:** Compare the payload length against the
   algorithm’s canonical size before attempting to decompress or expand the
   key. Reject any value that fails the length check to eliminate malformed
   inputs early.
3. **Run algorithm-specific decoding:** Use the same canonical decoders as
   `iroha_crypto` (`ed25519_dalek`, `pqcrypto_dilithium`, `w3f_bls`/`blstrs`,
   `sm2`, the TC26 helpers, etc.) so all implementations share the exact
   subgroup/point validation behaviour.
4. **Verify signature sizes:** Admission and SDKs must enforce the signature
   lengths listed below and reject any payload with a truncated or overlong
   signature before running the verifier.

| Algorithm | `curve_id` | Public Key Bytes | Signature Bytes | Critical Checks |
|-----------|------------|------------------|-----------------|-----------------|
| `ed25519` | `0x01` | 32 | 64 | Reject non-canonical compressed points, enforce cofactor clearing (no small-order points), and ensure `s < L` when validating signatures. |
| `ml-dsa` (Dilithium3) | `0x02` | 1952 | 3309 | Reject payloads that are not exactly 1952 bytes before decoding; parse the Dilithium3 public key and verify signatures using pqcrypto-dilithium with canonical byte lengths. |
| `bls_normal` | `0x03` | 48 | 96 | Accept only canonical compressed G1 public keys and compressed G2 signatures; reject identity points and non-canonical encodings. |
| `secp256k1` | `0x04` | 33 | 64 | Accept only SEC1-compressed points; decompress and reject non-canonical/invalid points, and verify signatures using the canonical 64-byte `r∥s` encoding (low-`s` normalisation enforced by the signer). |
| `bls_small` | `0x05` | 96 | 48 | Accept only canonical compressed G2 public keys and compressed G1 signatures; reject identity points and non-canonical encodings. |
| `gost3410-2012-256-paramset-a` | `0x0A` | 64 | 64 | Interpret the payload as `(x||y)` little-endian coordinates, ensure each coordinate `< p`, reject the identity point, and enforce canonical 32-byte `r`/`s` limbs when verifying signatures. |
| `gost3410-2012-256-paramset-b` | `0x0B` | 64 | 64 | Same validation as param set A but using the TC26 B domain parameters. |
| `gost3410-2012-256-paramset-c` | `0x0C` | 64 | 64 | Same validation as param set A but using the TC26 C domain parameters. |
| `gost3410-2012-512-paramset-a` | `0x0D` | 128 | 128 | Interpret `(x||y)` as 64-byte limbs, ensure `< p`, reject the identity point, and require 64-byte `r`/`s` limbs for signatures. |
| `gost3410-2012-512-paramset-b` | `0x0E` | 128 | 128 | Same validation as param set A but using the TC26 B 512-bit domain parameters. |
| `sm2` | `0x0F` | 2 + distid + 65 | 64 | Decode distid length (u16 BE), validate the DistID bytes, parse the SEC1 uncompressed point, enforce GM/T 0003 subgroup rules, apply the configured DistID, and require canonical `(r, s)` limbs per SM2. |

Each row maps to the `validation` object inside
[`address_curve_registry.json`](address_curve_registry.json). Tooling that
consumes the JSON export can rely on the `public_key_bytes`,
`signature_bytes`, and `checks` fields to automate the same validation steps
described above; variable-length encodings (for example SM2) set
`public_key_bytes` to null and document the length rule in `checks`.

## Requesting a New Curve Identifier

1. Draft the algorithm specification (encoding, validation, error handling) and
   secure governance approval for the rollout.
2. Submit a pull request updating both this document and
   `address_curve_registry.json`. New identifiers must be unique and fall within
   the inclusive range `0x01..=0xFE`.
3. Update SDKs, Norito fixtures, and operator documentation with the new
   identifier before deploying to production networks.
4. Coordinate with the security and observability leads to ensure telemetry,
   runbooks, and admission policies reflect the new algorithm.
