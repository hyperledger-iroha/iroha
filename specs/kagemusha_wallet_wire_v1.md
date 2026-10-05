# KAGEMUSHA wallet wire record V1

**Status.** This record describes the canonical G1 wire objects of the single
KAGEMUSHA split-lineage design ([proposal](kagemusha_single_design_proposal.md)
§§3, 3.1, 3.2, 4.1, 5.1 and 8; §10, G1). They are implemented in
`iroha_data_model::kagemusha::kagemusha_wallet_v1`
(`crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1.rs` and its child
files). Their cross-language vectors are `fixtures/kagemusha/wallet_v1_vectors.json`.
Kotlin `org.hyperledger.iroha.sdk.offline.KagemushaWalletWireV1` (`kotlin/core-jvm`)
and Swift `KagemushaWalletWireV1` (`IrohaSwift`) consume the vectors, and the Swift and
Kotlin peer carriers move envelope frames, the Lineage message included (§6). The Durable
State Provider binds the operation-dependent `proof_digest` (§3.2); the protocol
relations, the bridge, the ledger instructions and the Torii routes do not use these
objects yet. Proof bytes, relation bindings, empty-map roots and lineage roots in the
vectors are labelled stand-ins. The code is authoritative; a change to it updates this
record and the vectors together. The exception is a rule marked **Decided (TODO(G1))**:
an owner decision of 2026-10-05 (proposal revision 2026-10-05) that the code and
vectors do not implement yet. There the proposal governs; the Rust step implements it,
pins its element lists, transcript lengths, Poseidon domains, byte packing and vectors
in this record, and removes the mark.

Notation: `‖` is concatenation; `LE16`…`LE128` are little-endian unsigned
integers; every name without a width is a raw 32-byte digest, identifier, nonce or
σ-field value, and all-zero means "none"; `key` is a 65-byte uncompressed SEC1 P-256
key (`0x04 ‖ X ‖ Y`, on the curve); `sig` is a 64-byte big-endian `r ‖ s`; `tag` is
one byte. Numbers in parentheses are exact transcript lengths, pinned by tests.
"Design only" marks a rule these objects do not enforce or test.

## 1. Digests, transcripts and signatures

```text
H(role, body) = SHA-256("iroha:kagemusha:wallet:v1:" ‖ role ‖ 0x00 ‖ LE64(len(body)) ‖ body)
```

The 26-byte prefix is `69726f68613a6b6167656d757368613a77616c6c65743a76313a` and
`role` is an ASCII label below. Check value: `H("scheme", empty)` =
`90882608a8e8892521a2661be1e81c3fbfca0f8773e621000daed362a37485a5`.

**Poseidon** (proposal §3). `P(domain, items)` is the σ-field hash of §3.2. **Decided
(TODO(G1)):** `P_bytes(domain, b) = P(domain, [len(b)] ‖ c_0 ‖ … ‖ c_(m−1))`, where
`len(b)` is the byte length as one element, `m = ⌈len(b) / 31⌉` and `c_i` is bytes
`31i … 31i + 30` of `b` read as a little-endian integer, the last chunk zero-filled.
`P` covers `credit_id` (§3.4) and the blacklist and quota-window trees (§3.3);
`P_bytes` covers `proof_digest` in both domains (§3.2) and the Payment digest (§3.4).
Their SHA roles `credit`, `proof`, `step-proof`, `payment`, `blacklist-leaf`,
`blacklist-node`, `quota-window` and `quota-node` are deleted (62 → 54 roles). The Rust
step pins the packing and the new domains with vectors. The receipt body, the statement
and every other role below stay `H`.

- **Transcripts.** A body is a fixed-layout transcript of the object's fields in
  declaration order: fixed-width integers, raw digests, keys and signatures, and
  enums as a one-byte tag equal to the Norito tag. Where variants differ, the tag
  is followed by the variant's fields and zero fill to a pinned union width. Nested
  fixed records are inlined. A reference to a signer certificate is its 32-byte
  certificate digest. Only `account`, `certificate-set`, `evidence`, `proof`,
  `step-proof`, `lineage`, `credit-opening`, `marker`, `capsule`, `completion` and
  `fold` have variable-length bodies.
- **Signing.** The signer signs the exact preimage of `H(<x>-body, transcript)`
  with ECDSA-P256-SHA256, so the ECDSA message hash is the body digest `e`
  (Android Keystore `SHA256withECDSA` and Secure Enclave
  `ecdsaSignatureMessageX962SHA256` over that preimage: design only, no platform
  signer is wired yet). A fresh output, strict DER or raw `r ‖ s`, is normalized
  to low S and verified under the expected key before it is frozen into an
  object; an output that does not verify is rejected.
- **Verifying.** Only the fixed 64-byte `r ‖ s` with `1 ≤ r < n` and
  `1 ≤ s ≤ floor(n/2)` is accepted. High S, out-of-range scalars and other
  encodings are rejected; received bytes are never rewritten. JCA and CryptoKit
  accept high S, so SDKs apply the raw low-S check first.
- **Object digest.** `H(<x>, e ‖ sig)` (96-byte body). A signature confers only its
  signer role's authority; decoding or validating grants no monetary authority.

Role table (all 62 labels of `KagemushaWalletDigestRoleV1`; `A → B` names a signed
body role and its object-digest role):

| Role | Body | Bytes |
|---|---|---:|
| `scheme` | scheme transcript (§3.1) | 163 |
| `relation` | relation transcript (§3.1) | 162 |
| `provider-contract` | `LE16 1 ‖ "kagemusha-advance-journal-marker-v1"` zero-padded to 64 | 66 |
| `asset-scope` | asset scope transcript (§3.1) | 54 |
| `account` | complete canonical Norito frame of the domainless `AccountId` | var |
| `enrollment-challenge` | §3.1 | 194 |
| `enrollment-id`, `enrollment-key-binding` | `challenge_digest ‖ payment_key` | 97 |
| `wallet-id` | `scheme_id ‖ asset_digest ‖ payment_key ‖ enrollment_id` | 161 |
| `certificate-body` → `certificate` | §3.1 | 108 → 96 |
| `certificate-set` | `LE32 count ‖ certificate digests in set order` | 4+32n |
| `credential-body` → `credential` | §3.1 | 476 → 96 |
| `evidence` | `tag kind ‖ LE32 count ‖ (LE32 len ‖ original bytes)…` | var |
| `renewal-challenge`, `renewal-assertion` | §3.1 | 130 |
| `renewal-key-binding` | §3.1 | 163 |
| `artifact-manifest-body` → `artifact-manifest` | §3.1 | 290 → 96 |
| `statement` | §3.2 | 440 |
| `proof` | Send, Unload, Retiring: `LE32 len(Ω) ‖ Ω ‖ LE32 len(σ) ‖ σ` (§3.2); Decided (TODO(G1)): `P_bytes` | var |
| `step-proof` | every other operation: `LE32 len(σ) ‖ σ` (§3.2); Decided (TODO(G1)): `P_bytes` | var |
| `lineage` | Ω bytes: lineage public transcript (320) `‖` transport proof (§3.2) | var |
| `receipt-body` → `receipt` | §3.2 (derived, never transmitted) | 338 → 96 |
| `package` | `statement_digest ‖ proof_digest ‖ receipt_digest` | 96 |
| `operation-id` | `wallet_id ‖ tag operation_kind ‖ input` | 65 |
| `unload-nullifier` | `scheme_id ‖ wallet_id ‖ LE128 redeem_ordinal` | 80 |
| `scheme-policy-body` → `scheme-policy` | §3.3 | 142 → 96 |
| `fee-schedule-body` → `fee-schedule` | §3.3 | 191 → 96 |
| `blacklist-body` → `blacklist` | §3.3 | 118 → 96 |
| `blacklist-leaf`, `blacklist-node` | `lower ‖ upper`; `left ‖ right`; Decided (TODO(G1)): `P` (§3.3) | 64 |
| `quota-share-body` → `quota-share` | §3.3 | 190 → 96 |
| `quota-window`, `quota-node` | window leaf (§3.3); `left ‖ right`; Decided (TODO(G1)): `P` (§3.3) | 33; 64 |
| `time-anchor-body` → `time-anchor` | §3.3 | 138 → 96 |
| `charge-quote-body` → `charge-quote` | §3.3 | 219 → 96 |
| `offer-body` | §3.4 (signed; no object-digest role) | 194 |
| `session-control-body` | §3.4 (signed or unsigned; no object-digest role) | 197 |
| `request-body` → `request` | §3.4 | 354 → 96 |
| `credit` | the `request-body` transcript: `credit_id = H("credit", ·)`; Decided (TODO(G1)): `P` (§3.4) | 354 |
| `payment` | §3.4; Decided (TODO(G1)): `P_bytes` | 163 |
| `credit-opening` | §3.4 | 101+32n |
| `credit-status` | §3.4 | 162 |
| `credited` | §3.4 | 99 |
| `output` | `tag operation_kind ‖ statement_digest ‖ proof_digest ‖ payment_digest or zero` | 97 |
| `marker`, `capsule`, `completion`, `fold` | complete canonical Norito frame of the local object (§3.5) | var |
| `voucher-body` → `voucher` | §3.6 | 250 → 96 |
| `ledger-control-body` | §3.6 (signed; no object-digest role) | 211 |

Map leaves, chains, the state commitment and the σ statement are not SHA roles: they
are σ-field element lists hashed with Poseidon under the domains of §3.2.

## 2. Canonical frames and bounds

Canonical bytes are one complete `norito::encode_canonical` frame ([Norito](../norito.md)):

| Offset | Bytes | Content |
|---:|---:|---|
| 0 | 4 | magic `NRT0` (`4e525430`) |
| 4 | 2 | major `0`, minor `0` |
| 6 | 16 | schema hash: first 16 bytes of `SHA-256("norito:v1:type-name" ‖ 0x00 ‖ frame name)` |
| 22 | 1 | compression `0` |
| 23 | 8 | payload length, `LE64` |
| 31 | 8 | CRC-64/XZ of the payload, little-endian |
| 39 | 1 | flags `0x02` (`COMPACT_LEN`) |
| 40 | p | zero alignment padding |
| 40+p | len | payload |

- The frame name is `iroha_data_model::kagemusha::kagemusha_wallet_v1::<Type>`; the
  vectors pin the name and schema hash of all 25 framed types (envelope:
  `f03a9dc47142299cad9ffe0b2115fd42`).
- `p` is a property of the type, never inferred from the bytes. It is 8 when a
  `u128` is reachable without passing through a sequence (archived alignment 16;
  the envelope payload starts at byte 48), and 0 otherwise. The table below lists
  it per type. `armv7` is not an admitted native target: its `u128` alignment would
  change the padding.
- Payload (Norito derived layout under `COMPACT_LEN`): a record is its fields in
  declaration order, each `varint len ‖ payload`; integers are little-endian fixed
  width; a `bool` is one byte; a digest field is `0x20 ‖ 32 bytes`, a key
  `0x41 ‖ 65 bytes`, a signature `0x40 ‖ 64 bytes`; an enum is an `LE32` tag (equal
  to the transcript tag) followed by its variant's fields, each length-prefixed;
  `Vec<u8>` is `LE64 count ‖ bytes`; any other sequence is
  `LE64 count ‖ (varint len ‖ element)…`. `AccountId` and `AssetDefinitionId` use
  their own canonical encodings.
- **Decode order** for every framed type: (1) the complete-frame byte cap, before
  any parsing (10,000 for any envelope); (2) canonical decode under
  payload-derived limits (header, schema, flags, padding, CRC, exact length and
  canonical re-encoding); (3) every version
  field equals 1, outer value first, so a nested foreign body reports a version
  error, not a scheme mismatch; (4) envelopes only: the per-kind bound; (5) the
  expected scheme (expected wallet for a completion record); (6) structural
  validation. Decoding verifies the signatures whose keys the frame carries;
  certificate root, issuer and policy-signer signatures are checked by
  `verify(scheme, …)`, except that a certificate frame decodes against its scheme
  root. Payment, Credited and Lineage are validated structurally at decode; their
  full verification takes the session's inputs (§3.4).
- Flipping any byte of a vector Credential, Certificate, Offer, Request, Payment,
  Credited (both forms), Lineage, FoldRecord, Package, LoadVoucher, SchemePolicy,
  FeeSchedule, Blacklist, QuotaShare, TimeAnchor, ChargeQuote, LedgerControl or
  ArtifactManifest frame fails decoding or validation, or changes the object digest
  (tested). The quota share and blacklist digests cover only body and signature;
  the body's root and count bind the carried windows and entries. State,
  commitments, anchored time, unsigned session controls and renewal evidence bytes
  have no such binding.

| Bound | Value |
|---|---:|
| Envelope, Offer and SessionControl (complete frame) | 2,048 |
| Envelope, Request, Payment, Credited, PolicyData and Lineage | 10,000 |
| `kgm1:` text, session / message | 2,736 / 13,339 |
| Nested payer credential of an Offer (complete frame) | ≤ 1,024 |
| σ bytes and Ω transport-proof bytes | ≥ 1; the exact lengths of the frozen artifact allowlist, and until it freezes only the carrying frame (§7) |
| CreditStatus opening siblings | ≤ 256 (the bitmap population count) |
| Certificates per set | ≤ 3 |
| Blacklist entries | ≤ 65,535 |
| Quota windows per share | 1..=64 |
| Android renewal chain: certificates; each DER; total DER | 2..=8; 1..=16,384; ≤ 65,536 |
| App Attest renewal assertion | 1..=4,096 |
| Completion record output bytes | 1..=10,000 |
| Asset scale; fee basis points | ≤ 28; ≤ 10,000 |

Standalone frame caps (complete frame, checked before decoding) and padding:

| Type | Cap | p | Type | Cap | p |
|---|---:|---:|---|---:|---:|
| `SchemeV1` | 512 | 0 | `PaymentV1` | 10,000 | 8 |
| `SignerCertificateV1` | 512 | 0 | `PackageV1` | 10,000 | 8 |
| `CredentialV1` | 1,024 | 0 | `MarkerV1` | 1,024 | 8 |
| `RenewalRequestV1` | 73,728 | 0 | `RecoveryCapsuleV1` | 262,144 | 8 |
| `ArtifactManifestV1` | 1,024 | 0 | `CompletionRecordV1` | 65,536 | 0 |
| `SchemePolicyV1` | 1,024 | 0 | `FoldRecordV1` | 10,000 | 8 |
| `FeeScheduleV1` | 1,024 | 8 | `LoadVoucherV1` | 1,024 | 8 |
| `BlacklistV1` | 2,228,736 | 0 | `UnloadClaimV1` | 16,384 | 8 |
| `QuotaShareV1` | 8,192 | 0 | `FeeClaimV1` | 16,384 | 8 |
| `TimeAnchorV1` | 512 | 0 | `LedgerControlV1` | 1,024 | 8 |
| `ChargeQuoteV1` | 1,024 | 8 | `ActivationV1` | 16,384 | 8 |
| `EnvelopeV1` | per kind | 8 | `CloseLoadsV1` | 16,384 | 8 |
| | | | `AbandonmentV1` | 1,024 | 8 |

Type names omit the `KagemushaWallet` prefix. The blacklist cap is
`65,536 × 34 + 512`: each canonical entry takes 34 bytes. The blacklist is not a
peer message: a wallet downloads it only while online, from the issuer or ledger,
as this standalone frame, and peers never relay it, so its size does not bear on
the envelope bounds. The package cap applies where a completion record's output is
decoded as a package. The fold record carries one Ω, so it takes the Lineage
message bound.

**Text.** `kgm1:` followed by unpadded base64url of one canonical envelope frame.
Decoding rejects text above 13,339 bytes, a missing prefix, an empty body,
characters outside `A–Z a–z 0–9 - _` (so padding and whitespace), a body length of
`1 mod 4`, and text that does not re-encode to itself; the frame then follows the
decode order above.

## 3. Objects

Unless stated, a signed object's frame is `{body, signature}`, and its transcript
is the body's fields in the order shown.

### 3.1 Identity, certificates, credential, enrollment, renewal, artifact manifest

| Object | Fields in transcript order | Interoperability rules |
|---|---|---|
| Scheme (`scheme`) | `LE16 version ‖ network_id ‖ scheme_root_key(key) ‖ relation_id ‖ provider_contract` | `scheme_id = H("scheme", ·)`. `network_id` is the raw genesis `NetworkId`, carries the Iroha hash marker (last byte odd) and is not the marked zero `00…01`; `relation_id` nonzero; `provider_contract` is the V1 constant. Decoding requires the recomputed `scheme_id`. The root key signs certificates only. |
| Relation (`relation`) | `LE16 1 ‖ eq_protocol_digest ‖ ep_protocol_digest ‖ native_profile_digest ‖ verifying_key_set_digest ‖ artifact_inventory_digest` | Gives `relation_id`, fixed for the scheme's lifetime: one scheme-level identity that every statement and Ω carries. Decided (TODO(G1)): `verifying_key_set_digest` is the digest of the σ verifying-key allowlist that G1 defines, one entry per selector (operation tag, and for Send the enabled-controls mask) with its exact σ length, plus the exact Ω transport-proof length; the Rust step pins its layout. |
| Provider contract | see §1 | Constant `52b501e3344547c36579684aafb2b15eb0caf3393e77aebdfbaa14ac57d0cc8d`. |
| Asset scope (`asset-scope`) | `LE16 version ‖ asset UUID (16) ‖ asset_incarnation ‖ LE32 scale` | Frame `{version, asset: AssetDefinitionId, asset_incarnation, scale}`. UUIDv4 asset, valid `AxtAssetIncarnationV1`, `scale ≤ 28`. Gives `asset_digest`. |
| Signer certificate (`certificate-body`) | `LE16 version ‖ scheme_id ‖ tag role ‖ key ‖ LE64 serial` | Signed by the scheme root. Roles: Enrollment 1, LoadAuthorization 2, RegulatoryPolicy 3, TimeAnchor 4, Artifact 5. Fixed depth one; no validity period or revocation is evaluated offline; the consumer requires the role it needs. |
| Certificate set (`certificate-set`) | `LE32 count ‖ certificate digests` | Frame `{certificates}`: at most 3, unique, strictly ascending by certificate digest (unsigned byte order). Each carrier holds exactly the certificates it needs, with the required roles and scheme. |
| Enrollment challenge (`enrollment-challenge`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ account_digest ‖ app_policy ‖ enrollment_policy ‖ issuer_nonce` | All nonzero. `challenge_digest` is the KeyMint attestation challenge and the App Attest attestation `clientDataHash`. The App Attest enrollment assertion `clientDataHash` is `H("enrollment-key-binding", challenge_digest ‖ payment_key)`. H values go to App Attest unchanged. |
| Evidence digest (`evidence`) | `tag kind ‖ LE32 count ‖ (LE32 len ‖ bytes)…` | Kinds: AndroidKeyMintTee 1, AndroidKeyMintStrongBox 2, AppleAppAttest 3. Non-empty original items, never rewritten: Android attestation DER chain leaf first; Apple attestation object, then assertion. |
| Evidence record (inline, 56) | `digest ‖ LE64 time_ms ‖ LE32 facts ‖ LE32 os_patch_level ‖ LE32 vendor_patch_level ‖ LE32 boot_patch_level` | Fact bits below; `digest` nonzero. |
| Regulatory policy (inline, 20) | `LE32 permitted_controls ‖ LE64 blacklist_max_age_ms ‖ LE64 time_anchor_max_response_ms` | Controls: bit 0 BLACKLIST, 1 QUOTAS, 2 ATTESTATION_LEASE; others zero. `blacklist_max_age_ms > 0` requires bit 0. `time_anchor_max_response_ms > 0` iff bit 1, bit 2 or `blacklist_max_age_ms > 0`. |
| Credential (`credential-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ wallet_id ‖ account_digest ‖ payment_key(key) ‖ provider_contract ‖ tag evidence_kind ‖ enrollment_evidence(56) ‖ fresh_evidence(56) ‖ app_policy ‖ regulatory_policy(20) ‖ enrollment_id ‖ LE64 issued_at_ms ‖ LE32 renewal_sequence ‖ LE64 lease_expires_at_ms ‖ issuer_certificate` | Enrollment-role signer. Nonzero bindings; V1 provider contract even without a scheme; `wallet_id` recomputes. Renewal 0 requires `fresh_evidence = enrollment_evidence`; later renewals require `fresh.time_ms ≥ enrollment.time_ms`. `lease_expires_at_ms ≠ 0` iff the lease is permitted. A replacement changes only `fresh_evidence`, `issued_at_ms`, `lease_expires_at_ms` and `issuer_certificate`, and `renewal_sequence` is the predecessor's plus one. |
| Renewal transcripts | `renewal-challenge`, `renewal-assertion`: `LE16 1 ‖ scheme_id ‖ wallet_id ‖ credential_digest ‖ challenge`; `renewal-key-binding`: `LE16 1 ‖ scheme_id ‖ wallet_id ‖ challenge ‖ new_attested_key` | The payment key signs `renewal-challenge` (possession) and, on Android, `renewal-key-binding`. An App Attest renewal assertion covers `H("renewal-assertion", ·)`. |
| Renewal request (frame only) | `{version, scheme_id, wallet_id, credential_digest, challenge, possession_signature, evidence}` | Evidence: Android 1 `{new_attested_key, key_binding_signature, chain: [{der}]}` or Apple 2 `{assertion}` within the §2 bounds, on the credential's platform. Issuer verification of the chain or assertion is TODO(G5). |
| Artifact manifest (`artifact-manifest-body`) | `LE16 version ‖ network_id ‖ relation_id ‖ eq_protocol_digest ‖ ep_protocol_digest ‖ native_profile_digest ‖ verifying_key_set_digest ‖ artifact_inventory_digest ‖ provider_contract ‖ signer_certificate` | Artifact-role signer. Bindings nonzero; `relation_id` recomputes from the five bindings; network, relation and provider contract equal the scheme's. |

Fact bits: 0 HARDWARE_BACKED_KEY, 1 STRONGBOX, 2 BOOTLOADER_LOCKED, 3 VERIFIED_BOOT,
4 PATCH_POLICY_MET, 5 APP_SIGNING_IDENTITY, 6 APP_ATTEST_GENUINE_DEVICE,
7 APP_ATTEST_KEY_BINDING, 8 APP_ATTEST_PRODUCTION, 9 PLAY_INTEGRITY_SIGNAL,
10 REVOCATION_LIST_CLEAR, 11 LOCAL_COMPROMISE_CHECKS_CLEAR; 12–31 are zero.
Android records never carry 6–8; Apple records never carry 0–5 or 9 and have zero
patch levels. Enrollment evidence requires bits 0, 2, 3 and 5 on Android (plus 1
for StrongBox; TEE forbids 1) and bits 6 and 7 on Apple.

### 3.2 State, field encoding, statement, proofs, receipt, package

**σ field and element rule.** The step proofs σ are single-parity proofs over the
Pasta `Fp` (Vesta scalar field),
`p = 0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001`. A σ-field
value is its canonical 32-byte little-endian encoding (`< p`); decoding rejects a
noncanonical value wherever a field value is required. Element lists use one rule:
an integer, tag or mask is one element; a 32-byte SHA-256 digest or identifier is
two `u128` limbs, low 16 bytes first (each little-endian); a commitment, chain,
Poseidon map root or nonce is one element. `Poseidon(domain, items)`, written `P` in
§1, is the RP57 `iroha_pasta::poseidon::hash_with_domain`; the domain is the `u64` of 8
little-endian ASCII bytes:

| Domain | Use | Domain | Use |
|---|---|---|---|
| `kgwcore1` | state commitment | `kgwccrd1` | consumed-credit leaf |
| `kgwrest1` | rest digest | `kgwpout1` | pending-outgoing leaf |
| `kgwstmt1` | σ statement digest | `kgwload1` | load-recovery leaf |
| `kgwschn1` | `send_chain` append | `kgwrdm_1` | redeem-recovery leaf |
| `kgwrchn1` | `recv_chain` append | `kgwfee_1` | fee-claim leaf |
| `kgwcdig1` | credit-digest leaf | `kgwquse1` | quota-usage leaf |

The data model fixes every element list and validates canonical encodings.
**Decided (TODO(G1)):** `credit_id`, the Payment digest and the blacklist and
quota-window roots are σ-field values and enter element lists as one element each.
The data model depends on `iroha_pasta` and computes every `P` and `P_bytes` value
natively: `credit_id`, the chains, the rest digest and commitment, the map,
blacklist, quota-window and credit-digest roots and openings, `proof_digest` and the
Payment digest. `credit_id` uses `kgwcrdt1`; the Rust step adds and pins with vectors
the domains of `proof_digest` (both), the Payment digest, the blacklist and
quota-window leaves and nodes, and the internal nodes and default subtrees of every
tree. The shapes and empty roots of the state-map trees are open (§7).

**State.** The private state is not transmitted; it travels only inside a recovery
capsule. Its frame is `{version, core, rest}`.

- The *core* holds every field a step proof reads, changes or carries. Frame and
  element order (29 elements): `lifecycle` (Active 1, Retiring 2), `wallet_id` (2),
  `credential_digest` (2), `balance`, `burned_total`, `sequence`, `next_send`,
  `next_load`, `next_redeem` (`u128` each), `send_chain`, `recv_chain`,
  `consumed_credit_root`, `pending_outgoing_root`, `load_recovery_root`,
  `redeem_recovery_root`, `fee_claim_root`, `quota_usage_root`, `LE32
  enabled_controls`, `quota_windows_root` (2), `LE64 blacklist_version`,
  `blacklist_root` (2), `LE64 lease_expires_at_ms`, `LE64 policy_epoch`,
  `LE64 accepted_time_floor_ms`, `state_nonce`. The held blacklist and quota-window
  roots are SHA-256 tree roots (§3.3) and enter as two limbs.
- The *rest* is opened only by the lineage relation. Frame and element order (19
  elements): `scheme_id` (2), `asset_digest` (2), `regulatory_policy` (3: permitted
  controls, `blacklist_max_age_ms`, `time_anchor_max_response_ms`), `scheme_policy`
  (2), `fee_schedule` (2), `blacklist` (2), `LE64 blacklist_issued_at_ms`,
  `quota_share` (2), `LE64 quota_share_id`, `time_anchor` (2).
- **Decided (TODO(G1)):** `scheme_id`, `asset_digest`, `blacklist_issued_at_ms` and
  the regulatory policy's `blacklist_max_age_ms` move from the rest to the core; one
  `load_redeem_recovery_root`, for one map keyed by `(kind, ordinal)`, replaces
  `load_recovery_root` and `redeem_recovery_root`, leaving five map roots; the
  blacklist and quota-window roots become `P` roots of one element each. The Rust
  step pins the core and rest element order and counts.
- The commitment is one σ-field value:
  `Poseidon(kgwcore1, core elements ‖ Poseidon(kgwrest1, rest elements))`. Its
  transcript is the 32-byte encoding; all-zero is only the Bootstrap predecessor.
- Rules: nonzero identities; the six map roots and `state_nonce` are nonzero
  canonical values; chains are canonical (zero is the empty chain);
  `enabled_controls` ⊆ the regulatory policy's permitted controls. A scheme policy,
  blacklist or quota share is held exactly when its digest, its epoch, version or
  share id, and its root (if any) are all nonzero; without a scheme policy,
  `enabled_controls` and `fee_schedule` are zero; an unheld blacklist has a zero
  issue time; `lease_expires_at_ms ≠ 0` iff the lease is permitted. Bootstrap state
  has zero balance, `burned_total`, ordinals, sequence and chains, the map owner's
  empty roots, and zero policy fields except the credential's regulatory policy and
  lease.
- The spendable value is `balance − burned_total` with the lineage-adjusted
  `burned_total` of the Ω recorded for the head (§§3.2, 6.1 of the proposal); Send
  and Unload pre-checks use it.

**Leaves and chains** (element lists after the domain; no state commitment or chain
contains a Payment digest):

| Object | Domain | Elements |
|---|---|---|
| consumed credit (permanent map) | `kgwccrd1` | `credit_id` (2), `amount`, `receive_sequence` (4) |
| pending outgoing | `kgwpout1` | `credit_id` (2), `receiver_wallet_id` (2), `send_ordinal`, `amount`, `fee`, `request_digest` (2) (9) |
| load recovery | `kgwload1` | `ordinal`, `voucher_digest` (2), `amount` (4) |
| redeem recovery | `kgwrdm_1` | `ordinal`, `nullifier` (2), `amount`, `online_charge` (5) |
| fee claim | `kgwfee_1` | `credit_id` (2), `fee`, `fee_schedule_digest` (2) (5) |
| quota usage | `kgwquse1` | `window_kind` tag, `window_start_ms`, `window_end_ms`, `used` (4) |
| `send_chain` append | `kgwschn1` | `[send_chain]` ‖ the pending-outgoing elements (10) |
| `recv_chain` append | `kgwrchn1` | `[recv_chain]` ‖ `credit_id` (2), `payer_wallet_id` (2), `amount` (6) |
| credit-digest leaf (lineage level, not in the state) | `kgwcdig1` | `credit_id` (2), `payment_digest` (2), `burned` 0 or 1 (5) |

**Decided (TODO(G1)):** `credit_id` and `payment_digest` are one element each in the
lists above and in the statement effects below; the load and redeem recovery leaves
belong to one map keyed by `(kind, ordinal)`. The Rust step pins the resulting lists,
counts and leaf domains.

**Operation kinds and effects** (effect transcript (161) = `tag ‖ fields ‖ zero fill
to 160`; the effect tag equals the operation kind; Ω(pred) marks the kinds that
consume the predecessor's lineage proof):

| Tag | Kind | Effect fields (width) | Elements | `operation-id` input | Ω(pred) |
|---:|---|---|---:|---|---|
| 1 | Bootstrap | `enrollment_id ‖ enrollment_marker` (64) | 4 | `enrollment_id` | no |
| 2 | Load | `voucher ‖ LE128 load_ordinal ‖ LE128 amount ‖ LE128 online_charge` (80) | 5 | `voucher` | no |
| 3 | Send | `credit_id ‖ receiver_wallet_id ‖ LE128 send_ordinal ‖ LE128 amount ‖ LE128 fee ‖ request ‖ LE64 accepted_lower_ms ‖ LE64 accepted_upper_ms` (160) | 11 | `credit_id` | yes |
| 4 | Receive | `credit_id ‖ payer_wallet_id ‖ LE128 amount` (80) | 5 | `credit_id` | no |
| 5 | ArchiveSent | `credit_id ‖ credited` (64) | 4 | `credited` | no |
| 6 | Unload | `nullifier ‖ LE128 redeem_ordinal ‖ LE128 amount ‖ LE128 online_charge ‖ charge_quote` (112) | 7 | `nullifier` | yes |
| 7 | RefreshPolicy | `tag update_kind ‖ update ‖ LE64 accepted_time_floor_ms` (41) | 4 | `update` | no |
| 8 | Retiring | none (0) | 0 | 32 zero bytes | yes |

Update kinds: Credential 1, SchemePolicy 2, Blacklist 3, QuotaShare 4, TimeAnchor 5;
`update` is the applied object's digest. Effect rules: nonzero digests; Send,
Receive and Unload amounts positive; Send `amount + fee` fits `u128` and
`accepted_lower_ms ≤ accepted_upper_ms`; Unload `online_charge ≤ amount` and
`charge_quote` nonzero iff `online_charge > 0`. The Send `request` is the digest of
the signed Request, which binds the receiver credential, fee schedule and
certificates by digest; the Receive effect carries no Payment digest. `ArchiveSent`
uses the Credited digest as its operation input, so archiving again with new
evidence is a new operation.

- **Statement** (`statement`, 440): `LE16 version ‖ scheme_id ‖ relation_id ‖
  credential_digest ‖ asset_digest ‖ tag lifecycle ‖ LE128 sequence ‖ LE128
  next_load ‖ LE32 enabled_controls ‖ LE128 lineage_burned_total ‖
  lineage_pending_outgoing_root ‖ predecessor ‖ successor ‖ effect(161)`. Lifecycle,
  sequence and `next_load` are the successor's; `enabled_controls` is the
  predecessor's mask that σ enforced. For Send, Unload and Retiring the lineage
  fields are Ω(pred)'s `burned_total` and pending-outgoing root (the root nonzero);
  every other kind has zero lineage fields. Bootstrap alone has sequence 0, a zero
  predecessor, Active, `next_load` 0 and an empty mask; every other statement has a
  nonzero predecessor. Commitments are canonical and the successor is nonzero.
  Retiring yields lifecycle Retiring; Load yields `next_load = load_ordinal + 1`; a
  Credential refresh has `update = credential_digest`. A successor chains on the
  predecessor's successor commitment with sequence plus one; lifecycle changes only
  by Retiring from Active; `next_load` changes only by Load at the predecessor's
  `next_load`; `credential_digest` changes only by a Credential refresh. Against its
  credential: Bootstrap names the credential's `enrollment_id`, Send and Receive
  never name the wallet itself, and an Unload nullifier recomputes.
- **σ public input** (29 elements, `Poseidon(kgwstmt1, ·)`): `version`,
  `relation_id` (2), `scheme_id` (2), `asset_digest` (2), `credential_digest` (2),
  lifecycle, sequence, `next_load`, `enabled_controls`, `lineage_burned_total`,
  `lineage_pending_outgoing_root`, `predecessor`, `successor`, effect tag, then the
  effect's elements in field order zero-filled to 11. The prototype
  `iroha_kagemusha_proof` relations expose this encoding through
  `iroha_plonk_gadgets::statement::StatementV1`, whose native encoding reproduces the
  Send and Receive statement vectors of §5 (`statement_digest` test).
- **Consumer checks** (proposal §3.2) of a statement carrying Ω(pred), before
  mutation: scheme and relation equal Ω's; `predecessor = Ω.head`;
  `credential_digest = Ω.credential_digest`; the lineage fields equal Ω's
  `burned_total` and pending-outgoing root; `enabled_controls = Ω.enabled_controls`
  (which selects σ_send's verifying key); the predecessor lifecycle (Active for
  Retiring, else the statement's) equals Ω's; and the wallet-bound effect rules for
  `Ω.wallet_id`.
- **Operation and nullifier.** `operation_id = H("operation-id", wallet_id ‖ tag kind ‖ input)`;
  `nullifier = H("unload-nullifier", scheme_id ‖ wallet_id ‖ LE128 redeem_ordinal)`.
- **Step proof σ.** Frame `{bytes}`, non-empty. The PIPA-v1 layout comes from the
  artifact set (TODO(G3)) and the exact lengths from the verifying-key allowlist
  (§3.1 Relation).
- **Lineage proof Ω.** Frame `{public, proof}`; `proof` is the non-empty single-parity
  transport proof. The public transcript (320) is `LE16 version ‖ scheme_id ‖
  relation_id ‖ head ‖ wallet_id ‖ credential_digest ‖ payment_key(key) ‖ tag
  lifecycle ‖ LE64 policy_epoch ‖ LE32 enabled_controls ‖ LE128 burned_total ‖
  pending_outgoing_root ‖ credit_digest_root`; identities nonzero, the head and both
  roots nonzero canonical values, defined control bits only. The Ω bytes are that
  transcript followed by the proof, so `proof_digest` and the Payment digest bind
  every exposed value; `lineage_digest = H("lineage", Ω bytes)` identifies
  byte-identical reuse. A head has at most one recorded Ω, carried unchanged by
  every Lineage message, Payment and ledger package from it.
- **`proof_digest`.** Send, Unload and Retiring:
  `H("proof", LE32 len(Ω) ‖ Ω ‖ LE32 len(σ) ‖ σ)`; every other operation:
  `H("step-proof", LE32 len(σ) ‖ σ)`. It is recomputed on every use; the Durable
  State Provider binds it in Advance. **Decided (TODO(G1)):** `P_bytes` (§1) over the
  same byte strings under two distinct domains, giving one canonical σ-field value.
- **Receipt τ.** Frame `{version, operation_id, capsule_digest, payment_digest,
  signature}`. Its signed body is derived by every verifier and never transmitted:
  `receipt-body` (338) = `LE16 version ‖ scheme_id ‖ wallet_id ‖ provider_contract`
  (from the signer) `‖ LE128 sequence ‖ operation_id ‖ predecessor ‖ successor` (from
  the statement) `‖ statement_digest ‖ proof_digest ‖ capsule_digest ‖
  payment_digest`. `payment_digest` is the full canonical Payment digest for Receive
  and zero otherwise. The signer is the wallet's credential, or, for a consumer of a
  package carrying Ω(pred), Ω's scheme, wallet and payment key under the V1 provider
  contract. The carried `operation_id` must equal the recomputed one.
- **Package.** Frame `{version, statement, lineage, step_proof, receipt}`; `lineage`
  is None 0 or Present 1 `{lineage: Ω(pred)}`, present exactly for Send, Unload and
  Retiring, and a present Ω passes the consumer checks.
  `package_digest = H("package", statement_digest ‖ proof_digest ‖ receipt_digest)`,
  defined only after the receipt verifies (under the credential, which must also
  carry Ω's wallet and payment key, or under Ω for a consumer without the
  credential). σ's verifying key is selected by the operation tag and, for Send, the
  mask, from the verifying-key allowlist (§3.1 Relation).

### 3.3 Policy objects

All are signed by a RegulatoryPolicy-role key except the time anchor
(TimeAnchor role). Each body's `signer_certificate` names the signer.

| Object | Body fields in transcript order | Interoperability rules |
|---|---|---|
| Scheme policy (`scheme-policy-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ LE64 policy_epoch ‖ LE32 enabled_controls ‖ fee_schedule ‖ signer_certificate` | `policy_epoch ≥ 1` (0 is the implicit default); defined control bits only; zero `fee_schedule` means no fee. |
| Fee schedule (`fee-schedule-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ LE64 schedule_id ‖ beneficiary_account_digest ‖ LE32 basis_points ‖ LE128 fixed ‖ LE128 minimum ‖ LE128 maximum ‖ tag rounding ‖ signer_certificate` | Rounding Down 1, Up 2; `basis_points ≤ 10,000`; `maximum ≥ minimum`. For `a = 10,000·q + r`, `fee(a) = clamp(fixed + q·bp + ⌊r·bp / 10,000⌋ (+1 if Up and r·bp mod 10,000 ≠ 0), minimum, maximum)`; an unclamped `u128` overflow rejects. |
| Blacklist (`blacklist-body`) | `LE16 version ‖ scheme_id ‖ LE64 list_version ‖ LE64 issued_at_ms ‖ LE32 entry_count ‖ entries_root ‖ signer_certificate` | Frame `{body, signature, entries: [{account_digest}]}`. `list_version ≥ 1`; entries strictly ascending, never `00…00` or `FF…FF`; count and root recompute. Downloaded only online, from the issuer or ledger, as one standalone frame; never a peer message. |
| Quota share (`quota-share-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ wallet_id ‖ LE64 share_id ‖ LE64 issued_at_ms ‖ LE64 expires_at_ms ‖ windows_root ‖ LE32 window_count ‖ signer_certificate` | Frame `{body, windows, signature}` (windows before the signature). `share_id ≥ 1`; `issued < expires`; each window has `start < end` within `[issued, expires]`; windows strictly sorted by `(kind, start)`, never overlapping within a kind; count and root recompute. |
| Time anchor (`time-anchor-body`) | `LE16 version ‖ scheme_id ‖ wallet_id ‖ nonce ‖ LE64 issuer_time_ms ‖ signer_certificate` | Answers one wallet nonce. |
| Charge quote (`charge-quote-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ wallet_id ‖ tag kind ‖ LE128 ordinal ‖ LE128 net_amount ‖ LE128 online_charge ‖ beneficiary_account_digest ‖ LE64 issued_at_ms ‖ signer_certificate` | Kinds Load 1, Unload 2. `online_charge > 0`. Load: the ledger debit `net_amount + online_charge` fits `u128`. Unload: `net_amount > 0` and the payout `net_amount − online_charge` does not underflow. |

- **Blacklist tree.** Fixed depth 16 over 65,536 gap leaves. With sentinels
  `s_0 = 00…00`, entries `s_1…s_n` and `s_{n+1} = FF…FF`, leaf `i ≤ n` is
  `H("blacklist-leaf", s_i ‖ s_{i+1})`; unused leaves are
  `H("blacklist-leaf", FF…FF ‖ FF…FF)`; a node is `H("blacklist-node", left ‖ right)`.
  Non-membership of `x` is one gap leaf with `lower < x < upper` and its 16 siblings,
  a local witness that is never transmitted. **Decided (TODO(G1)):** leaves and nodes
  are `P` values under distinct domains, a leaf over the limbs of `s_i ‖ s_{i+1}`; the
  root is one σ-field value.
- **Quota tree.** A window leaf (33) is `tag kind ‖ LE64 start_ms ‖ LE64 end_ms ‖
  LE128 limit` (Daily 1, Monthly 2; half-open `[start, end)`); an empty slot is
  `H("quota-window", 33 zero bytes)`; the root is a depth-6 tree over 64 slots,
  windows first, with `H("quota-node", left ‖ right)`. **Decided (TODO(G1)):** leaves
  (the window's four fields as elements) and nodes are `P` values under distinct
  domains; the root is one σ-field value. The Rust step pins both trees' domains,
  empty leaves and vectors.
- **Time.** The local anchored time `{anchor, boot_id, LE64 request_monotonic_ms,
  LE64 receive_monotonic_ms}` gives, at monotonic reading `m` in the same boot,
  `[T + (m − m_rcv), T + (m − m_req)]`, valid when `m ≥ m_rcv` and
  `m_rcv − m_req ≤ time_anchor_max_response_ms`. A Send binds `L = max(floor,
  anchor lower, receiver_accepted_time_ms)` and `U = max(anchor upper, L)` (`U = L`
  without an anchor). An active time-dependent control without the committed
  same-boot anchor refuses Send. A deadline (lease expiry, quota share expiry) has
  passed iff `U ≥ deadline`. Blacklist age is `U − blacklist_issued_at_ms`; σ_send
  enforces the maximum-age rule from the core (Decided (TODO(G1)), §3.2). A quota
  window is touched iff `start ≤ U` and `L < end`; every touched window needs
  `used + amount + fee ≤ limit`, and each window kind the share defines must be
  touched.
- **RefreshPolicy.** One signed update per transition. Epochs, list versions and
  share ids strictly increase; a time anchor must differ from the held one; a
  quota share keeps the end of every existing usage key; `quota_usage_root` never
  changes. The new floor is `max(old floor, t)`, where `t` is the credential's
  `issued_at_ms`, the old floor for a scheme policy, the list's or share's
  `issued_at_ms`, or the anchor's `issuer_time_ms`.

### 3.4 Messages and envelope

The envelope frame is `{version, message}`; message tags are Offer 1, Request 2,
Payment 3, Credited 4, SessionControl 5, PolicyData 6 and Lineage 7. The scheme
checked at decode is the body scheme (Offer, Request), the carried Request's scheme
(Payment), Ω's scheme (Lineage) or the message's own field.

| Message | Frame and transcript | Interoperability rules |
|---|---|---|
| Offer | `{body, payer_credential, certificates, signature}`; `offer-body` = `LE16 version ‖ scheme_id ‖ asset_digest ‖ payer_wallet_id ‖ payer_credential_digest ‖ LE128 next_send ‖ LE128 amount ‖ session_nonce` | Payer payment key signs. Body matches the credential's scheme, asset, wallet and digest; the credential frame is at most 1,024 bytes; `amount > 0`; certificates exactly the payer issuer. No debit or credit authority; a delivery retry also opens with an Offer, so the receiver holds the payer credential. |
| Lineage | `{version, lineage: Ω}` | Unsigned and sent only after an authenticated Offer. The receiver rate-limits it and checks Ω's scheme, `wallet_id`, credential digest and `payment_key` against the Offer's credential; a Payment whose Ω(pred) has the same `lineage_digest` reuses that verification. |
| Request | `{body, receiver_credential, fee_schedule, certificates, signature}`; `request-body` = `LE16 version ‖ scheme_id ‖ asset_digest ‖ payer_wallet_id ‖ receiver_wallet_id ‖ LE128 send_ordinal ‖ receiver_credential_digest ‖ LE128 amount ‖ fee_schedule ‖ LE128 fee ‖ LE64 policy_epoch ‖ scheme_policy ‖ LE64 receiver_accepted_time_ms ‖ certificates ‖ nonce` | Receiver payment key signs. Slot None 0 or Present 1 `{schedule}`. `amount > 0`; payer ≠ receiver wallet; `amount + fee` fits; `policy_epoch = 0` iff `scheme_policy` zero. None requires zero `fee_schedule` and `fee`; Present requires the digest, the Request's scheme and asset, and `fee = fee(amount)`. Certificates exactly the receiver issuer (Enrollment) plus the fee signer (RegulatoryPolicy) when present; their set digest is `body.certificates`. `credit_id = H("credit", request-body)`; Decided (TODO(G1)): `credit_id = P(kgwcrdt1, ·)` over the 24 request-body elements in transcript order, a canonical σ-field value. `request_digest = H("request", e ‖ sig)`. |
| Payment | `{version, request: {body, signature}, payer_payment_key, payer_credential_digest, send: Package}`; `payment` (163) = `LE16 version ‖ request_digest ‖ payer_payment_key(key) ‖ payer_credential_digest ‖ package_digest`; Decided (TODO(G1)): the Payment digest is `P_bytes` of this transcript under its own domain | The compact layout: the receiver's credential, fee schedule and certificates are bound by digest in the Request body, and the payer's credential and certificates travel in the Offer. Structurally (no `payment_digest` without it): `send` is a Send package carrying Ω(pred) that passes the consumer checks with τ verified under `Ω.payment_key`; the Request payer is `Ω.wallet_id`; the carried key and credential digest are Ω's; the effect's credit, receiver, ordinal, amount, fee and `request` equal the Request's, `accepted_lower_ms ≥ receiver_accepted_time_ms`, the statement's scheme and asset are the Request's; `Ω.policy_epoch ≥` the Request's. At Receive: the carried Request is the receiver's held one, which verifies in full; the receiver is matched by the Request's receiver wallet and its receiver credential's key, never by credential digest, so a Request signed before a renewal stays receivable (Decided (TODO(G1))); the Offer's payer credential verifies under the scheme, is the Request's payer with a key other than the receiver's, and matches the carried digest and key; the statement's relation is the scheme's. σ_send and Ω(pred) with its decide are the proof owner's. |
| Credited | `{version, scheme_id, evidence}`; `credited` (99) = `LE16 version ‖ tag evidence ‖ credit_id ‖ payment_digest ‖ evidence digest` | Evidence Receive 1 `{package}` (status *credited, unfolded*; evidence digest = its package digest) or Status 2 `{status: CreditStatus}` (status *credited* or *burned*; evidence digest = its `credit-status` digest). `scheme_id` is the evidence statement's scheme. The payer verifies it against its scheme, held Request and retained Payment: in both forms the evidence statement names the payer's scheme and relation identity (rejected before the `ArchiveSent` mutation, as `Λ_archive` rejects it); Receive form, a Receive package (no Ω) whose effect credit, payer wallet and amount match, whose receipt binds the Payment digest, whose scheme and asset are the Request's, and whose τ verifies under the Request's receiver credential; Status form, Ω(h)'s wallet and credential digest equal the Request's receiver (Decided (TODO(G1)): Ω(h)'s `wallet_id` and `payment_key` equal the Request's receiver wallet and its receiver credential's key; credential digests are not compared) and the opening names this credit and Payment digest. `ArchiveSent` also requires the retained Payment's pending leaf. |
| CreditStatus (inside Credited) | `{version, statement, proof_digest, receipt, lineage: Ω(h), opening}`; `credit-status` (162) = `LE16 version ‖ statement_digest ‖ proof_digest ‖ receipt_digest ‖ lineage_digest ‖ opening_digest` | Read-only evidence from a folded receiver head `h`, carrying no σ and no Ω(pred): the statement's successor is `Ω(h).head`; scheme, relation, credential digest and lifecycle equal Ω's; τ(h) verifies under Ω(h)'s key over the carried statement and nonzero `proof_digest`, with its own capsule and Payment digests. The decide of Ω(h) is the proof owner's (TODO(G3)); the opening against `Ω(h).credit_digest_root` is recomputed by the data model (Decided (TODO(G1))). A proof of absence is not evidence. |
| Credit opening (inside CreditStatus) | `{credit_id, payment_digest, burned: bool, path_bitmap, siblings: bytes}`; `credit-opening` (101+32n) = `credit_id ‖ payment_digest ‖ u8 burned ‖ path_bitmap ‖ LE32 n ‖ siblings` | A compressed opening of the credit-digest leaf in the depth-256 sparse Merkle `P` tree keyed by the canonical `credit_id` bits (Decided (TODO(G1)): the Rust step pins the key-bit order, node domain and default subtrees): bit `i` of `path_bitmap` (byte `i/8`, least significant bit first) marks a non-default sibling; `siblings` holds exactly those, root-ward, as `n` concatenated canonical 32-byte values. Nonzero credit and Payment digests. |
| SessionControl | `{version, scheme_id, asset_digest, sender_wallet_id, peer_wallet_id, session_nonce, kind, LE16 reason, credit_id, auth}`; `session-control-body` = all fields except `auth`, kind as a tag | Kinds SetupDeclined 1, UnsupportedScheme 2, ReceiveDeferred 3, Close 4; auth Unsigned 0 or Signed 1 `{signature}`. Scheme, asset and nonce nonzero; sender nonzero except UnsupportedScheme; peer zero (unknown) or not the sender; opaque `reason` only for SetupDeclined and ReceiveDeferred; `credit_id` nonzero iff ReceiveDeferred. Unsigned only for UnsupportedScheme and for SetupDeclined before the sender's own Offer or Request; otherwise signed by the payment key of that session's Offer or Request credential. Invalid controls are dropped. |
| PolicyData | `{version, scheme_id, asset_digest, item}` | Items SchemePolicy 1, FeeSchedule 2, Certificates 3 (non-empty); tag 4 is unused, because the blacklist is never peer-carried (§3.3). Item scheme equals `scheme_id`; scheme policy and fee schedule asset equal `asset_digest`. Signers are selected by certificate digest. |

- **Unknown scheme.** A receiver may decode an Offer envelope through the per-kind
  bound without the scheme check, validate only its body, and reply with an
  unsigned UnsupportedScheme naming the Offer's scheme, asset and session nonce,
  zero sender and the payer as peer.
- **Send rule** (native pre-check; relation enforcement is design only, G3): the
  Request names the payer's wallet and `next_send`; the Ω is the payer's (wallet,
  credential and key); payer `policy_epoch ≥` the Request's, with an equal
  `scheme_policy` at equal epochs; `fee_schedule` equals the payer's held schedule;
  payer and receiver keys differ; `balance − Ω.burned_total ≥ amount + fee`.

### 3.5 Custody objects (local, never transmitted)

Their enums are Norito enums: frames carry 4-byte tags; the `output` transcript
carries one byte.

| Object | Frame | Interoperability rules |
|---|---|---|
| Marker (`marker` over the frame) | `{version, scheme_id, asset_digest, wallet_id, payment_key, LE128 generation, state}`; state Enrollment 1 `{challenge_digest, enrollment_id}`, Head 2 `{LE128 sequence, operation_id, head, capsule_digest, predecessor_capsule_digest}`, Terminal 3 `{reason, last_capsule_digest}` | Reasons Abandoned 1, CustodyDeleted 2. Generation 0 iff Enrollment, whose `enrollment_id` and `wallet_id` recompute. Head: complete `head`; zero predecessor capsule iff sequence 0. Terminal: zero last capsule iff Abandoned. Successors raise the generation by one: Enrollment → Head (sequence 0) or Terminal Abandoned; Head → Head (sequence + 1, linked capsule) or Terminal CustodyDeleted (last = capsule). The Bootstrap `enrollment_marker` is the generation-0 marker digest. |
| Output descriptor | `{kind, digest}` | `digest = H("output", ·)` over the statement digest, `proof_digest` and, for Receive, the Payment digest the receipt binds: receipt-free, so the capsule freezes before the receipt. Every other kind input is bound by the statement. |
| Recovery capsule (`capsule` over the frame) | `{version, scheme_id, wallet_id, operation_id, kind, predecessor_capsule_digest, successor_state, statement, predecessor_lineage, step_proof, payment_digest, map_openings: [bytes], retained_inputs: [{role, bytes}], output}` | Retained roles: Request 1, Payment 2, Credited 3, LoadVoucher 4, ChargeQuote 5, PolicyUpdate 6, CertificateSet 7, Credential 8. State and statement agree on scheme, wallet, credential, asset, lifecycle, sequence and `next_load`; kind, effect kind and output kind agree; `operation_id` recomputes; zero predecessor capsule iff sequence 0; `predecessor_lineage` (the Ω recorded at fold time) is present exactly for Send, Unload and Retiring, passes the consumer checks for this wallet, and the successor's `burned_total` is its `burned_total`; `payment_digest` is nonzero exactly for Receive; the output rebuilds for every kind. Openings and retained inputs are non-empty, and the fold witnesses (every consumed input `Λ` verifies for the step, proposal §4.1) are retained: Receive needs Request, Payment, CertificateSet and Credential; ArchiveSent Request, Payment and Credited; Send the Request (the Payment binds its fee schedule only by digest); Load LoadVoucher and CertificateSet (the voucher's LoadAuthorization certificate); RefreshPolicy PolicyUpdate and CertificateSet. The receipt signs `capsule_digest`. The opening layout is TODO(G3). |
| Completion record (`completion` over the frame) | `{version, wallet_id, operation_id, capsule_digest, receipt, output: bytes}` | Decoded against the expected wallet. `output` is the canonical compact Payment for Send, whose payer key and credential digest are the wallet's, and the canonical Package frame otherwise. Its receipt equals the record's; its statement, Ω(pred), σ and receipt Payment digest are the capsule's; its receipt-free parts rebuild the capsule's output descriptor; the receipt verifies over `capsule_digest`. |
| Fold record (`fold` over the frame) | `{version, scheme_id, wallet_id, LE128 first_sequence, LE128 sequence, head, capsule_digest, lineage: Ω}` | The self-verified Ω of one folded head covering the run `first_sequence..=sequence`. Nonzero identities and capsule digest; complete `head`; Ω valid with this scheme, wallet and head; `first_sequence ≤ sequence`. |

### 3.6 Ledger objects

| Object | Frame and transcript | Interoperability rules |
|---|---|---|
| Load voucher (`voucher-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ wallet_id ‖ LE128 ordinal ‖ LE128 amount ‖ LE128 online_charge ‖ charge_quote ‖ transaction_hash ‖ LE64 block_height ‖ authorizer_certificate` | LoadAuthorization-role signer. `block_height ≥ 1`; `amount > 0`; `charge_quote` nonzero iff `online_charge > 0`, and then a Load quote with the same scheme, asset, wallet, ordinal, net amount and charge; `amount + online_charge` fits. A state absorbs it only at its `next_load`. |
| Unload claim | `{version, credential, package, account: AccountId, charge, certificates}`; charge None 0 or Quoted 1 `{quote, beneficiary: AccountId}` | Certificates exactly the issuer plus the quote signer when quoted. `H("account", account)` equals the credential's account digest. The package is a verified Unload carrying Ω(pred), which names the credential's wallet and payment key and passes the consumer checks; `charge` is Quoted iff the effect names a quote, which must be an Unload quote for the effect's exact terms, with `H("account", beneficiary)` as its beneficiary. Payout `amount − online_charge`. |
| Fee claim | `{version, payment, beneficiary: AccountId}` | Structurally a valid Payment naming a fee schedule with `fee > 0`. The payout is checked against the historical schedule the Payment names (digest, scheme, asset, `fee = fee(amount)`), and `H("account", beneficiary)` equals its beneficiary. Full verification takes the schedule, the receiver's Request and the payer's credential and certificates from the ledger's records by the digests the Payment binds, and verifies the Payment as at Receive. |
| Ledger control (`ledger-control-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ wallet_id ‖ tag action ‖ fields zero-filled to 80 ‖ nonce` | Actions Activate 1 `{package_digest}`, CloseLoads 2 `{package_digest, LE128 next_load}`, Abandon 3 `{enrollment_id, LE128 marker_generation, terminal_marker_digest}`. Signed by the wallet payment key; nonzero digests; `marker_generation ≥ 1`. |
| Activation | `{version, control, credential, bootstrap: Package, asset: AssetScope, certificates}` | Certificates exactly the issuer; asset digest equals the credential's; an Activate control by the credential's key naming the digest of the verified Bootstrap package. |
| Close loads | `{version, control, credential, package, certificates}` | A CloseLoads control naming the digest and `next_load` of a verified Retiring, Send or Unload package (so carrying Ω(pred) and passing the consumer checks) whose lifecycle is Retiring. |
| Abandonment | `{version, control, payment_key, challenge_digest}` | An Abandon control whose `enrollment_id` and `wallet_id` recompute from `payment_key` and `challenge_digest`, naming the generation and digest of the Terminal Abandoned marker. Signing only after that marker is durable is design only (G2). |

Exactly-once payouts per nullifier and per `credit_id`, recording activation and
closure, and rejecting Abandon after Activate (and Activate or loads after
Abandon) are ledger state: design only, TODO(G6).

## 4. Measured sizes

Worst-case valid envelopes, printed by `size_tests.rs`. The Payment and Request carry
a fee schedule and two Request certificates. σ_send is the measured 3,296-byte
proof of the prototype `sigma_send` (`iroha_kagemusha_proof`, two-level, `k = 12`,
one lane, with this statement encoding; spec §11: σ is not yet measured for the full
§3 core). Ω, σ_recv and the opening size are unmeasured: the rows use the named
placeholders Ω transport proof 4,000 bytes, σ_recv 3,296 bytes and 32 opening
siblings (TODO(G3)).

| Envelope | Bytes / bound | Fixed overhead | Largest fitting proof |
|---|---:|---|---|
| Offer (credential frame 618 / 1,024) | 1,104 / 2,048 | — | — |
| SessionControl (signed ReceiveDeferred) | 338 / 2,048 | — | — |
| Request (fee schedule, 2 certificates) | 1,735 / 10,000 | — | — |
| Payment (σ_send 3,296; Ω proof 4,000) | 8,911 / 10,000 | 1,615 + Ω proof + σ_send | Ω proof 5,089 with the measured σ_send; Ω proof + σ_send 8,385 |
| Credited::Receive (σ_recv 3,296) | 3,975 / 10,000 | 679 + σ_recv | — |
| Credited::Status (Ω(h) proof 4,000, 32 siblings; CreditStatus frame 6,128) | 6,183 / 10,000 | 1,159 + Ω(h) proof + 32 per sibling | Ω(h) proof 7,817 with 32 siblings |
| Lineage (Ω proof 4,000) | 4,413 / 10,000 | 413 + Ω proof | — |
| PolicyData certificates (3) | 698 / 10,000 | — | — |

A Send package alone is 5,358 bytes with a 1,000-byte Ω proof and 8,358 with a
4,000-byte one (1,062 bytes beyond the two proofs). Other measured frames: the
largest Android renewal request (8 certificates totalling 65,536 DER bytes) is
66,021 of 73,728 bytes; a 64-window quota share is 2,941 bytes; the full
65,535-entry blacklist, an online download rather than an envelope, is 2,228,433 of
2,228,736 bytes. Vector frames with 48-byte stand-in σ and Ω proofs: envelopes
Payment 1,708, Credited::Receive 725, Credited::Status 1,277 (3 siblings) and
Lineage 460; standalone scheme 208, certificate 222, credential 618, Receive
recovery capsule 5,970 and fold record 616.

## 5. Vectors

`fixtures/kagemusha/wallet_v1_vectors.json` holds the prefix and digest rule, one
digest vector per role (62: body, preimage, digest), 18 signature vectors with their
high-S twins (`codec_ok` false, `verify_ok` true), the low-S boundary scalars
(`s = floor(n/2)` accepted; `floor(n/2) + 1`, `r` or `s` zero or `n` rejected), nine
envelope vectors (header fields, padding, CRC, canonical bytes, `kgm1:` text and
bound), the 25 frame identities and caps, 29 pinned object frames, the enum tag
table, and the σ-field encodings: the modulus, the element rule, the twelve
Poseidon domains, the element lists of every map leaf and the credit-digest leaf, a
`send_chain` append from the empty chain and a `recv_chain` append, a Send and a
Receive statement, and a Receive successor state's core and rest elements. Poseidon
digests of those lists are not in the vectors yet. **Decided (TODO(G1)):** the Rust
step adds the `P` digests of every list, the `P_bytes` packing vectors (empty input
and 31-byte chunk boundaries), `credit_id`, `proof_digest`, the Payment digest, tree
roots and openings, and the new domains; Kotlin and Swift consume them. Keys come from fixed
scalars; signatures are RFC 6979, frozen to low S; stand-ins follow the labelled
rules in `stand_ins`. The Rust test
`kagemusha_wallet_v1_vectors_file_matches_the_generated_vectors` compares the file
byte for byte; `IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS=1` rewrites it (test-only).
The Kotlin (`KagemushaWalletVectorsV1Test`) and Swift (`KagemushaWalletVectorsV1Tests`)
consumers recompute digests, apply the low-S rule before platform verification,
validate envelope headers and per-kind bounds, and round-trip `kgm1:` text; typed
SDK decoding of message bodies is TODO(G4).

## 6. Carriers

Every peer message travels as one complete canonical envelope frame (§2). A carrier
moves that frame unchanged; its framing is outside every digest and signature and
grants no authority. Before handing a frame to the wallet, a carrier checks it
structurally (`KagemushaWalletWireV1.inspectEnvelope`): the byte cap, the Norito header,
schema hash, flags, padding, CRC and exact field spans, the envelope version, a known
message tag and the per-kind bound, without the expected-scheme check. Swift also checks
the message's top-level version and that its decode-time scheme field is 32 bytes; Kotlin
does not yet (TODO(G4), §7). Nested version fields, the scheme check, typed decoding and
signature verification stay with the wallet (TODO(G4)). The implementations are
Swift `IrohaPeerWireV1`, `IrohaPeerQRV1`, `IrohaPeerNfcV1` and `IrohaPeerNearbyV1`
(`IrohaSwift`, with the platform adapters in `IrohaSwiftMobileTransports`) and the
Kotlin `org.hyperledger.iroha.sdk.offline.IrohaPeer*` types (`kotlin/core-jvm`, with
the Android NFC and Nearby adapters in `client-android`). Their tests carry the
envelope vectors of §5 and structural envelopes of every kind.

**IPM1 message.** An 84-byte header, then the encoded body:

| Offset | Bytes | Content |
|---:|---:|---|
| 0 | 4 | magic `IPM1` |
| 4 | 1 | wire version `1` |
| 5 | 1 | encoding: `0` none, `1` zlib |
| 6 | 2 | profile, big-endian: `1` KAGEMUSHA wallet V1 (`0` is reserved and rejected) |
| 8 | 1 | kind: the envelope message tag (Offer 1, Request 2, Payment 3, Credited 4, SessionControl 5, PolicyData 6, Lineage 7) |
| 9 | 1 | flags `0` |
| 10 | 2 | schema version, big-endian: `1` |
| 12 | 4 | canonical (frame) length, big-endian |
| 16 | 4 | encoded body length, big-endian |
| 20 | 32 | canonical hash: BLAKE2b-256 of `"IROHA-PEER-PAYLOAD-V1" ‖ 0x00 ‖ BE16 profile ‖ kind ‖ BE16 schema ‖ frame` |
| 52 | 32 | wire hash: BLAKE2b-256 of `"IROHA-PEER-MESSAGE-V1" ‖ 0x00 ‖ header bytes 0..52 ‖ body` |

- The canonical payload is exactly one envelope frame whose message tag equals the
  IPM1 kind, within that kind's bound (2,048 bytes for Offer and SessionControl,
  10,000 otherwise). Both lengths are positive and at most 10,000.
- zlib is the RFC 1950 form (`78 9C`, DEFLATE, Adler-32 of the frame). A sender uses it
  only when it saves at least 32 bytes and at least one 256-byte QR shard; a decoder
  rejects any other zlib header or length pair.
- Decoding checks the header and both bounds before allocating, then the total length,
  the wire hash, decompression to exactly the declared length, the envelope, and the
  canonical hash. The stream identifier is the first 16 bytes of the wire hash.

**QR.** A QR text is `IQR1:` ‖ Base45 (RFC 9285, canonical) of one `IRQR` frame ‖ `:`,
at most 700 bytes. An `IRQR` frame is `"IRQR" ‖ 1 ‖ frame kind ‖ BE16 profile ‖ IPM1
kind ‖ 0 ‖ stream identifier (16) ‖ BE16 index ‖ BE16 total ‖ BE16 payload length ‖
payload ‖ BE32 CRC-32C` of everything before the checksum; frame kinds are complete 0,
header 1, data 2 and parity 3. A message whose complete frame fits one text may be shown
as that single static text. Otherwise the sequence is the header frame (the 84-byte IPM1
header), then
256-byte data shards (the last zero-filled) with one XOR parity shard per pair, in the
order D0, D1, P0, D2, D3, P1, …, and the identical header again after every 12
non-header frames; `total` counts data shards (40 for a 10,000-byte body). A receiver
accepts frames in any order, ignores identical duplicates, recovers one missing shard
per pair from parity, quarantines a stream on a conflicting duplicate or an invalid
message, bounds active streams (3) and frames before the header (12 frames, 3,072 bytes
per stream), and decodes only a complete message.

**NFC.** ISO/IEC 7816 application `F0504B45504B524E464301`, proprietary class `0x80`,
chunks of at most 4,096 bytes and messages of at most 10,084 bytes (header plus 10,000).
One session carries the receiver-hosted Request, the payer's Payment (begun, written in
contiguous chunks and committed) and the receiver's durable acknowledgement, which is
the Credited envelope. The receiver exposes Credited only after the Payment is durably
admitted; a resumed or retried session must present the byte-identical Payment. The
Kotlin carrier also requires the Payment to carry exactly the hosted Request's signed
body and signature, and Credited to name that Request's scheme
(`KagemushaWalletWireV1.requireExchangeBinding`); the Credited evidence itself is checked
by the wallet's typed decoder (TODO(G4)). TODO(G4): Offer and SessionControl
over NFC; until then the receiver obtains the Offer over another carrier before it
builds the Request.

**Nearby and Petal.** A Nearby session (service `org.hyperledger.iroha.kagemusha.transfer.v1`)
authenticates both devices and then carries complete IPM1 messages of at most 32,704
bytes in encrypted, per-direction sequenced records; the Kotlin session opens a record
only to a verified IPM1 message of its profile. Petal Stream ([petal_stream.md](petal_stream.md))
carries the encoded IPM1 message with the IPM1 kind as its `kind` byte. Transport
encryption and checksums never replace the wallet's verification.

## 7. Open items

- TODO(G4): Offer and SessionControl over the NFC carrier (§6).
- TODO(G4): the Kotlin `inspectEnvelope` accepts a message whose own top-level version
  is not 1 or whose decode-time scheme field is malformed, which the Swift carrier
  and the Rust decoder reject (§6); no carrier checks nested version fields yet.
- TODO(G3): the PIPA-v1 σ layout, the Ω transport-proof layout and their exact
  lengths. Once the artifacts freeze, the σ and Ω caps are the exact lengths in the
  verifying-key allowlist (§3.1 Relation), which must satisfy Ω proof + largest
  σ_send ≤ 10,000 − F_payment, 8,385 bytes with the §4 overhead (R9). Until then σ
  and Ω are bounded only by their frames.
- TODO(G1): implement every **Decided** rule above (owner answers of 2026-10-05):
  `iroha_pasta` as an `iroha_data_model` dependency with a coherent `Cargo.lock`;
  native `P` and `P_bytes` values with canonical-encoding checks; deletion of the
  eight SHA roles of §1; the new core, rest, leaves and effect element lists; the
  Poseidon blacklist and quota-window trees; receiver matching by wallet and payment
  key; the verifying-key allowlist and its `verifying_key_set_digest` preimage; and
  regenerated vectors with their Kotlin and Swift consumers.
- TODO(G3): the capsule map-opening layout and the `artifact_inventory_digest`
  preimage. `iroha_kagemusha_proof` exposes the statement encoding of §3.2, but its
  core layout, chain entries, credit identifier and domains are still the
  prototype's.
- Open owner questions: the tree shapes and empty roots of the five state maps,
  which the data model now computes (§3.2); and whether the `lineage`,
  `credit-opening`, `credit-status` and `credited` digests, which cover Ω or a large
  opening and which `Λ_archive` may recompute in-circuit, also move to `P_bytes`.
- TODO(owner), kept as is: a Send capsule retains the Request message (§8);
  `Λ_load` and `Λ_unload` do not verify ChargeQuote signatures; a Retiring wallet's
  refusal to issue Requests is wallet behaviour (TODO(G4)).
- TODO(G5): `app_policy` and `enrollment_policy` preimages with the Torii
  enrollment family; issuer verification of renewal evidence.
- TODO(G4/G6): JavaScript, Python and C# consumers when their wire copies migrate.
- TODO(G6): the ledger instruction family; TODO(G3/G6): the release install path
  carrying the artifact manifest.

## 8. Differences from the implementation design

- The full blacklist cap is 2,228,736 bytes, not the design's 2,228,224, which
  would reject a maximum list (2,228,433 bytes). SDKs take the cap from the vectors.
- The marker terminal reason, output descriptor kind and retained-input role are
  Norito enums, not `u8` fields: their frames carry 4-byte tags, and the `marker`,
  `capsule` and `completion` digests cover those frames.
- Frame padding follows the type's archived alignment, not every contained `u128`:
  a `u128` reached only through a sequence (quota share windows) adds no padding.
  Use the per-type table of §2.
- The credit-opening siblings are one flat byte string of 32-byte values, not a
  sequence of 32-byte arrays: Norito would spend 65 bytes per array element inside
  the 10,000-byte Credited bound.
- The ArchiveSent capsule also retains the Request, which the Payment binds only
  by digest. A Send capsule retains the Request it consumed (its fee schedule) and a
  Load capsule the voucher's certificate set, which the design's fold-witness list
  omits although `Λ` verifies them (proposal §§3.2, 4.1).
- Credited is verified against the payer's scheme as well as its Request and
  Payment, so a relation identity other than the scheme's is rejected natively.
