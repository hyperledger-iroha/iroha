# KAGEMUSHA wallet wire record V1

**Status.** This record describes the canonical G1 wire objects of the single
KAGEMUSHA design ([proposal](kagemusha_single_design_proposal.md) §10, G1). They
are implemented in `iroha_data_model::kagemusha::kagemusha_wallet_v1`
(`crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1.rs` and its child
files). Their cross-language vectors are `fixtures/kagemusha/wallet_v1_vectors.json`.
Kotlin `org.hyperledger.iroha.sdk.offline.KagemushaWalletWireV1` (`kotlin/core-jvm`)
and Swift `KagemushaWalletWireV1` (`IrohaSwift`) consume the vectors. The protocol
relation, the Advance provider, the bridge, the ledger instructions and the Torii
routes do not use these objects yet. Proof bytes, relation bindings and empty-map
roots in the vectors are labelled stand-ins. The code is authoritative; a change
to it updates this record and the vectors together.

Notation: `‖` is concatenation; `LE16`…`LE128` are little-endian unsigned
integers; every name without a width is a raw 32-byte digest, identifier or nonce,
and all-zero means "none"; `key` is a 65-byte uncompressed SEC1 P-256 key
(`0x04 ‖ X ‖ Y`, on the curve); `sig` is a 64-byte big-endian `r ‖ s`; `tag` is one
byte. Numbers in parentheses are exact transcript lengths, pinned by tests.
"Design only" marks a rule these objects do not enforce or test.

## 1. Digests, transcripts and signatures

```text
H(role, body) = SHA-256("iroha:kagemusha:wallet:v1:" ‖ role ‖ 0x00 ‖ LE64(len(body)) ‖ body)
```

The 26-byte prefix is `69726f68613a6b6167656d757368613a77616c6c65743a76313a` and
`role` is an ASCII label below. Check value: `H("scheme", empty)` =
`90882608a8e8892521a2661be1e81c3fbfca0f8773e621000daed362a37485a5`.

- **Transcripts.** A body is a fixed-layout transcript of the object's fields in
  declaration order: fixed-width integers, raw digests, keys and signatures, and
  enums as a one-byte tag equal to the Norito tag. Where variants differ, the tag
  is followed by the variant's fields and zero fill to a pinned union width. Nested
  fixed records are inlined. A reference to a signer certificate is its 32-byte
  certificate digest. Only `account`, `certificate-set`, `evidence`, `proof`,
  `marker`, `capsule` and `completion` have variable-length bodies.
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

Role table (all 59 labels of `KagemushaWalletDigestRoleV1`; `A → B` names a signed
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
| `statement` | §3.2 | 484 |
| `proof` | raw proof bytes | var |
| `receipt-body` → `receipt` | §3.2 (derived, never transmitted) | 370 → 96 |
| `package` | `statement_digest ‖ proof_digest ‖ receipt_digest` | 96 |
| `operation-id` | `wallet_id ‖ tag operation_kind ‖ input` | 65 |
| `unload-nullifier` | `scheme_id ‖ wallet_id ‖ LE128 redeem_ordinal` | 80 |
| `scheme-policy-body` → `scheme-policy` | §3.3 | 142 → 96 |
| `fee-schedule-body` → `fee-schedule` | §3.3 | 191 → 96 |
| `blacklist-body` → `blacklist` | §3.3 | 118 → 96 |
| `blacklist-leaf`, `blacklist-node` | `lower ‖ upper`; `left ‖ right` | 64 |
| `quota-share-body` → `quota-share` | §3.3 | 190 → 96 |
| `quota-window`, `quota-node` | window leaf (§3.3); `left ‖ right` | 33; 64 |
| `time-anchor-body` → `time-anchor` | §3.3 | 138 → 96 |
| `charge-quote-body` → `charge-quote` | §3.3 | 219 → 96 |
| `offer-body` | §3.4 (signed; no object-digest role) | 194 |
| `session-control-body` | §3.4 (signed or unsigned; no object-digest role) | 197 |
| `request-body` → `request` | §3.4 | 354 → 96 |
| `credit` | the `request-body` transcript: `credit_id = H("credit", ·)` | 354 |
| `dependencies` | `LE32 3 ‖ payer issuer ‖ receiver issuer ‖ fee signer or zero` (certificate digests) | 100 |
| `payment` | `LE16 1 ‖ request_digest ‖ payer_credential_digest ‖ package_digest ‖ certificate-set digest` | 130 |
| `credit-status-statement` | §3.4 | 370 |
| `credited` | §3.4 | 227 |
| `output` | `tag operation_kind ‖ statement_digest ‖ proof_digest ‖ kind input (96, zero-filled)` | 161 |
| `marker`, `capsule`, `completion` | complete canonical Norito frame of the local object (§3.5) | var |
| `voucher-body` → `voucher` | §3.6 | 250 → 96 |
| `ledger-control-body` | §3.6 (signed; no object-digest role) | 211 |

Map leaves are not SHA roles; they use the Poseidon domains of §3.2.

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
  vectors pin the name and schema hash of all 24 framed types (envelope:
  `f03a9dc47142299cad9ffe0b2115fd42`).
- `p` is a property of the type, never inferred from the bytes. It is 8 when a
  `u128` is reachable without passing through a sequence (archived alignment 16;
  the envelope payload starts at byte 48), and 0 otherwise. The table below lists
  it per type. `armv7` is not an admitted native target: its `u128` alignment would
  change the padding.
- Payload (Norito derived layout under `COMPACT_LEN`): a record is its fields in
  declaration order, each `varint len ‖ payload`; integers are little-endian fixed
  width; a digest field is `0x20 ‖ 32 bytes`, a key `0x41 ‖ 65 bytes`, a signature
  `0x40 ‖ 64 bytes`; an enum is an `LE32` tag (equal to the transcript tag) followed
  by its variant's fields, each length-prefixed; `Vec<u8>` is `LE64 count ‖ bytes`;
  any other sequence is `LE64 count ‖ (varint len ‖ element)…`. `AccountId` and
  `AssetDefinitionId` use their own canonical encodings.
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
  root.
- Flipping any byte of a vector Credential, Certificate, Offer, Request, Payment,
  Credited, Package, LoadVoucher, SchemePolicy, FeeSchedule, Blacklist, QuotaShare,
  TimeAnchor, ChargeQuote, LedgerControl or ArtifactManifest frame fails decoding
  or validation, or changes the object digest (tested). The quota share and
  blacklist digests cover only body and signature; the body's root and count bind
  the carried windows and entries. State, commitments, anchored time, unsigned
  session controls and renewal evidence bytes have no such binding.

| Bound | Value |
|---|---:|
| Envelope, Offer and SessionControl (complete frame) | 2,048 |
| Envelope, Request, Payment, Credited and PolicyData | 10,000 |
| `kgm1:` text, session / message | 2,736 / 13,339 |
| Transition proof bytes (**provisional**, TODO(G3)) | 1..=6,016 |
| `CreditStatus` proof bytes (**provisional**, TODO(G3)) | 1..=2,000 |
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
| `SchemePolicyV1` | 1,024 | 0 | `LoadVoucherV1` | 1,024 | 8 |
| `FeeScheduleV1` | 1,024 | 8 | `UnloadClaimV1` | 16,384 | 8 |
| `BlacklistV1` | 2,228,736 | 0 | `FeeClaimV1` | 16,384 | 8 |
| `QuotaShareV1` | 8,192 | 0 | `LedgerControlV1` | 1,024 | 8 |
| `TimeAnchorV1` | 512 | 0 | `ActivationV1` | 16,384 | 8 |
| `ChargeQuoteV1` | 1,024 | 8 | `CloseLoadsV1` | 16,384 | 8 |
| `EnvelopeV1` | per kind | 8 | `AbandonmentV1` | 1,024 | 8 |

Type names omit the `KagemushaWallet` prefix. The blacklist cap is
`65,536 × 34 + 512`: each canonical entry takes 34 bytes. The blacklist is not a
peer message: a wallet downloads it only while online, from the issuer or ledger,
as this standalone frame, and peers never relay it, so its size does not bear on
the envelope bounds. The package cap applies
where a completion record's output is decoded as a package.

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
| Relation (`relation`) | `LE16 1 ‖ eq_protocol_digest ‖ ep_protocol_digest ‖ native_profile_digest ‖ verifying_key_set_digest ‖ artifact_inventory_digest` | Gives `relation_id`, fixed for the scheme's lifetime. |
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

### 3.2 State, leaves, effects, statement, proof, receipt, package

The private state is not transmitted; it is committed by the recursive state owner
(TODO(G3)) and travels only inside a recovery capsule. Its frame fields are
`version, scheme_id, asset_digest, wallet_id, credential_digest, lifecycle
(Active 1, Retiring 2), balance, sequence, next_send, next_load, next_redeem
(LE128 each), consumed_credit_root, pending_outgoing_root, load_recovery_root,
redeem_recovery_root, fee_claim_root, policy, state_nonce`. The policy part is
`regulatory_policy, scheme_policy, LE64 policy_epoch, LE32 enabled_controls,
fee_schedule, blacklist, LE64 blacklist_version, blacklist_root,
LE64 blacklist_issued_at_ms, quota_share, LE64 quota_share_id, quota_windows_root,
quota_usage_root, LE64 lease_expires_at_ms, LE64 accepted_time_floor_ms,
time_anchor`. Identities, map roots, `quota_usage_root` and `state_nonce` are
nonzero. A scheme policy, blacklist or quota share is held exactly when its digest,
its epoch, version or share id, and its root (if any) are all nonzero; without a
scheme policy, `enabled_controls` and `fee_schedule` are zero. `enabled_controls`
is the policy's enabled controls masked by the credential's permitted controls.
Bootstrap state has zero balance, ordinals and sequence, the map owner's empty
roots, and zero policy fields except the credential's regulatory policy and lease
and the empty quota-usage root. A commitment is `eq ‖ ep` (64); all-zero is only
the Bootstrap predecessor.

Map leaves (field order and limb rule; Poseidon hashing and vectors are TODO(G3)):
each digest becomes two `u128` limbs, low 16 bytes first (little-endian); every
integer, and the window kind tag, is widened to `u128`.

| Map | Domain (`u64` from 8 LE ASCII bytes) | Fields |
|---|---|---|
| consumed credit (permanent) | `kgwccrd1` | `credit_id, payment_digest` |
| pending outgoing | `kgwpout1` | `credit_id, receiver_wallet_id, send_ordinal, amount, fee, request_digest` |
| load recovery | `kgwload1` | `ordinal, voucher_digest, amount` |
| redeem recovery | `kgwrdm_1` | `ordinal, nullifier, amount, online_charge` |
| fee claim | `kgwfee_1` | `credit_id, fee, fee_schedule_digest` |
| quota usage | `kgwquse1` | `window_kind, window_start_ms, window_end_ms, used` |

Operation kinds and effects (effect transcript (193) = `tag ‖ fields ‖ zero fill to 192`;
the effect tag equals the operation kind):

| Tag | Kind | Effect fields (width) | `operation-id` input | `output` kind input |
|---:|---|---|---|---|
| 1 | Bootstrap | `enrollment_id ‖ enrollment_marker` (64) | `enrollment_id` | zero |
| 2 | Load | `voucher ‖ LE128 load_ordinal ‖ LE128 amount ‖ LE128 online_charge` (80) | `voucher` | `voucher` |
| 3 | Send | `credit_id ‖ receiver_wallet_id ‖ LE128 send_ordinal ‖ LE128 amount ‖ LE128 fee ‖ request ‖ dependencies ‖ LE64 accepted_lower_ms ‖ LE64 accepted_upper_ms` (192) | `credit_id` | `request ‖ payer credential digest ‖ Payment certificate-set digest` |
| 4 | Receive | `credit_id ‖ payer_wallet_id ‖ payment ‖ LE128 amount` (112) | `credit_id` | `payment` |
| 5 | ArchiveSent | `credit_id ‖ credited` (64) | `credit_id` | `credited` |
| 6 | Unload | `nullifier ‖ LE128 redeem_ordinal ‖ LE128 amount ‖ LE128 online_charge ‖ charge_quote` (112) | `nullifier` | `nullifier` |
| 7 | RefreshPolicy | `tag update_kind ‖ update ‖ LE64 accepted_time_floor_ms` (41) | `update` | zero |
| 8 | Retiring | none (0) | 32 zero bytes | zero |

Update kinds: Credential 1, SchemePolicy 2, Blacklist 3, QuotaShare 4, TimeAnchor 5;
`update` is the applied object's digest. Effect rules: nonzero digests; Send,
Receive and Unload amounts positive; Send `amount + fee` fits `u128` and
`accepted_lower_ms ≤ accepted_upper_ms`; Unload `online_charge ≤ amount` and
`charge_quote` nonzero iff `online_charge > 0`.

- **Statement** (`statement`, 484): `LE16 version ‖ scheme_id ‖ relation_id ‖
  credential_digest ‖ asset_digest ‖ tag lifecycle ‖ LE128 sequence ‖ LE128
  next_load ‖ predecessor(64) ‖ successor(64) ‖ effect(193)`. Lifecycle, sequence
  and `next_load` are the successor's. Bootstrap alone has sequence 0, a zero
  predecessor, Active and `next_load` 0; every other statement has a complete
  predecessor. The successor is complete. Retiring yields lifecycle Retiring; Load
  yields `next_load = load_ordinal + 1`; a Credential refresh has
  `update = credential_digest`. A successor chains on the predecessor's successor
  commitment with sequence plus one; lifecycle changes only by Retiring from
  Active; `next_load` changes only by Load at the predecessor's `next_load`;
  `credential_digest` changes only by a Credential refresh. Against its
  credential: Bootstrap names the credential's `enrollment_id`, Send and Receive
  never name the wallet itself, and an Unload nullifier recomputes.
- **Operation and nullifier.** `operation_id = H("operation-id", wallet_id ‖ tag kind ‖ input)`;
  `nullifier = H("unload-nullifier", scheme_id ‖ wallet_id ‖ LE128 redeem_ordinal)`.
- **Proof.** Frame `{bytes}`; `proof_digest = H("proof", bytes)` over the raw bytes,
  recomputed on every use. The layout and in-circuit binding are TODO(G3).
- **Receipt.** Frame `{version, operation_id, capsule_digest, signature}`. Its
  signed body is derived by every verifier and never transmitted: `receipt-body`
  (370) = `LE16 version ‖ scheme_id ‖ wallet_id ‖ provider_contract` (from the
  credential) `‖ LE128 sequence ‖ operation_id ‖ predecessor ‖ successor` (from the
  statement) `‖ statement_digest ‖ proof_digest ‖ capsule_digest`. The credential's
  payment key signs it; the carried `operation_id` must equal the recomputed one.
- **Package.** Frame `{version, statement, proof, receipt}`;
  `package_digest = H("package", statement_digest ‖ proof_digest ‖ receipt_digest)`,
  defined only after the receipt verifies under the credential's payment key.

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
  a local witness that is never transmitted.
- **Quota tree.** A window leaf (33) is `tag kind ‖ LE64 start_ms ‖ LE64 end_ms ‖
  LE128 limit` (Daily 1, Monthly 2; half-open `[start, end)`); an empty slot is
  `H("quota-window", 33 zero bytes)`; the root is a depth-6 tree over 64 slots,
  windows first, with `H("quota-node", left ‖ right)`.
- **Time.** The local anchored time `{anchor, boot_id, LE64 request_monotonic_ms,
  LE64 receive_monotonic_ms}` gives, at monotonic reading `m` in the same boot,
  `[T + (m − m_rcv), T + (m − m_req)]`, valid when `m ≥ m_rcv` and
  `m_rcv − m_req ≤ time_anchor_max_response_ms`. A Send binds `L = max(floor,
  anchor lower, receiver_accepted_time_ms)` and `U = max(anchor upper, L)` (`U = L`
  without an anchor). An active time-dependent control without the committed
  same-boot anchor refuses Send. A deadline (lease expiry, quota share expiry) has
  passed iff `U ≥ deadline`. Blacklist age is `U − blacklist_issued_at_ms`. A quota
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
Payment 3, Credited 4, SessionControl 5 and PolicyData 6. The scheme checked at
decode is the body scheme (Offer, Request), the Request scheme (Payment), the
receiver credential scheme (Credited) or the message's own field.

| Message | Frame and transcript | Interoperability rules |
|---|---|---|
| Offer | `{body, payer_credential, certificates, signature}`; `offer-body` = `LE16 version ‖ scheme_id ‖ asset_digest ‖ payer_wallet_id ‖ payer_credential_digest ‖ LE128 next_send ‖ LE128 amount ‖ session_nonce` | Payer payment key signs. Body matches the credential's scheme, asset, wallet and digest; `amount > 0`; certificates exactly the payer issuer. No debit or credit authority. |
| Request | `{body, receiver_credential, fee_schedule, certificates, signature}`; `request-body` = `LE16 version ‖ scheme_id ‖ asset_digest ‖ payer_wallet_id ‖ receiver_wallet_id ‖ LE128 send_ordinal ‖ receiver_credential_digest ‖ LE128 amount ‖ fee_schedule ‖ LE128 fee ‖ LE64 policy_epoch ‖ scheme_policy ‖ LE64 receiver_accepted_time_ms ‖ certificates ‖ nonce` | Receiver payment key signs. Slot None 0 or Present 1 `{schedule}`. `amount > 0`; payer ≠ receiver wallet; `amount + fee` fits; `policy_epoch = 0` iff `scheme_policy` zero. None requires zero `fee_schedule` and `fee`; Present requires the digest, the Request's scheme and asset, and `fee = fee(amount)`. Certificates exactly the receiver issuer (Enrollment) plus the fee signer (RegulatoryPolicy) when present; their set digest is `body.certificates`. |
| Payment | `{version, request, payer_credential, send: Package, certificates}` | Certificates are exactly the payer issuer unless the Request's set already carries it (then empty). Payer credential matches the Request's scheme, asset and payer wallet with a key different from the receiver's. The Send effect equals the Request's credit, receiver, ordinal, amount, fee, `request_digest` and positional dependencies, with `accepted_lower_ms ≥ receiver_accepted_time_ms`; the package verifies under the payer credential; the Request signature verifies. Only then is `payment_digest` defined. |
| Credited | `{version, credit_id, payment_digest, receiver_credential, evidence, certificates}`; `credited` (227) = `LE16 version ‖ credit_id ‖ payment_digest ‖ receiver_credential_digest ‖ tag evidence ‖ package_digest ‖ status statement digest or zero ‖ status proof digest or zero ‖ certificate-set digest` | Evidence Receive 1 `{package}` or Status 2 `{current: Package, status: CreditStatus}`; certificates exactly the receiver issuer. Receive: a Receive package for this credit and Payment that verifies under the receiver credential. Status: the claim's credit and Payment equal; scheme, relation and asset equal the current statement; wallet and credential digest equal the receiver credential; `current`, sequence, statement and receipt digests equal the verified current package. The payer matches by `receiver_wallet_id`, never by credential digest; `ArchiveSent` also requires the retained Payment's digest and pending leaf. |
| CreditStatus | `{version, statement, proof}`; `credit-status-statement` = `LE16 version ‖ scheme_id ‖ relation_id ‖ asset_digest ‖ receiver_wallet_id ‖ receiver_credential_digest ‖ credit_id ‖ payment_digest ‖ current.eq ‖ current.ep ‖ LE128 current_sequence ‖ current_statement_digest ‖ current_receipt_digest` | Read-only membership; nonzero fields; complete `current`; proof within the provisional 2,000 bytes. A proof of absence is not evidence. |
| SessionControl | `{version, scheme_id, asset_digest, sender_wallet_id, peer_wallet_id, session_nonce, kind, LE16 reason, credit_id, auth}`; `session-control-body` = all fields except `auth`, kind as a tag | Kinds SetupDeclined 1, UnsupportedScheme 2, ReceiveDeferred 3, Close 4; auth Unsigned 0 or Signed 1 `{signature}`. Scheme, asset and nonce nonzero; sender nonzero except UnsupportedScheme; peer zero (unknown) or not the sender; opaque `reason` only for SetupDeclined and ReceiveDeferred; `credit_id` nonzero iff ReceiveDeferred. Unsigned only for UnsupportedScheme and for SetupDeclined before the sender's own Offer or Request; otherwise signed by the payment key of that session's Offer or Request credential. Invalid controls are dropped. |
| PolicyData | `{version, scheme_id, asset_digest, item}` | Items SchemePolicy 1, FeeSchedule 2, Certificates 3 (non-empty); tag 4 is unused, because the blacklist is never peer-carried (§3.3). Item scheme equals `scheme_id`; scheme policy and fee schedule asset equal `asset_digest`. Signers are selected by certificate digest. |

- **Unknown scheme.** A receiver may decode an Offer envelope through the per-kind
  bound without the scheme check, validate only its body, and reply with an
  unsigned UnsupportedScheme naming the Offer's scheme, asset and session nonce,
  zero sender and the payer as peer.
- **Send rule** (native pre-check; relation enforcement is design only, G3): the
  Request names the payer's wallet and `next_send`; payer `policy_epoch ≥` the
  Request's, with an equal `scheme_policy` at equal epochs; `fee_schedule` equals
  the payer's held schedule; payer and receiver keys differ; balance
  `≥ amount + fee`.

### 3.5 Custody objects (local, never transmitted)

Their enums are Norito enums: frames carry 4-byte tags; the `output` transcript
carries one byte.

| Object | Frame | Interoperability rules |
|---|---|---|
| Marker (`marker` over the frame) | `{version, scheme_id, asset_digest, wallet_id, payment_key, LE128 generation, state}`; state Enrollment 1 `{challenge_digest, enrollment_id}`, Head 2 `{LE128 sequence, operation_id, head, capsule_digest, predecessor_capsule_digest}`, Terminal 3 `{reason, last_capsule_digest}` | Reasons Abandoned 1, CustodyDeleted 2. Generation 0 iff Enrollment, whose `enrollment_id` and `wallet_id` recompute. Head: complete `head`; zero predecessor capsule iff sequence 0. Terminal: zero last capsule iff Abandoned. Successors raise the generation by one: Enrollment → Head (sequence 0) or Terminal Abandoned; Head → Head (sequence + 1, linked capsule) or Terminal CustodyDeleted (last = capsule). The Bootstrap `enrollment_marker` is the generation-0 marker digest. |
| Output descriptor | `{kind, digest}` | `digest = H("output", ·)` with the kind input of §3.2: it is receipt-free, so the capsule freezes before the receipt. |
| Recovery capsule (`capsule` over the frame) | `{version, scheme_id, wallet_id, operation_id, kind, predecessor_capsule_digest, successor_state, statement, proof, map_openings: [bytes], retained_inputs: [{role, bytes}], output}` | Retained roles: Request 1, Payment 2, Credited 3, LoadVoucher 4, ChargeQuote 5, PolicyUpdate 6, CertificateSet 7, Credential 8. State and statement agree on scheme, wallet, credential, asset, lifecycle, sequence and `next_load`; kind, effect kind and output kind agree; `operation_id` recomputes; zero predecessor capsule iff sequence 0; a non-Send output rebuilds from statement and proof; each opening and retained input is non-empty. The receipt signs `capsule_digest`. The opening layout is TODO(G3). |
| Completion record (`completion` over the frame) | `{version, wallet_id, operation_id, capsule_digest, receipt, output: bytes}` | Decoded against the expected wallet. `output` is the canonical Payment frame for Send and the canonical Package frame otherwise. Its receipt equals the record's; its receipt-free parts rebuild the capsule's output descriptor; the receipt verifies over `capsule_digest`. |

### 3.6 Ledger objects

| Object | Frame and transcript | Interoperability rules |
|---|---|---|
| Load voucher (`voucher-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ wallet_id ‖ LE128 ordinal ‖ LE128 amount ‖ LE128 online_charge ‖ charge_quote ‖ transaction_hash ‖ LE64 block_height ‖ authorizer_certificate` | LoadAuthorization-role signer. `block_height ≥ 1`; `amount > 0`; `charge_quote` nonzero iff `online_charge > 0`, and then a Load quote with the same scheme, asset, wallet, ordinal, net amount and charge; `amount + online_charge` fits. A state absorbs it only at its `next_load`. |
| Unload claim | `{version, credential, package, account: AccountId, charge, certificates}`; charge None 0 or Quoted 1 `{quote, beneficiary: AccountId}` | Certificates exactly the issuer plus the quote signer when quoted. `H("account", account)` equals the credential's account digest. The package is a verified Unload; `charge` is Quoted iff the effect names a quote, which must be an Unload quote for the effect's exact terms, with `H("account", beneficiary)` as its beneficiary. Payout `amount − online_charge`. |
| Fee claim | `{version, payment, beneficiary: AccountId}` | Valid Payment with a fee schedule and `fee > 0`; `H("account", beneficiary)` equals the schedule's beneficiary. |
| Ledger control (`ledger-control-body`) | `LE16 version ‖ scheme_id ‖ asset_digest ‖ wallet_id ‖ tag action ‖ fields zero-filled to 80 ‖ nonce` | Actions Activate 1 `{package_digest}`, CloseLoads 2 `{package_digest, LE128 next_load}`, Abandon 3 `{enrollment_id, LE128 marker_generation, terminal_marker_digest}`. Signed by the wallet payment key; nonzero digests; `marker_generation ≥ 1`. |
| Activation | `{version, control, credential, bootstrap: Package, asset: AssetScope, certificates}` | Certificates exactly the issuer; asset digest equals the credential's; an Activate control by the credential's key naming the digest of the verified Bootstrap package. |
| Close loads | `{version, control, credential, package, certificates}` | A CloseLoads control naming the digest and `next_load` of a verified package whose lifecycle is Retiring. |
| Abandonment | `{version, control, payment_key, challenge_digest}` | An Abandon control whose `enrollment_id` and `wallet_id` recompute from `payment_key` and `challenge_digest`, naming the generation and digest of the Terminal Abandoned marker. Signing only after that marker is durable is design only (G2). |

Exactly-once payouts per nullifier and per `credit_id`, recording activation and
closure, and rejecting Abandon after Activate (and Activate or loads after
Abandon) are ledger state: design only, TODO(G6).

## 4. Measured sizes

Worst-case valid envelopes at the provisional proof caps, printed by `size_tests.rs`
(the Payment carries a fee schedule and 3 certificates; the Status current package
is a Send):

| Envelope | Bytes / bound | Largest fitting proof |
|---|---:|---|
| Offer | 1,104 / 2,048 | — |
| SessionControl (signed ReceiveDeferred) | 338 / 2,048 | — |
| Request (fee schedule, 2 certificates) | 1,735 / 10,000 | — |
| Payment (6,016-byte proof) | 9,202 / 10,000 | 6,814 |
| Credited::Receive (6,016-byte proof) | 7,510 / 10,000 | 8,506 |
| Credited::Status (6,016 + 2,000 proof bytes) | 9,999 / 10,000 | 6,017 with a 2,000-byte status proof; 2,001 with a 6,016-byte transition proof (8,017 combined) |
| PolicyData certificates (3) | 698 / 10,000 | — |

Other measured frames: the largest Android renewal request (8 certificates totalling
65,536 DER bytes) is 66,021 of 73,728 bytes; the full 65,535-entry blacklist, an
online download rather than an envelope, is 2,228,433 of 2,228,736 bytes. Vector
frames with 48-byte stand-in proofs: envelopes Payment 3,232, Credited::Receive
1,540 and Credited::Status 2,051; standalone scheme 208, certificate 222,
credential 618 and Receive recovery capsule 4,788.

## 5. Vectors

`fixtures/kagemusha/wallet_v1_vectors.json` holds the prefix and digest rule, one
digest vector per role (body, preimage, digest), 17 signature vectors with their
high-S twins (`codec_ok` false, `verify_ok` true), the low-S boundary scalars
(`s = floor(n/2)` accepted; `floor(n/2) + 1`, `r` or `s` zero or `n` rejected), eight
envelope vectors (header fields, padding, CRC, canonical bytes, `kgm1:` text and
bound), the 24 frame identities and caps, 28 pinned object frames, and the enum
tag table. Keys come from fixed scalars; signatures are RFC 6979, frozen to low S.
The Rust test `kagemusha_wallet_v1_vectors_file_matches_the_generated_vectors`
compares the file byte for byte; `IROHA_UPDATE_KAGEMUSHA_WALLET_VECTORS=1`
rewrites it (test-only). The Kotlin (`KagemushaWalletVectorsV1Test`) and Swift
(`KagemushaWalletVectorsV1Tests`) consumers recompute digests, apply the low-S rule
before platform verification, validate envelope headers and per-kind bounds, and
round-trip `kgm1:` text; typed SDK decoding of message bodies is TODO(G4).

## 6. Open items

- TODO(G3): concrete proof layout, in-circuit `proof_digest` binding, and measured
  caps replacing the provisional 6,016 and 2,000 bytes.
- TODO(G3): Poseidon leaf vectors under the six map domains and the empty-map
  roots (the vectors use stand-ins); the capsule map-opening layout; the
  `verifying_key_set_digest` and `artifact_inventory_digest` preimages.
- TODO(G5): `app_policy` and `enrollment_policy` preimages with the Torii
  enrollment family; issuer verification of renewal evidence.
- TODO(G4/G6): JavaScript, Python and C# consumers when their wire copies migrate.
- TODO(G6): the ledger instruction family; TODO(G3/G6): the release install path
  carrying the artifact manifest.
- Owner decision before G3 freezes the relation: Credited::Status measures 9,999 of
  10,000 bytes at 6,016 + 2,000 proof bytes. The recorded alternative carries the
  current package as `{statement, proof_digest, receipt}` and has the CreditStatus
  relation verify the current proof recursively.

## 7. Differences from the implementation design

- The full blacklist cap is 2,228,736 bytes, not the design's 2,228,224, which
  would reject a maximum list (2,228,433 bytes). SDKs take the cap from the vectors.
- The marker terminal reason, output descriptor kind and retained-input role are
  Norito enums, not `u8` fields: their frames carry 4-byte tags, and the `marker`,
  `capsule` and `completion` digests cover those frames.
- Frame padding follows the type's archived alignment, not every contained `u128`:
  a `u128` reached only through a sequence (quota share windows) adds no padding.
  Use the per-type table of §2.
