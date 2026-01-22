<!-- TODO: Translation pending; content synced from English for technical accuracy. -->

# Account Structure RFC

**Status:** Accepted (ADDR-1)  
**Audience:** Data model, Torii, Nexus, Wallet, Governance teams  
**Related issues:** TBD

## Summary

This document describes the shipping account-addressing stack implemented in
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) and the
companion tooling. It provides:

- A checksummed, human-facing **Iroha Base58 address (IH58)** produced by
  `AccountAddress::to_ih58` that binds a chain discriminant to the account
  controller and offers deterministic interop-friendly textual forms.
- Domain selectors for implicit default domains and local digests, with a
  reserved global-registry selector tag for future Nexus-backed routing (the
  registry lookup is **not yet shipped**).

## Motivation

Wallets and off-chain tooling rely on raw `alias@domain` routing aliases today. This
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

- Describe the IH58 Base58 envelope implemented in the data model and the
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
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical IH58 literal (no `@domain` suffix)
Parse accepts:
- IH58 (preferred), `snx1` compressed, or canonical hex (`0x...`) inputs, with
  optional `@<domain>` suffixes for explicit routing hints.
- `<label>@<domain>` aliases resolved through the account-alias resolver
  (Torii installs one; plain data-model parsing requires a resolver to be set).
- `<public_key>@<domain>` where `public_key` is the canonical multihash string.
- `uaid:<hex>` / `opaque:<hex>` literals resolved via UAID/opaque resolvers.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
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

The Rust data model exposes a single canonical payload representation
(`AccountAddress`) that can be emitted as several human-facing formats. IH58 is
the preferred account format for sharing and canonical output; the compressed
`snx1` form is a second-best, Sora-only option for UX where the kana alphabet
adds value. Canonical hex remains a debugging aid.

- **IH58 (Iroha Base58)** – a Base58 envelope that embeds the chain
  discriminant. Decoders validate the prefix before promoting the payload to
  the canonical form.
- **Sora-compressed view** – a Sora-only alphabet of **105 symbols** built by
  appending the half-width イロハ poem (including ヰ and ヱ) to the 58-character
  IH58 set. Strings start with the sentinel `snx1`, embed a Bech32m-derived
  checksum, and omit the network prefix (Sora Nexus is implied by the sentinel).

  ```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex** – a debugging-friendly `0x…` encoding of the canonical byte
  envelope.

`AccountAddress::parse_any` auto-detects IH58 (preferred), compressed (`snx1`, second-best), or canonical hex
(`0x...` only; bare hex is rejected) inputs and returns both the decoded payload and the detected
`AccountAddressFormat`. Torii now calls `parse_any` for ISO 20022 supplementary
addresses and stores the canonical hex form so metadata remains deterministic
regardless of the original representation.

#### 2.1 Header byte layout (ADDR-1a)

Every canonical payload is laid out as `header · domain selector · controller`. The
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

#### 2.2 Domain selector encodings (ADDR-1a)

The domain selector immediately follows the header and is a tagged union:

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Implicit default domain | none | Matches the configured `default_domain_name()`. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
domain selector
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

The selector is immediately adjacent to the controller payload, so a decoder can walk
the wire format in order: read the tag byte, read the tag-specific payload, then move on
to the controller bytes.

**Selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  Breakdown: `0x02` header, `0x02` selector tag, `00 00 00 2A` registry id, `0x00`
  controller tag, `0x01` curve id, `0x20` key length, 32-byte Ed25519 key payload.

#### 2.3 Controller payload encodings (ADDR-1a)

The controller payload is another tagged union appended after the domain selector:

| Tag | Controller | Layout | Notes |
|-----|------------|--------|-------|
| `0x00` | Single key | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` maps to Ed25519 today. `key_len` is bounded to `u8`; larger values raise `AccountAddressError::KeyPayloadTooLong` (so single-key ML‑DSA public keys, which are >255 bytes, cannot be encoded and must use multisig). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | Supports up to 255 members (`CONTROLLER_MULTISIG_MEMBER_MAX`). Unknown curves raise `AccountAddressError::UnknownCurve`; malformed policies bubble up as `AccountAddressError::InvalidMultisigPolicy`. |

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
  `version:u8`, `threshold:u16`, `member_count:u8`, then each member’s
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. This is exactly what
  `AccountAddress::canonical_bytes()` writes to IH58 (preferred)/snx1 (second-best) payloads.
- Hashing (`MultisigPolicy::digest_blake2b256()`) uses Blake2b-256 with the
  `iroha-ms-policy` personalization string so governance manifests can bind to a
  deterministic policy ID that matches the controller bytes embedded in IH58.
- Fixture coverage lives in `fixtures/account/address_vectors.json` (cases
  `addr-multisig-*`). Wallets and SDKs should assert the canonical IH58 strings
  below to confirm their encoders match the Rust implementation.

| Case ID | Threshold / members | IH58 literal (prefix `0x02F1`) | Sora compressed (`snx1`) literal | Notes |
|---------|---------------------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` weight, members `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `snx13vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Council-domain governance quorum. |
| `addr-multisig-wonderland-threshold2` | `≥2`, members `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `snx12ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Dual-signature wonderland example (weight 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, members `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `snx1ﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Implicit-default domain quorum used for base governance.

#### 2.4 Failure rules (ADDR-1a)

- Payloads shorter than the required header + selector or with leftover bytes emit `AccountAddressError::InvalidLength` or `AccountAddressError::UnexpectedTrailingBytes`.
- Headers that set the reserved `ext_flag` or advertise unsupported versions/classes MUST be rejected using `UnexpectedExtensionFlag`, `InvalidHeaderVersion`, or `UnknownAddressClass`.
- Unknown selector/controller tags raise `UnknownDomainTag` or `UnknownControllerTag`.
- Oversized or malformed key material raises `KeyPayloadTooLong` or `InvalidPublicKey`.
- Multisig controllers exceeding 255 members raise `MultisigMemberOverflow`.
- IME/NFKC conversions: half-width Sora kana can be normalised to their full-width forms without breaking decoding, but the ASCII `snx1` sentinel and IH58 digits/letters MUST stay ASCII. Full-width or case-folded sentinels surface `ERR_MISSING_COMPRESSED_SENTINEL`, full-width ASCII payloads raise `ERR_INVALID_COMPRESSED_CHAR`, and checksum mismatches bubble up as `ERR_CHECKSUM_MISMATCH`. Property tests in `crates/iroha_data_model/src/account/address.rs` cover these paths so SDKs and wallets can rely on deterministic failures.
- Torii and SDK parsing of `address@domain` aliases now emit the same `ERR_*` codes when IH58 (preferred)/snx1 (second-best) inputs fail before alias fallback (e.g., checksum mismatch, domain digest mismatch), so clients can relay structured reasons without guessing from prose strings.
- Local selector payloads shorter than 12 bytes surface `ERR_LOCAL8_DEPRECATED`, preserving a hard cutover from legacy Local‑8 digests.
- Domainless IH58 (preferred)/snx1 (second-best) literals resolve the embedded selector via the domain-selector resolver; if none is installed (or the selector cannot be resolved) parsing fails with `ERR_DOMAIN_SELECTOR_UNRESOLVED`. The implicit default selector resolves to the configured default domain label without requiring a resolver.

#### 2.5 Normative binary vectors

- **Implicit default domain (`default`, seed byte `0x00`)**  
  Canonical hex: `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  Breakdown: `0x02` header, `0x00` selector (implicit default), `0x00` controller tag, `0x01` curve id (Ed25519), `0x20` key length, followed by the 32-byte key payload.
- **Local domain digest (`treasury`, seed byte `0x01`)**  
  Canonical hex: `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.  
  Breakdown: `0x02` header, selector tag `0x01` plus digest `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, followed by the single-key payload (`0x00` tag, `0x01` curve id, `0x20` length, 32-byte Ed25519 key).

Unit tests (`account::address::tests::parse_any_accepts_all_formats`) assert the V1 vectors below via `AccountAddress::parse_any`, guaranteeing that tooling can rely on the canonical payload across hex, IH58 (preferred), and compressed (`snx1`, second-best) forms. Regenerate the extended fixture set with `cargo run -p iroha_data_model --example address_vectors`.

| Domain      | Seed byte | Canonical hex                                                                 | Compressed (`snx1`) |
|-------------|-----------|-------------------------------------------------------------------------------|------------|
| default     | `0x00`    | `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201` | `snx12QGﾈkﾀｱﾚiﾉﾘuﾛWRヱﾏxﾁSuﾁepnhｽvｶrﾓｶ9Tｹｿp3ﾇVWｳｲｾU4N5E5` |
| treasury    | `0x01`    | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8` | `snx15ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾘｻYﾃhﾓMQ9CBEﾅﾊﾈｷﾉVRｺnKRwTﾋｼqﾅWrﾎU7ｼiﾍQt1TPGNJ` |
| wonderland  | `0x02`    | `0x0201b8ae571b79c5a80f5834da2b000120ad29ac2c12d4daaa4a2415235f2b01730bff1193dd4a6eaee29e945b01a4a212` | `snx15ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓRKSｷﾗﾕneﾀM3Rabvﾂ1JﾚﾉｺｲﾕｹｺﾀFﾔﾇﾖFSXsﾜCHmB59S5KS` |
| iroha       | `0x03`    | `0x0201de8b36819700c807083608e2000120ce6d4f240893505e112cdc1b83585d8efc271ea6f934c5f6a49217e27e61b9e7` | `snx15ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓsYヰxﾎﾍﾚﾇｺﾊehjyzXGヰaｿSﾔ1kWｺｾJeﾒAWkwﾋﾐRRQQKXYE` |
| alpha       | `0x04`    | `0x020146be2154ae86826a3fef0ec000012077143459c5b54808313cd57ded18322fc02c4616de930e0e3af578bb509bb5dc` | `snx15ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRSﾗgPﾏrHﾔGﾀqﾂｹfoﾂHwﾉoﾊ4ﾎﾇ74ｼﾕﾎUw8JaU3ﾙJFYHVLUS` |
| omega       | `0x05`    | `0x0201390d946885bc8416b3d30c9d000120e18cbb31e5249ff9205b72fe50e50dcc78fb80e28028bdc4c47bcf63ee61c6b8` | `snx15ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8qfBﾌﾒaTjQpTxPﾊｦNﾚnヱvorHｷﾎkﾈEヱFﾎｻTUﾗhiVqURKRVM` |
| governance  | `0x06`    | `0x0201989eb45a80940d187e2c908f0001208a5bd65d39ba61bde2a87ee10d242bd5575cd02bf687c4b5960d4141698dd76a` | `snx15ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾌbﾜｸeGｵzﾙzｽﾐﾌﾉQﾃw2ﾕLDﾔｽﾙFﾙﾇﾋBTdUXﾎﾙsｽRDJCHS` |
| validators  | `0x07`    | `0x0201e4ffa58704c69afaeb7cc2d7000120f0f80d8a09aa1276d2e605bc904137f7a52b9c4847b9b5366d4002ca4049daeb` | `snx15ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊWTﾂfKｳmU7fWﾍXｱ2JnyﾋE4ﾕghZｱVｶｦﾇaFﾕbr8qﾑR4VEFR` |
| explorer    | `0x08`    | `0x02013b35422c65c2a83c99c523ad00012033af4073c5815cbe5d0fec37cffe02e542302b60e24d8a7c0819f772ca6886f9` | `snx15ｻ4nmｻaﾚﾚPvNLgｿｱv6MHdPﾓWﾍヱpeﾕFﾕmFﾌﾀKhﾉWｴeﾋbｷXMﾎ2ﾃnQﾐﾗﾑﾎBBｻC8P548` |
| soranet     | `0x09`    | `0x0201047d9ea7f5d5dbec3f7bfc58000120cd3c119f6c81e28a2747c531f5cbe8dbc44ed8e16751bc4a467445b272db4097` | `snx15ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvwS8JｳEnQaｿHTdﾒXZﾍvﾆazｿgﾔﾙhF9hcsﾘvNﾌヱJ9MGDNBW` |
| kitsune     | `0x0A`    | `0x0201e91933de397fd7723dc9a76c0001206e4c4188e1b8455ff3369dc163240a5d653f13a6f420fd0edbb23303bad239e7` | `snx15ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpzﾘKmC6ｱSﾇhqｦJB1gﾙwCwﾁﾍeﾉﾔABﾆpqYﾍEﾌｼﾆﾃFNCAT97` |
| da          | `0x0B`    | `0x02016838cf5bb0ce0f3d4f380e1c00012056f02721c153689b09efafd07d8ef7bed2c4a581dd00faa118aed1d51f7a1ad6` | `snx15ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKmgﾁﾃｻ3ｸGLpﾄﾆHnXGﾘﾑDﾃJ6ｸXﾐﾂﾊwhｶtｵｾｴﾃ9PﾖDFC3YQ` |

Reviewed-by: Data Model WG, Cryptography WG — scope approved for ADDR-1a.

##### Sora Nexus reference aliases

Sora Nexus networks default to `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). The
`AccountAddress::to_ih58` and `to_compressed_sora` helpers therefore emit
consistent textual forms for every canonical payload. Selected fixtures from
`fixtures/account/address_vectors.json` (generated via
`cargo xtask address-vectors`) are shown below for quick reference:

| Account / selector | IH58 literal (prefix `0x02F1`) | Sora compressed (`snx1`) literal |
|--------------------|--------------------------------|-------------------------|
| `default` domain (implicit selector, seed `0x00`) | `RnuaJGGDL9CghX9U4iqYRMghp31xkGuCvqQTzXu9AF8kzt7etZdZeGqS` | `snx12QGﾈkﾀｱﾚiﾉﾘuﾛWRヱﾏxﾁSuﾁepnhｽvｶrﾓｶ9Tｹｿp3ﾇVWｳｲｾU4N5E5` (optional `@default` suffix when providing explicit routing hints) |
| `treasury` (local digest selector, seed `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `snx15ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾘｻYﾃhﾓMQ9CBEﾅﾊﾈｷﾉVRｺnKRwTﾋｼqﾅWrﾎU7ｼiﾍQt1TPGNJ` |
| Global registry pointer (`registry_id = 0x0000_002A`, equivalent to `treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `snx1kXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

These strings match the ones emitted by the CLI (`iroha address convert`), Torii
responses (`address_format=ih58|compressed`), and SDK helpers, so UX copy/paste
flows can rely on them verbatim. Append `<address>@<domain>` only when you need an explicit routing hint; the suffix is not part of the canonical output.

#### 2.6 Textual aliases for interoperability (planned)

- **Chain-alias style:** `ih:<chain-alias>:<alias@domain>` for logs and human
  entry. Wallets must parse the prefix, verify the embedded chain, and block
  mismatches.
- **CAIP-10 form:** `iroha:<caip-2-id>:<ih58-addr>` for chain-agnostic
  integrations. This mapping is **not yet implemented** in the shipped
  toolchains.
- **Machine helpers:** Publish codecs for Rust, TypeScript/JavaScript, Python,
  and Kotlin covering IH58 and compressed formats (`AccountAddress::to_ih58`,
  `AccountAddress::parse_any`, and their SDK equivalents). CAIP-10 helpers are
  future work.

#### 2.7 Deterministic IH58 alias

- **Prefix mapping:** Reuse the `chain_discriminant` as the IH58 network prefix.
  `encode_ih58_prefix()` (see `crates/iroha_data_model/src/account/address.rs`)
  emits a 6‑bit prefix (single byte) for values `<64` and a 14‑bit, two-byte
  form for larger networks. The authoritative assignments live in
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDKs MUST keep the matching JSON registry in sync to avoid collisions.
- **Account material:** IH58 encodes the canonical payload built by
  `AccountAddress::canonical_bytes()`—header byte, domain selector, and
  controller payload. There is no additional hashing step; IH58 embeds the
  binary controller payload (single key or multisig) as produced by the Rust
  encoder, not the CTAP2 map used for multisig policy digests.
- **Encoding:** `encode_ih58()` concatenates the prefix bytes with the canonical
  payload and appends a 16-bit checksum derived from Blake2b-512 with the fixed
  prefix `IH58PRE` (`b"IH58PRE" || prefix || payload`). The result is Base58-encoded via `bs58`.
  CLI/SDK helpers expose the same procedure, and `AccountAddress::parse_any`
  reverses it via `decode_ih58`.

#### 2.8 Normative textual test vectors

`fixtures/account/address_vectors.json` contains full IH58 (preferred) and compressed (`snx1`, second-best)
literals for every canonical payload. Highlights:

- **`addr-single-default-ed25519` (Sora Nexus, prefix `0x02F1`).**  
  IH58 `RnuaJGGDL9CghX9U4iqYRMghp31xkGuCvqQTzXu9AF8kzt7etZdZeGqS`, compressed (`snx1`)
  `snx12QG…U4N5E5`. Torii emits these exact strings from `AccountId`’s
  `Display` implementation (canonical IH58) and `AccountAddress::to_compressed_sora`.
- **`addr-global-registry-002a` (registry selector → treasury).**  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, compressed (`snx1`)
  `snx1kX…CM6AEP`. Demonstrates that registry selectors still decode to
  the same canonical payload as the corresponding local digest.
- **Failure case (`ih58-prefix-mismatch`).**  
  Parsing an IH58 literal encoded with prefix `NETWORK_PREFIX + 1` on a node
  expecting the default prefix yields
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  before domain routing is attempted. The `ih58-checksum-mismatch` fixture
  exercises tampering detection over the Blake2b checksum.

#### 2.9 Compliance fixtures

ADDR‑2 ships a replayable fixture bundle covering positive and negative
scenarios across canonical hex, IH58 (preferred), compressed (`snx1`, half-/full-width), implicit
default selectors, global registry aliases, and multisignature controllers. The
canonical JSON lives in `fixtures/account/address_vectors.json` and can be
regenerated with:

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
| `global_domain` | Declares that a domain is registered globally and should map to a chain discriminant and IH58 prefix. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | Retires an alias/selector permanently. Required when erasing Local‑8 digests or removing a domain. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` entries may optionally include a `manifest_url` or `sorafs_cid`
to point wallets at signed chain metadata, but the canonical tuple remains
`{domain, chain, discriminant/ih58_prefix}`. `tombstone` records **must** cite
the selector being retired and the ticket/governance artefact that authorised
the change so the audit trail is reconstructable offline.

#### Alias/tombstone workflow & telemetry

1. **Détecter la dérive.** Utiliser
   `torii_address_local8_total{endpoint}` et
   `torii_address_invalid_total{endpoint,reason}`
   (affichés dans `dashboards/grafana/address_ingest.json`) pour confirmer que
   les chaînes Local‑8 ne sont plus acceptées en production avant de proposer
   un `tombstone`.
2. **Dériver les digests canoniques.** Lancer
   `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753`
   (ou consommer `fixtures/account/address_vectors.json` via
   `scripts/account_fixture_helper.py`) pour capturer exactement le champ
   `digest_hex`. Le CLI accepte des entrées comme `snx1...@wonderland` ; le
   résumé JSON expose le domaine via `input_domain` et l’option
   `--append-domain` rejoue l’encodage converti sous la forme
   `<ih58>@wonderland` pour la mise à jour du manifest. Pour des exports
   ligne‑par‑ligne, utiliser
   afin de convertir massivement des sélecteurs Local en formes IH58
   canoniques (ou compressées/hex/JSON) en ignorant les lignes non locales.
   Pour une preuve exploitable sous forme de feuille de calcul, exécuter
   pour obtenir un CSV (`input,status,format,domain_kind,…`) mettant en
   évidence sélecteurs locaux, encodages canoniques et échecs de parse.
3. **Ajouter les entrées au manifest.** Préparer l’entrée `tombstone` (et le
   `global_domain` correspondant lors de la migration vers le registre global)
   et valider le manifest avec `cargo xtask address-manifest verify` avant de
   demander les signatures.
4. **Vérifier et publier.** Suivre la checklist du runbook (hashes, Sigstore,
   monotonie de `sequence`) avant de répliquer le bundle vers SoraFS. Torii
   les clusters de production exigent immédiatement des littéraux
   IH58/compressés canoniques après la mise à jour du bundle.
5. **Surveiller et, si besoin, revenir en arrière.** Garder les panneaux
   Local‑8 à zéro pendant 30 jours ; en cas de régression, republier le bundle
   de manifests précédent et, uniquement sur les environnements non
   stabilisation de la télémétrie.
