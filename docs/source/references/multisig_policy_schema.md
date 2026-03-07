<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Multisignature Controller Schema v1 (ADDR‑1c)

This note is the canonical reference for the multisignature controller payload
used by the Sora Name Service roadmap (**ADDR‑1c**). It formalises the CTAP2
CBOR map, documents the validation rules enforced by
`MultisigPolicy::validate()`, and records the golden fixtures that SDKs and
operations teams rely on when reproducing controller hashes.

## Deterministic CTAP2 map

`AccountController::Multisig` policies are serialised as a CTAP2-style CBOR map
with integer keys. The layout mirrors the on‑chain payload; the CBOR form is
used solely for hashing/signing and never appears inside the address envelope.

- Maps are definite-length (`0xA3`) and omit optional fields entirely.
- Integer fields are encoded big-endian using the smallest width that fits
  (`cbor_write_unsigned` in `crates/iroha_data_model/src/account/controller.rs`).
- Member maps are emitted in canonical order; duplicate keys are rejected
  during validation prior to serialisation.

The structure can be expressed in CDDL as:

```text
ms-policy = {
  1: ms-version,   ; always 1 until a new policy version rolls out
  2: ms-threshold, ; approval weight required (uint16)
  3: [* ms-member] ; definite-length array
}
ms-version = 1
ms-threshold = uint .size 2

ms-member = {
  1: curve-id,          ; matches docs/source/references/address_curve_registry.md
  2: member-weight,     ; uint16 > 0
  3: bytes .size (1..=256) ; raw PublicKey payload
}
curve-id = uint .size 1
member-weight = uint .size 2
```

`MultisigPolicy::encode_ctap2()` is the only supported encoder and is the source
of truth for the map layout.

## Validation invariants

Policies are normalised and validated before they can be embedded into an
account address:

| Rule | Notes / source | Failure mode |
|------|----------------|--------------|
| `version == MultisigPolicy::CURRENT_VERSION (1)` | Ensures all nodes agree on semantics before accepting a policy. | `MultisigPolicyError::UnsupportedVersion` |
| `1 <= threshold <= Σ member.weight` | Enforcement happens after deduplication so callers cannot bypass the check by repeating members. | `ZeroThreshold` or `ThresholdExceedsTotal` |
| `1 <= members.len() <= CONTROLLER_MULTISIG_MEMBER_MAX (255)` | `CONTROLLER_MULTISIG_MEMBER_MAX` is shared with the binary controller encoding documented in `docs/account_structure.md`. | `EmptyMembers` or `AccountAddressError::MultisigMemberOverflow` |
| Member weight ≥ 1 | Enforced inside `MultisigMember::new`. | `MemberWeightZero` |
| Curves must exist in `address_curve_registry.md` **and** be enabled in `crypto.allowed_signing` | Guarantees deterministic rejection when future curves are advertised but not enabled on a cluster. | `UnsupportedCurve` / `AccountAddressError::UnknownCurve` |
| Members are deduplicated after canonical sorting by `(algorithm_string || 0x00 || key_bytes)` | Prevents equivalent public keys from inflating total weight; the canonical order feeds directly into CTAP2 encoding. | `DuplicateMember` |

These invariants ensure every controller hash is deterministic regardless of the
host language. SDKs must surface the same validation errors so users receive
early feedback before Torii admission rejects the payload.

## Controller registration

`MultisigRegister` still requires callers to supply the `account` field, but the
on-chain controller id is derived from the multisig spec. Registration rekeys
the supplied account to the canonical `AccountController::Multisig` identifier
computed from the spec once validation succeeds.

- Tooling may still mint a fresh keypair for the `account` field and discard the
  private key, because multisig controllers never sign transactions directly.
- Newly registered controllers persist `multisig/spec` metadata and are rekeyed
  to the canonical multisig account id.
- JSON decoding errors when the `account` field is omitted, so clients must send
  it even though the final account id is deterministic.
- Signatories must be single-key accounts; nested multisig controllers are
  rejected.

## Deterministic digest

To sign or compare policies, hosts compute a Blake2b‑256 MAC over the CTAP2
payload with an empty key/salt and the personalisation string
`"iroha-ms-policy"`. The implementation in
`MultisigPolicy::digest_blake2b256()` is:

```rust
pub fn digest_blake2b256(&self) -> [u8; 32] {
    let encoded = self.encode_ctap2();
    let mut mac = Blake2bMac::<U32>::new_with_salt_and_personal(
        &[],
        &[],
        b"iroha-ms-policy",
    )
    .expect("personalised Blake2b parameters must be valid");
    Mac::update(&mut mac, &encoded);
    mac.finalize().into_bytes().into()
}
```

The digest is stable across languages as long as the CBOR encoder and
validation invariants above are respected. Empty-personalisation Blake2b
hashes are rejected explicitly to avoid accidental data drift.

## Golden policies

Three deterministic policies ship with the address compliance vectors and offer
fixture coverage for both the CTAP2 payloads and the digests. The canonical JSON
lives in `fixtures/account/address_vectors.json`.

| Case ID | Domain | Threshold | Members (curve, weight) | `ctap2_cbor_hex` | `digest_blake2b256_hex` |
|---------|--------|-----------|-------------------------|------------------|-------------------------|
| `addr-multisig-council-threshold3` | `council` | 3 | `(1,2) · (1,1) · (1,1)` | `0xA3010102030383A301010201035820591B509F4A1B29C9D8CADA0F876EE97041CBD43C0DE3251B1E6C77BB621F40B0A30101020203582062FFA8DE6DA654EFEBFA3D80FDEF8F188034CFA3E211859346E41A5FEA06532BA301010201035820D4ABAFC1F385826A8EBC60DF90568C52CCFBF91E0B134DC66E42731A7561E4DA` | `0x24E6E8231BC3DCF93DA43F2ABAE739419340B0D532F671808A59239CB2E7A4D6` |
| `addr-multisig-wonderland-threshold2` | `wonderland` | 2 | `(1,1) · (1,2)` | `0xA3010102020382A301010201035820590E3F8A4263537D1AECA330EDAADB5722597ACD54807B3B1C5A7F2CCAB13D45A3010102020358208C9C7A57B25A194205E227E1FE67780A0E8D54C9AEC7ABD9439CF69B5CFCCCB5` | `0x13F157FB9400EF6B096A5D50EE1AAD30E89EF523C22FCF845631F21089130F12` |
| `addr-multisig-default-quorum3` | `default` (selector-free canonical payload) | 3 | `(1,1) · (1,1) · (1,1) · (1,1)` | `0xA3010102030384A3010102010358200DCE150576DC9D2677EBC1B5EE02C019CA69EB1892732157F7D551D2F83976C4A30101020103582032708B35523549F429E95F7AF11F7C8320B9BA29163D2F27AF7D81C1C5991041A301010201035820683BA155CD7E73B91FA9B0E61722DF50AE625E7F5DA9CF5013162891439C2C3FA301010201035820BEF1E6A19FC053B29945A3F1D2A4759B5297F9ADD0A5C3961405804EFD1780BB` | `0xD596346A497D45B9954EEE3BC71C82C8D4B33E9AC9A8A226D626C45891CD2378` |

Consumers should assert both strings when verifying their own encoders; the
fixtures back the Rust, JS, Swift, and Android SDK tests as well as the Torii
admission suites.

## Verification workflow

| Step | Command | Purpose |
|------|---------|---------|
| Generate/verify fixtures | `cargo xtask address-vectors --out fixtures/account/address_vectors.json` or `cargo xtask address-vectors --verify` | Regenerates the canonical JSON, including the multisig CTAP2 payloads and digests, or checks that the committed file matches the generator. |
| Inspect payloads ad hoc | `cargo run -p iroha_data_model --example account_address_vectors | jq '.vectors[] | select(.category == "multisig")'` | Dumps the same JSON without writing to disk for quick experiments. |
| Host validation tests | `crates/iroha_data_model/tests/account_address_vectors.rs` | Ensures `AccountController` parsing + multisig hashing match the fixtures. |

## References

- Binary controller encoding: [`docs/account_structure.md`](../../account_structure.md#23-controller-payload-encodings-addr-1a)
- Canonical curve registry: [`docs/source/references/address_curve_registry.md`](address_curve_registry.md)
- Implementation: [`crates/iroha_data_model/src/account/controller.rs`](../../../crates/iroha_data_model/src/account/controller.rs)
- Compliance fixtures: [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json)
