# Account Structure RFC

**Status:** Accepted (ADDR-1)  
**Audience:** Data model, Torii, Nexus, Wallet, Governance teams  
**Related issues:** TBD

## Summary

This document describes the shipping account-addressing stack implemented in
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) and the
companion tooling. It provides:

- A checksummed, human-facing **Iroha Base58 address (I105)** produced by
  `AccountAddress::to_i105` that binds a chain discriminant to the account
  controller and offers deterministic interop-friendly textual forms.
- A domainless canonical account payload keyed only by the controller.
  Explicit domain context now lives outside the payload via
  `ScopedAccountId { account, domain }` and domain-link state.

## Motivation

Wallets and off-chain tooling rely on raw `alias@domain` (rejected legacy form) routing aliases today. This
has two major drawbacks:

1. **No network binding.** The string has no checksum or chain prefix, so users
   can paste an address from the wrong network without immediate feedback. The
   transaction will eventually be rejected (chain mismatch) or, worse, succeed
   against an unintended account if the destination exists locally.
2. **Domain collision.** Domains are namespace-only and can be reused on each
   chain. Federation of services (custodians, bridges, cross-chain workflows)
   becomes brittle because `finance` on chain A is unrelated to `finance` on
   chain B.

We need a human-friendly address format that guards against copy/paste errors
and a deterministic mapping from domain name to the authoritative chain.

## Goals

- Describe the I105 Base58 envelope implemented in the data model and the
  canonical parsing/alias rules that `AccountId` and `AccountAddress` follow.
- Encode the configured chain discriminant directly into each address and
  define its governance/registry process.
- Describe how to introduce a global domain registry without breaking current
  deployments and specify normalization/anti-spoofing rules.

## Non-goals

- Implementing cross-chain asset transfers. The routing layer only returns the
  target chain.
- Finalising governance for global domain issuance. This RFC focuses on the data
  model and transport primitives.

## Background

### Current routing alias

```
AccountId {
    controller: AccountController // single PublicKey or multisig policy
}
ScopedAccountId {
    account: AccountId,
    domain: DomainId,
}

Display / JSON text: canonical I105 literal only
Parse accepts:
- Canonical I105 account literals only.
- Runtime parsers reject non-canonical/legacy dotted I105 literals, legacy `norito:<hex>`,
  canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as
  `label@domain`.

Domain context is explicit and out-of-band. There is no public
`AccountSubjectId`; subject identity is `AccountId`.
```

`ChainId` lives outside of `AccountId`. Nodes check the transaction’s `ChainId`
against configuration during admission (`AcceptTransactionFail::ChainIdMismatch`)
and reject foreign transactions, but the account string itself carries no
network hint.

### Domain identifiers

`DomainId` wraps a `Name` (normalized string) and is scoped to the local chain.
Every chain can register `wonderland`, `finance`, etc. independently.

### Nexus context

Nexus is responsible for cross-component coordination (lanes/data-spaces). It
currently has no concept of cross-chain domain routing.

## Proposed Design

### 1. Deterministic chain discriminant

`iroha_config::parameters::actual::Common` now exposes:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Constraints:**
  - Unique per active network; managed through a signed public registry with
    explicit reserved ranges (e.g., `0x0000–0x0FFF` test/dev, `0x1000–0x7FFF`
    community allocations, `0x8000–0xFFEF` governance-approved, `0xFFF0–0xFFFF`
    reserved).
  - Immutable for a running chain. Changing it requires a hard fork and a
    registry update.
- **Governance & registry (planned):** A multi-signature governance set will
  maintain a signed JSON registry mapping discriminants to human aliases and
  CAIP-2 identifiers. This registry is not yet part of the shipped runtime.
- **Usage:** Threaded through state admission, Torii, SDKs, and wallet APIs so
  every component can embed or validate it. CAIP-2 exposure remains a future
  interop task.

### 2. Canonical address codecs

The Rust data model now distinguishes the public `AccountId` surface from the
lower-level `AccountAddress` helper.

- `AccountId` text/JSON parsing is hard-cut to canonical I105 only.
- `AccountAddress` remains the canonical binary envelope and can still be
  rendered as I105 or as canonical hex for low-level debugging and
  address-envelope JSON.

- **I105 (Iroha Base58)** – a Base58 envelope that embeds the chain
  discriminant. Decoders validate the prefix before promoting the payload to
  the canonical form.
- **Canonical hex** – a debugging-friendly `0x…` encoding of the canonical byte
  envelope.

`AccountAddress::parse_encoded` accepts I105 forms for
the raw address envelope. `AccountId::parse_encoded`, `FromStr`, and JSON
deserialization accept only canonical I105 and reject non-canonical/legacy
I105 forms, canonical hex, `norito:`, alias, and `@domain` forms.

#### 2.1 Header byte layout (ADDR-1a)

Every canonical payload is laid out as `header · controller`. The
`header` is a single byte that communicates which parser rules apply to the bytes that
follow:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

The first byte therefore packs the schema metadata for downstream decoders:

| Bits | Field | Allowed values | Error on violation |
|------|-------|----------------|--------------------|
| 7-5  | `addr_version` | `0` (v1). Values `1-7` are reserved for future revisions. | Values outside `0-7` trigger `AccountAddressError::InvalidHeaderVersion`; implementations MUST treat non-zero versions as unsupported today. |
| 4-3  | `addr_class` | `0` = single key, `1` = multisig. | Other values raise `AccountAddressError::UnknownAddressClass`. |
| 2-1  | `norm_version` | `1` (Norm v1). Values `0`, `2`, `3` are reserved. | Values outside `0-3` raise `AccountAddressError::InvalidNormVersion`. |
| 0    | `ext_flag` | MUST be `0`. | Set bit raises `AccountAddressError::UnexpectedExtensionFlag`. |

The Rust encoder writes `0x02` for single-key controllers (version 0, class 0,
norm v1, extension flag cleared) and `0x0A` for multisig controllers (version 0,
class 1, norm v1, extension flag cleared).

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` and via account-domain link state. Converting an `AccountAddress`
into a scoped account therefore requires the caller to supply the domain
explicitly. The public subject-identity surface is just `AccountId`;
`AccountSubjectId` is not part of the public API.

#### 2.3 Controller payload encodings (ADDR-1a)

The controller payload is a tagged union appended immediately after the header in
canonical payloads:

| Tag | Controller | Layout | Notes |
|-----|------------|--------|-------|
| `0x00` | Single key | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` maps to Ed25519 today. `key_len` is bounded to `u8`; larger values raise `AccountAddressError::KeyPayloadTooLong` (so single-key ML‑DSA public keys, which are >255 bytes, cannot be encoded and must use multisig). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u16` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | The encoded member count is 16-bit; the old 255-member hard cap is gone. Unknown curves raise `AccountAddressError::UnknownCurve`; malformed policies bubble up as `AccountAddressError::InvalidMultisigPolicy`. |

Multisig policies also expose a CTAP2-style CBOR map and canonical digest so
hosts and SDKs can verify the controller deterministically. See
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) for the schema,
validation rules, hashing procedure, and golden fixtures.

All key bytes are encoded exactly as returned by `PublicKey::to_bytes`; decoders reconstruct `PublicKey` instances and raise `AccountAddressError::InvalidPublicKey` if the bytes do not match the declared curve.

> **Ed25519 canonical enforcement (ADDR-3a):** curve `0x01` keys must decode to the exact byte string emitted by the signer and must not lie in the small-order subgroup. Nodes now reject non-canonical encodings (e.g., values reduced modulo `2^255-19`) and weak points such as the identity element, so SDKs should surface matching validation errors before submitting addresses.

##### 2.3.1 Curve identifier registry (ADDR-1d)

| ID (`curve_id`) | Algorithm | Feature gate | Notes |
|-----------------|-----------|--------------|-------|
| `0x00` | Reserved | — | MUST NOT be emitted; decoders surface `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Canonical v1 algorithm (`Algorithm::Ed25519`); enabled in the default config. |
| `0x02` | ML‑DSA (Dilithium3) | — | Uses the Dilithium3 public key bytes (1952 bytes). Single‑key addresses cannot encode ML‑DSA because `key_len` is `u8`; multisig uses `u16` lengths. |
| `0x03` | BLS12‑381 (normal) | `bls` | Public keys in G1 (48 bytes), signatures in G2 (96 bytes). |
| `0x04` | secp256k1 | — | Deterministic ECDSA over SHA‑256; public keys use the 33‑byte SEC1 compressed form and signatures use the canonical 64‑byte `r∥s` layout. |
| `0x05` | BLS12‑381 (small) | `bls` | Public keys in G2 (96 bytes), signatures in G1 (48 bytes). |
| `0x0A` | GOST R 34.10‑2012 (256, set A) | `gost` | Available only when the `gost` feature is enabled. |
| `0x0B` | GOST R 34.10‑2012 (256, set B) | `gost` | Available only when the `gost` feature is enabled. |
| `0x0C` | GOST R 34.10‑2012 (256, set C) | `gost` | Available only when the `gost` feature is enabled. |
| `0x0D` | GOST R 34.10‑2012 (512, set A) | `gost` | Available only when the `gost` feature is enabled. |
| `0x0E` | GOST R 34.10‑2012 (512, set B) | `gost` | Available only when the `gost` feature is enabled. |
| `0x0F` | SM2 | `sm` | DistID length (u16 BE) + DistID bytes + 65‑byte SEC1 uncompressed SM2 key; available only when `sm` is enabled. |

Slots `0x06–0x09` remain unassigned for additional curves; introducing a new
algorithm requires a roadmap update and matching SDK/host coverage. Encoders
MUST reject any unsupported algorithm with `ERR_UNSUPPORTED_ALGORITHM`, and
decoders MUST fail fast on unknown ids with `ERR_UNKNOWN_CURVE` to preserve
fail-closed behaviour.

The canonical registry (including a machine-readable JSON export) lives under
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Tooling SHOULD consume that dataset directly so curve identifiers remain
consistent across SDKs and operator workflows.

- **SDK gating:** SDKs default to Ed25519-only validation/encoding. Swift exposes
  compile-time flags (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); the Java/Android SDK requires
  `AccountAddress.configureCurveSupport(...)`; the JavaScript SDK uses
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 support is available but not enabled by default in the JS/Android
  SDKs; callers must opt in explicitly when emitting non‑Ed25519 controllers.
- **Host gating:** `Register<Account>` rejects controllers whose signatories use algorithms
  missing from the node’s `crypto.allowed_signing` list **or** curve identifiers absent from
  `crypto.curves.allowed_curve_ids`, so clusters must advertise support (configuration +
  genesis) before ML‑DSA/GOST/SM controllers can be registered. BLS controller
  algorithms are always allowed when compiled (consensus keys rely on them),
  and the default configuration enables Ed25519 + secp256k1.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig controller guidance

`AccountController::Multisig` serialises policies via
`crates/iroha_data_model/src/account/controller.rs` and enforces the schema
documented in [`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md).
Key implementation details:

- Policies are normalised and validated by `MultisigPolicy::validate()` before
  being embedded. Thresholds must be ≥ 1 and ≤ Σ weight; duplicate members are
  removed deterministically after sorting by `(algorithm || 0x00 || key_bytes)`.
- The binary controller payload (`ControllerPayload::Multisig`) encodes
  `version:u8`, `threshold:u16`, `member_count:u16`, then each member’s
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. This is exactly what
  `AccountAddress::canonical_bytes()` writes to I105 payloads.
- Multisig address encoding is no longer limited by an 8-bit member counter.
  The binary field is `u16`, so the old 255-member hard cap no longer applies.
- Hashing (`MultisigPolicy::digest_blake2b256()`) uses Blake2b-256 with the
  `iroha-ms-policy` personalization string so governance manifests can bind to a
  deterministic policy ID that matches the controller bytes embedded in I105.
- Fixture coverage lives in `fixtures/account/address_vectors.json` (cases
  `addr-multisig-*`). Wallets and SDKs should assert the canonical I105 strings
  below to confirm their encoders match the Rust implementation.

| Case ID | Threshold / members | I105 literal (prefix `0x02F1`) | Sora I105 literal | Notes |
|---------|---------------------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` weight, members `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Council-domain governance quorum. |
| `addr-multisig-wonderland-threshold2` | `≥2`, members `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Dual-signature wonderland example (weight 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, members `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Implicit-default domain quorum used for base governance.

#### 2.4 Failure rules (ADDR-1a)

- Payloads shorter than the required canonical header+controller size emit
  `AccountAddressError::InvalidLength` or
  `AccountAddressError::UnexpectedTrailingBytes`.
- Headers that set the reserved `ext_flag` or advertise unsupported versions/classes MUST be rejected using `UnexpectedExtensionFlag`, `InvalidHeaderVersion`, or `UnknownAddressClass`.
- Unknown controller tags raise `UnknownControllerTag`.
- Oversized or malformed key material raises `KeyPayloadTooLong` or `InvalidPublicKey`.
- Multisig controllers that exceed the encodable `u16` member count raise
  `MultisigMemberOverflow`; there is no 255-member limit.
- IME/NFKC conversions: half-width Sora kana can be normalised to their full-width forms without breaking decoding, but the ASCII `sora` sentinel and I105 digits/letters MUST stay ASCII. Full-width or case-folded sentinels surface `ERR_MISSING_COMPRESSED_SENTINEL`, full-width ASCII payloads raise `ERR_INVALID_COMPRESSED_CHAR`, and checksum mismatches bubble up as `ERR_CHECKSUM_MISMATCH`. Property tests in `crates/iroha_data_model/src/account/address.rs` cover these paths so SDKs and wallets can rely on deterministic failures.
- `AccountId` text/JSON parsing does not attempt alias or domain fallback:
  non-canonical/legacy I105 literals, `norito:<hex>`, canonical hex, `@domain`, and alias
  forms are rejected up front in favor of canonical I105 only.

#### 2.5 Normative binary vectors

- **Selector-free canonical single-key payload (`seed byte 0x00`)**  
  Canonical hex: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Breakdown: `0x02` header, `0x00` controller tag, `0x01` curve id (Ed25519),
  `0x20` key length, followed by the 32-byte key payload.
- **Canonical multisig payload layout**  
  `0x0A | 0x01 | version:u8 | threshold:u16 | member_count:u16 | ...members`
  where each member is encoded as `(curve_id:u8, weight:u16, key_len:u16,
  key_bytes)`.

The fixture bundle in `fixtures/account/address_vectors.json` still publishes
I105 outputs plus non-canonical legacy-vector fixtures for the same canonical payloads.
Public `AccountId` parser tests separately assert that only canonical I105 is
accepted.

Reviewed-by: Data Model WG, Cryptography WG — scope approved for ADDR-1a.

##### Sora Nexus reference aliases

Sora Nexus networks default to `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). The
`AccountAddress::to_i105` helpers therefore emit
consistent textual forms for every canonical payload. Selected fixtures from
`fixtures/account/address_vectors.json` (generated via
`cargo xtask address-vectors`) are shown below for quick reference:

| Account label / note | I105 literal (prefix `0x02F1`) | Sora I105 literal |
|--------------------|--------------------------------|-------------------------|
| `default` account (selector-free canonical payload, seed `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| `treasury` account (selector-free canonical payload, seed `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |

These strings match the ones emitted by the CLI (`iroha tools address convert`), Torii
responses (`canonical I105 literal rendering`), and SDK helpers, so UX copy/paste
flows can rely on them verbatim. Canonical I105 is the only public `AccountId`
text/JSON input form.

#### 2.6 Textual aliases for interoperability (planned)

- **Chain-alias style:** `ih:<chain-alias>:<alias@domain>` for logs and human
  entry. Wallets must parse the prefix, verify the embedded chain, and block
  mismatches.
- **CAIP-10 form:** `iroha:<caip-2-id>:<i105-addr>` for chain-agnostic
  integrations. This mapping is **not yet implemented** in the shipped
  toolchains.
- **Machine helpers:** Publish codecs for Rust, TypeScript/JavaScript, Python,
  and Kotlin covering canonical I105 (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded`, and their SDK equivalents). CAIP-10 helpers are
  future work.

#### 2.7 Deterministic I105 alias

- **Prefix mapping:** Reuse the `chain_discriminant` as the I105 network prefix.
  `encode_i105_prefix()` (see `crates/iroha_data_model/src/account/address.rs`)
  emits a 6‑bit prefix (single byte) for values `<64` and a 14‑bit, two-byte
  form for larger networks. The authoritative assignments live in
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDKs MUST keep the matching JSON registry in sync to avoid collisions.
- **Account material:** I105 encodes the canonical payload built by
  `AccountAddress::canonical_bytes()`—header byte and controller payload.
  Domain-selector bytes are not emitted in canonical payloads. There is no additional hashing step; I105 embeds the
  binary controller payload (single key or multisig) as produced by the Rust
  encoder, not the CTAP2 map used for multisig policy digests.
- **Encoding:** `encode_i105()` concatenates the prefix bytes with the canonical
  payload and appends a 16-bit checksum derived from Blake2b-512 with the fixed
  prefix `I105PRE` (`b"I105PRE" || prefix || payload`). The result is Base58-encoded via `bs58`.
  CLI/SDK helpers expose the same procedure, and `AccountAddress::parse_encoded`
  reverses it via `decode_i105`.

#### 2.8 Normative textual test vectors

`fixtures/account/address_vectors.json` contains full I105 and
I105 helper literals for canonical payloads.
Highlights:

- **`addr-single-default-ed25519` (Sora Nexus, prefix `0x02F1`).**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`. Torii emits this exact
  string from `AccountId`’s `Display` implementation (canonical I105).
- **Multisig controller payloads.**  
  Canonical controller bytes now encode `member_count:u16`, so address encoding
  no longer has the old 255-member hard cap.
- **Failure case (`i105-prefix-mismatch`).**  
  Parsing an I105 literal encoded with prefix `NETWORK_PREFIX + 1` on a node
  expecting the default prefix yields
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  before domain routing is attempted. Negative-path `AccountId` tests also cover
  rejected `sora...`, `norito:...`, `0x...`, alias, and `@domain` inputs. The
  `i105-checksum-mismatch` fixture exercises tampering detection over the Blake2b
  checksum.

#### 2.9 Compliance fixtures

ADDR‑2 ships a replayable fixture bundle covering positive and negative
scenarios across canonical I105, non-canonical legacy-vector output, canonical
`AccountAddress` hex renderings, selector-free payloads, multisignature
controllers, and rejected legacy `AccountId` literal forms. The canonical JSON
lives in `fixtures/account/address_vectors.json` and can be regenerated with:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

For ad-hoc experiments (different paths/formats) the example binary is still
available:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

Rust unit tests in `crates/iroha_data_model/tests/account_address_vectors.rs`
and `crates/iroha_torii/tests/account_address_vectors.rs`, together with the JS,
Swift, and Android harnesses (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
consume the same fixture to guarantee codec parity across SDKs and Torii admission.

### 3. Globally unique domains & normalization

See also: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
for the canonical Norm v1 pipeline used across Torii, the data model, and SDKs.

Redefine `DomainId` as a tagged tuple:

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` wraps the existing Name for domains managed by the current chain.
When a domain is registered through the global registry, we persist the owning
chain’s discriminant. Display / parsing stays unchanged for now, but the
expanded structure allows routing decisions.

#### 3.1 Normalization & spoofing defenses

Norm v1 defines the canonical pipeline every component must use before a domain
name is persisted or embedded into an `AccountAddress`. The full walkthrough
lives in [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md);
the summary below captures the steps that wallets, Torii, SDKs, and governance
tools must implement.

1. **Input validation.** Reject empty strings, whitespace, and the reserved
   delimiters `@`, `#`, `$`. This matches the invariants enforced by
   `Name::validate_str`.
2. **Unicode NFC composition.** Apply ICU-backed NFC normalisation so canonically
   equivalent sequences collapse deterministically (e.g., `e\u{0301}` → `é`).
3. **UTS-46 normalisation.** Run the NFC output through UTS‑46 with
   `use_std3_ascii_rules = true`, `transitional_processing = false`, and
   DNS-length enforcement enabled. The result is a lower-case A-label sequence;
   inputs that violate STD3 rules fail here.
4. **Length limits.** Enforce the DNS-style bounds: each label MUST be 1–63
   bytes and the full domain MUST NOT exceed 255 bytes after step 3.
5. **Optional confusable policy.** UTS‑39 script checks are tracked for
   Norm v2; operators can enable them early, but failing the check must abort
   processing.

If every stage succeeds, the lower-case A-label string is cached and used for
address encoding, configuration, manifests, and registry lookups. Local digest
selectors derive their 12-byte value as `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` using the step 3 output. All other attempts (mixed
case, upper-case, raw Unicode input) are rejected with structured
`ParseError`s at the boundary where the name was supplied.

Canonical fixtures demonstrating these rules — including punycode round-trips
and invalid STD3 sequences — are listed in
`docs/source/references/address_norm_v1.md` and are mirrored in the SDK CI
vector suites tracked under ADDR‑2.

### 4. Nexus domain registry & routing

- **Registry schema:** Nexus maintains a signed map `DomainName -> ChainRecord`
  where `ChainRecord` includes the chain discriminant, optional metadata (RPC
  endpoints), and a proof of authority (e.g., governance multi-signature).
- **Sync mechanism:**
  - Chains submit signed domain claims to Nexus (either during genesis or via
    governance instruction).
  - Nexus publishes periodic manifests (signed JSON plus optional Merkle root)
    over HTTPS and content-addressed storage (e.g., IPFS). Clients pin the
    latest manifest and verify signatures.
- **Lookup flow:**
  - Torii receives a transaction referencing `DomainId`.
  - If the domain is unknown locally, Torii queries the cached Nexus manifest.
  - If the manifest indicates a foreign chain, the transaction is rejected with
    a deterministic `ForeignDomain` error and the remote chain info.
  - If the domain is missing from Nexus, Torii returns `UnknownDomain`.
- **Trust anchors & rotation:** Governance keys sign manifests; rotation or
  revocation is published as a new manifest entry. Clients enforce manifest
  TTLs (e.g., 24h) and refuse to consult stale data beyond that window.
- **Failure modes:** If manifest retrieval fails, Torii falls back to cached
  data within TTL; past TTL it emits `RegistryUnavailable` and refuses
  cross-domain routing to avoid inconsistent state.

### 4.1 Registry immutability, aliases, and tombstones (ADDR-7c)

Nexus publishes an **append-only manifest** so every domain or alias assignment
can be audited and replayed. Operators must treat the bundle described in the
[address manifest runbook](source/runbooks/address_manifest_ops.md) as the
sole source of truth: if a manifest is missing or fails validation, Torii must
refuse to resolve the affected domain.

Automation support: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
replays the checksum, schema, and previous-digest checks spelled out in the
runbook. Include the command output in change tickets to show the `sequence`
and `previous_digest` linkage was validated before publishing the bundle.

#### Manifest header & signature contract

| Field | Requirement |
|-------|-------------|
| `version` | Currently `1`. Bump only with a matching spec update. |
| `sequence` | Increment by **exactly** one per publication. Torii caches refuse revisions with gaps or regressions. |
| `generated_ms` + `ttl_hours` | Establish cache freshness (default 24 h). If the TTL expires before the next publication, Torii flips to `RegistryUnavailable`. |
| `previous_digest` | BLAKE3 digest (hex) of the prior manifest body. Verifiers recompute it with `b3sum` to prove immutability. |
| `signatures` | Manifests are signed via Sigstore (`cosign sign-blob`). Ops must run `cosign verify-blob --bundle manifest.sigstore manifest.json` and enforce the governance identity/issuer constraints before rollout. |

The release automation emits `manifest.sigstore` and `checksums.sha256`
alongside the JSON body. Keep the files together when mirroring to SoraFS or
HTTP endpoints so auditors can replay the verification steps verbatim.

#### Entry types

| Type | Purpose | Required fields |
|------|---------|-----------------|
| `global_domain` | Declares that a domain is registered globally and should map to a chain discriminant and I105 prefix. | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | Retires an alias/selector permanently. Required when erasing Local‑8 digests or removing a domain. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` entries may optionally include a `manifest_url` or `sorafs_cid`
to point wallets at signed chain metadata, but the canonical tuple remains
`{domain, chain, discriminant/i105_prefix}`. `tombstone` records **must** cite
the selector being retired and the ticket/governance artefact that authorised
the change so the audit trail is reconstructable offline.

#### Alias/tombstone workflow & telemetry

1. **Detect drift.** Use `torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, and
   `torii_address_invalid_total{endpoint,reason}` (rendered in
   `dashboards/grafana/address_ingest.json`) to confirm Local submissions and
   Local-12 collisions stay at zero before proposing a tombstone. The
   per-domain counters let owners prove that only dev/test domains emit Local‑8
   traffic (and that Local‑12 collisions map to known staging domains) while
   includes the **Domain Kind Mix (5m)** panel so SREs can graph how much
   `domain_kind="local12"` traffic remains, and the `AddressLocal12Traffic`
   alert fires whenever production still sees Local-12 selectors despite the
   retirement gate.
2. **Derive canonical digests.** Run
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (or consume `fixtures/account/address_vectors.json` via
   `scripts/account_fixture_helper.py`) to capture the exact `digest_hex`.
  The CLI address tool accepts canonical I105 and canonical `0x…` literals;
  runtime `AccountId` parsers continue to accept canonical I105 only.
  The JSON summary reports the parsed format/domain kind plus canonical
  encodings (I105 and canonical hex) for each input.
  For newline-oriented exports use
  `iroha tools address normalize --input <file>` to rewrite newline-separated
  address lists into canonical I105, canonical hex, or JSON forms.
  When auditors need spreadsheet-friendly evidence, run
  `iroha tools address audit --input <file> --format csv` to emit a CSV summary
  (`input,status,format,domain_kind,…`) that highlights domain kind,
  canonical encodings, and parse failures in the same file.
3. **Append manifest entries.** Draft the `tombstone` record (and the follow-up
   `global_domain` record when migrating to the global registry) and validate
   the manifest with `cargo xtask address-vectors` before requesting signatures.
4. **Verify & publish.** Follow the runbook checklist (hashes, Sigstore,
   sequence monotonicity) before mirroring the bundle to SoraFS. Torii now
   canonicalizes account filters and path literals from canonical I105 input only.
5. **Monitor & rollback.** Keep the Local‑8 and Local‑12 collision panels at
   zero for 30 days; if regressions appear, republish the previous manifest
   only in the affected non-production environment until telemetry stabilises.

All of the steps above are mandatory evidence for ADDR‑7c: manifests without
the `cosign` signature bundle or without matching `previous_digest` values must
be rejected automatically, and operators must attach the verification logs to
their change tickets.

### 5. Wallet & API ergonomics

- **Display defaults:** Wallets show the I105 address (short, checksummed)
  plus the resolved domain as a label fetched from the registry. Domains are
  clearly marked as descriptive metadata that may change, while I105 is the
  stable address.
- **Input canonicalization:** Torii and SDKs accept canonical I105 account IDs only and reject `@domain` suffixes, alias forms, non-canonical/legacy I105 literals, and canonical-hex account literals in runtime parser paths. There is no
  strict-mode toggle; raw phone/email identifiers must be kept off-ledger
  via UAID/opaque mappings.
- **Error prevention:** Wallets parse I105 prefixes and enforce chain-discriminant
  expectations. Chain mismatches trigger hard failures with actionable diagnostics.
- **Codec libraries:** Official Rust, TypeScript/JavaScript, Python, and Kotlin
  libraries provide I105 encoding/decoding plus I105 support to
  avoid fragmented implementations. CAIP-10 conversions are not shipped yet.

#### Accessibility & Safe Sharing Guidance

- Implementation guidance for product surfaces is tracked live in
  `docs/portal/docs/reference/address-safety.md`; reference that checklist when
  adapting these requirements to wallet or explorer UX.
- **Safe sharing flows:** Surfaces that copy or display addresses default to the I105 form and expose an adjacent “share” action that presents both the full string and a QR code derived from the same payload so users can verify the checksum visually or by scanning. When truncation is unavoidable (e.g., small screens), retain the start and end of the string, add clear ellipses, and keep the full address accessible via copy-to-clipboard to prevent accidental clipping.
- **IME safeguards:** Address inputs MUST reject composition artefacts from IME/IME-style keyboards. Enforce ASCII-only entry, present an inline warning when full-width or Kana characters are detected, and offer a plain-text paste zone that strips combining marks before validation so Japanese and Chinese users can disable their IME without losing progress.
- **Screen-reader support:** Provide visually hidden labels (`aria-label`/`aria-describedby`) that describe the leading Base58 prefix digits and chunk the I105 payload into 4- or 8-character groups, so assistive technology reads grouped characters instead of a run-on string. Announce copy/share success via polite live regions and ensure QR previews include descriptive alt text (“I105 address for <alias> on chain 0x02F1”).
- **Single-format usage:** Keep address sharing on canonical I105 only and avoid secondary account-literal formats in wallet/explorer copy flows.

## Implementation Checklist

- **I105 envelope:** Prefix encodes the `chain_discriminant` using the compact
  6-/14-bit scheme from `encode_i105_prefix()`, the body is the canonical bytes
  (`AccountAddress::canonical_bytes()`), and the checksum is the first two bytes
  of Blake2b-512(`b"I105PRE"` || prefix || body). The full payload is Base58-
  encoded via `bs58`.
- **Registry contract:** Signed JSON (and optional Merkle root) publishing
  `{discriminant, i105_prefix, chain_alias, endpoints}` with 24h TTL and
  rotation keys.
- **Domain policy:** ASCII `Name` today; if enabling i18n, apply UTS-46 for
  normalization and UTS-39 for confusable checks. Enforce max label (63) and
  total (255) lengths.
- **Textual helpers:** Ship I105 ↔ canonical I105 codecs in Rust,
  TypeScript/JavaScript, Python, and Kotlin with shared test vectors (CAIP-10
  mappings remain future work).
- **CLI tooling:** Provide a deterministic operator workflow via `iroha tools address convert`
  (see `crates/iroha_cli/src/address.rs`), which accepts canonical I105 literals,
  defaults to I105 output using the Sora Nexus prefix (`753`), enforces prefix
  expectations on parse, and rejects `@domain` suffixes so operator pipelines
  stay on canonical address literals only.
- **Wallet/explorer UX:** Follow the [address display guidelines](source/sns/address_display_guidelines.md)
  shipped with ADDR-6—keep canonical I105 as the single copy/QR payload and
  apply IME-safe input/output handling.
- **Torii integration:** Cache Nexus manifests respecting TTL, emit
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` deterministically, and
  keep strict account-literal parsing canonical-I105-only (reject non-canonical
  forms and any `@domain` suffix) with canonical I105 output.

### Torii response formats

- `GET /v2/accounts` and `POST /v2/accounts/query` emit canonical I105 account
  literals in responses.
- Asset holder listings (`GET /v2/assets/{definition_id}/holders`) and their JSON
  envelope counterpart (`POST …/holders/query`) also emit canonical I105 account
  identifiers in `items[*].account_id`.
- **Testing:** Add unit tests for encoder/decoder round-trips, wrong-chain
  failures, and manifest lookups; add integration coverage in Torii and SDKs
  for I105 flows end to end.

## Error Code Registry

Address encoders and decoders expose failures through
`AccountAddressError::code_str()`. The following tables provide the stable codes
that SDKs, wallets, and Torii surfaces should surface alongside human-readable
messages, plus recommended remediation guidance.

### Canonical Construction

| Code | Failure | Recommended Remediation |
|------|---------|-------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | Encoder received a signing algorithm not supported by the registry or build features. | Restrict account construction to curves enabled in the registry and configuration. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | Signing key payload length exceeds the supported limit. | Single-key controllers are limited to `u8` lengths; use multisig for large public keys (e.g., ML‑DSA). |
| `ERR_INVALID_HEADER_VERSION` | Address header version is outside the supported range. | Emit header version `0` for V1 addresses; upgrade encoders before adopting new versions. |
| `ERR_INVALID_NORM_VERSION` | Normalisation version flag is not recognised. | Use normalisation version `1` and avoid toggling reserved bits. |
| `ERR_INVALID_I105_PREFIX` | Requested I105 network prefix cannot be encoded. | Pick a prefix within the inclusive `0..=16383` range published in the chain registry. |
| `ERR_CANONICAL_HASH_FAILURE` | Canonical payload hashing failed. | Retry the operation; if the error persists, treat it as an internal bug in the hashing stack. |

### Format Decoding and Auto Detection

| Code | Failure | Recommended Remediation |
|------|---------|-------------------------|
| `ERR_INVALID_I105_ENCODING` | I105 string contains characters outside the alphabet. | Ensure the address uses the published I105 alphabet and has not been truncated during copy/paste. |
| `ERR_INVALID_LENGTH` | Payload length does not match the expected canonical size for header/controller (or legacy decode-compat selector variants). | Supply the full canonical payload emitted by the official encoder, or a complete legacy payload when decoding historical data. |
| `ERR_CHECKSUM_MISMATCH` | Canonical I105 checksum validation failed. | Regenerate the canonical I105 address from a trusted source; this typically indicates a copy/paste error. |
| `ERR_INVALID_I105_PREFIX_ENCODING` | I105 prefix bytes are malformed. | Re-encode the address with a compliant encoder; do not alter the leading Base58 bytes manually. |
| `ERR_INVALID_HEX_ADDRESS` | Canonical hexadecimal form failed to decode. | Provide a `0x`-prefixed, even-length hex string produced by the official encoder. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | Legacy/non-canonical I105 form does not start with the expected sentinel. | Use canonical I105 output and avoid manual sentinel rewriting. |
| `ERR_COMPRESSED_TOO_SHORT` | Legacy/non-canonical I105 string is truncated before payload+checksum complete. | Use the full canonical I105 string emitted by the encoder. |
| `ERR_INVALID_COMPRESSED_CHAR` | Legacy/non-canonical I105 payload includes an invalid glyph. | Replace with canonical I105 output generated by official codecs. |
| `ERR_INVALID_COMPRESSED_BASE` | Encoder attempted to use an unsupported legacy radix. | File a bug against the encoder; production flows must stay canonical I105. |
| `ERR_INVALID_COMPRESSED_DIGIT` | Legacy/non-canonical digit value exceeds the supported legacy alphabet size. | Regenerate canonical I105 and avoid manual digit manipulation. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | Auto-detection could not recognise the input format. | Provide I105, I105, or canonical `0x` hex strings when invoking parsers. |

### Domain and Network Validation

| Code | Failure | Recommended Remediation |
|------|---------|-------------------------|
| `ERR_DOMAIN_MISMATCH` | Domain selector does not match the expected domain. | Use an address issued for the intended domain or update the expectation. |
| `ERR_INVALID_DOMAIN_LABEL` | Domain label failed normalisation checks. | Canonicalise the domain using UTS-46 non-transitional processing before encoding. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | Decoded I105 network prefix differs from the configured value. | Switch to an address from the target chain or adjust the expected discriminant/prefix. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Address class bits are not recognised. | Upgrade the decoder to a release that understands the new class, or avoid tampering with the header bits. |
| `ERR_UNKNOWN_DOMAIN_TAG` | Domain selector tag is unknown. | Update to a release that supports the new selector type, or avoid using experimental payloads on V1 nodes. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Reserved extension bit was set. | Clear reserved bits; they remain gated until a future ABI introduces them. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Controller payload tag not recognised. | Upgrade the decoder to recognise new controller types before parsing them. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | Canonical payload contained trailing bytes after decoding. | Regenerate the canonical payload; only the documented length should be present. |

### Controller Payload Validation

| Code | Failure | Recommended Remediation |
|------|---------|-------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Key bytes do not match the declared curve. | Ensure the key bytes are encoded exactly as required for the selected curve (e.g., 32-byte Ed25519). |
| `ERR_UNKNOWN_CURVE` | Curve identifier is not registered. | Use curve ID `1` (Ed25519) until additional curves are approved and published in the registry. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig controller declares more members than supported. | Reduce the multisig membership to the documented limit before encoding. |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig policy payload failed validation (threshold/weights/schema). | Rebuild the policy so that it satisfies the CTAP2 schema, weight bounds, and threshold constraints. |

## Alternatives Considered

- **Pure Base58Check (Bitcoin-style).** Simpler checksum but weaker error detection
  than the Blake2b-derived I105 checksum (`encode_i105` truncates a 512-bit hash)
  and lacks explicit prefix semantics for 16-bit discriminants.
- **Embedding chain name in the domain string (e.g., `finance@chain`).** Breaks
- **Rely solely on Nexus routing without changing addresses.** Users would still
  copy/paste ambiguous strings; we want the address itself to carry context.
- **Bech32m envelope.** QR-friendly and offers a human-readable prefix, but
  would diverge from the shipping I105 implementation (`AccountAddress::to_i105`)
  and require recreating all fixtures/SDKs. The current roadmap keeps I105 +
  I105 support while continuing research into future
  Bech32m/QR layers (CAIP-10 mapping is deferred).

## Open Questions

- Confirm that `u16` discriminants plus reserved ranges cover long-term demand;
  otherwise evaluate `u32` with varint encoding.
- Finalize the multi-signature governance process for registry updates and how
  revocations/expired allocations are handled.
- Define the exact manifest signature scheme (e.g., Ed25519 multi-sig) and
  transport security (HTTPS pinning, IPFS hash format) for Nexus distribution.
- Determine whether to support domain aliases/redirects for migrations and how
  to surface them without breaking determinism.
- Specify how Kotodama/IVM contracts access I105 helpers (`to_address()`,
  `parse_address()`) and whether on-chain storage should ever expose CAIP-10
  mappings (today I105 is canonical).
- Explore registering Iroha chains in external registries (e.g., I105 registry,
  CAIP namespace directory) for broader ecosystem alignment.

## Next Steps

1. I105 encoding landed in `iroha_data_model` (`AccountAddress::to_i105`,
   `parse_encoded`); continue porting fixtures/tests to every SDK and purge any
   Bech32m placeholders.
2. Extend configuration schema with `chain_discriminant` and derive sensible
  defaults for existing test/dev setups. **(Done: `common.chain_discriminant`
  now ships in `iroha_config`, defaulting to `0x02F1` with per-network
  overrides.)**
3. Draft the Nexus registry schema and proof-of-concept manifest publisher.
4. Collect feedback from wallet providers and custodians on human-factor aspects
   (HRP naming, display formatting).
5. Update documentation (`docs/source/data_model.md`, Torii API docs) once the
   implementation path is committed.
6. Ship official codec libraries (Rust/TS/Python/Kotlin) with normative test
   vectors covering success and failure cases.
