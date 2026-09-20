# SoraFS final-promotion receipt V1

This specification describes the implemented outer signer-receipt boundary.
Its source authorities are the [Manifest receipt](../../crates/sorafs_manifest/src/signer/final_promotion.rs),
[statement parser](../../crates/sorafs_manifest/src/signer/final_promotion/statement.rs),
[public evidence verifier](../../crates/sorafs_manifest/src/signer/final_promotion/evidence.rs),
and [daemon service](../../crates/irohad/src/signer_operation/final_promotion.rs).
V1 is the sole accepted contract; there are no compatibility aliases or fallback profiles.

## Identity and encoding

`SignerRoleV1::FinalPromotionProvenance = 14` has label
`final_promotion_provenance`, domain
`sorafs.production-readiness.final-promotion-provenance.v1`, Ed25519 keys,
and purpose `FinalPromotionProvenance { deployment_id }`.
Foundational-promotion and release-manifest roles cannot substitute for it.
The generic external signer service currently rejects this role because its
purpose-specific operation dispatch is unfinished. The dedicated signer provider
interface supports software and optional hardware implementations under the same
authorization contract; hardware custody is not required.

All binary documents use canonical, schema-identified Norito frames; malformed,
noncanonical, trailing, oversized, and cross-schema inputs are rejected.
The receipt has magic `IRSFPR01`, version 1, and a 64 KiB maximum.
The request and receipt retain these exact Norito identities:

```text
sorafs_manifest::signer::final_promotion::SignerFinalPromotionRequestV1
sorafs_manifest::signer::final_promotion::SignerFinalPromotionReceiptV1
```

Public evidence documents have a 64 KiB maximum. Their identities are under
`sorafs_manifest::signer::final_promotion::evidence::`:

| Type suffix | V1 magic |
| --- | --- |
| `SignerFinalPromotionEvidencePolicyV1` | `IRSFP001` |
| `SignerFinalPromotionEvidenceTrustV1` | `IRSFPT01` |
| `SignerFinalPromotionStateObservationBodyV1` | `IRSFPS01` |
| `SignerFinalPromotionStateObservationV1` | Signed wrapper around the marked body |

The observer signs the canonical body prefixed with
`iroha.sorafs.final-promotion.finalized-state.v1\0`; `\0` denotes one zero byte.
Shared operation types keep their existing protocol and receipt identities.

## Exact statement and signing transaction

The role payload is at most 256 KiB: the prefix
`iroha:sorafs:production-readiness:production-promotion-provenance:v1\0`
followed by canonical ASCII JSON with sorted object keys, compact separators,
canonical escaping, and integers only. Duplicate and unknown keys are rejected.
The closed 24-root-field schema is
`sorafs.production_readiness.production_promotion_provenance.v1`.
`ROOT_FIELDS` and the nested field sets in the parser are authoritative.
It binds chain, network, deployment, configured signer identity, the
22-input baseline, six ordered negative receipts, runner/checker/toolchain,
Python runtime, four positive output hashes, and cosign/OIDC provenance claims.
`authentication.signature_hex` is detached before constructing this preimage;
the remaining eight authentication fields are signed and checked against custody.
Backend and hardware/software qualification claims are absent from V1. Custody
authorization binds the public key, purpose, policy, authority, and finalized
enrollment state; it makes no claim about key origin or exportability.

`prepare_final_promotion_statement_v1` checks exact syntax and binding before
provider I/O and returns an opaque `PreparedFinalPromotionStatementV1`.
`SignerFinalPromotionRequestV1::new` requires that witness, independently reviewed
operation ID, statement size and domain-separated digest, and verified custody.
It commits the complete binding and original custody record/control-state identity.
`signer_final_promotion_digest_v1` owns the payload digest; it is distinct from
the SHA-256 used to pin the exact statement file in public evidence.

The daemon constructor pins immutable `Arc<[u8]>` statement bytes and independently
reviewed operation ID, digest and size. It rejects invalid syntax, binding or
coordinates before any custody-source or signer I/O. `sign()` and `recover()`
accept no replacement statement. Each signing attempt obtains fresh custody and
the current audit predecessor; recovery checks fresh custody and the exact original
completion without obtaining a new signing predecessor.

`recovery_service()` returns a `SignerFinalPromotionRecoveryServiceV1` sharing the
same receipt core, journal lease and nonblocking lifecycle gate. Its `recover()`
uses the canonical completed-receipt checks and has no signing method. Dropping
the signing service can release its protected receipt provider while this view
retains the pinned statement and journal. The generic constructor's observation
adapter still retains the supplied StateSource, which may own other drivers.
TODO: Assemble standalone native recovery with a concrete receipt-only source;
this view alone does not establish that every source-owned driver is absent or
that native submission, observer spending, UTC or floor providers are qualified.

One reservation produces exactly four ordered signatures:

1. `RolePayload`: the complete canonical statement bytes.
2. `AuditRecord`: the final-promotion audit binding request, intent, reservation,
   previous audit head, and statement signature.
3. `Provenance`: shared provenance binding custody, operation and audit.
4. `Response`: the final-promotion response digest binding the request,
   provenance and preceding three signatures.

The daemon observes fresh signing state for the intent's previous audit head;
the authority reserves by compare-and-swap (CAS), and admitted original custody
must still match. It durably stages the immutable complete receipt before the
completion CAS. It checks fresh completed state and rechecks the staged journal
before releasing bytes. Failure never releases a partial receipt. Recovery
validates the exact staged receipt against the original completed operation and
fresh custody/revocation state. It performs no new Reserve, Complete or protected
role14/role15 key operation. The native source must submit fresh observer-signed
receipt Checks under the separate deployment Check permission and an approved
observer fee budget. The original operator must retain Operate permission; current
role15 key custody is not an extra requirement for releasing the completed receipt.

## Independent verification authority

Candidate artifacts cannot select their own trust, expected subject, or time.
Independent policy pins the whole signer binding, operation ID, exact statement
SHA-256 and size, and a finalized anchor floor including the exact hash at equal
height. Independent trust pins the signer identity and state observer, their
keys, identities, revisions, policy digests and validity windows. The public
signer key, policy bytes and trust bytes have independently supplied SHA-256 pins.

The observer key and both observer identities must be independent of signer and
attester. Its signed observation binds the reviewed policy and statement,
chain/network/deployment, current finalized anchor and ACTIVE custody head,
revocations, and the full original completed operation. Observation freshness is
bounded by independently pinned trust and at most five minutes. The verifier
checks current active signer custody and the exact original completion's
intent, custody, reservation/fence, four-signature digest, commitment, time and
finalized anchor. An observer signature is accountable finalized-state testimony;
the observer remains responsible for authenticating consensus finality.

## Offline command and scoped result

The [CLI owner](../../crates/iroha_cli/src/commands/sorafs/toolkit/validation/final_promotion_receipt.rs)
implements `iroha app sorafs toolkit final-promotion-receipt`, with 11 required flags:

```text
--statement --signature --public-key --public-key-fingerprint
--signer-policy --signer-policy-sha256 --custody-trust --custody-trust-sha256
--completed-operation-state --operation-receipt --now-unix-ms
```

`--format=json` is optional and is the only format. Signature and public-key
files contain exactly 64 and 32 raw bytes. Time is a canonical positive Unix
millisecond integer. Success emits one JSON object with exactly these 25 fields:

```text
schema status verification_scope statement_sha256 statement_size
signature_sha256 public_key_fingerprint_sha256 signer_policy_sha256
custody_trust_sha256 completed_operation_state_sha256 operation_receipt_sha256
operation_id custody_record_digest policy_digest key_revision policy_revision
service_id administrator_id role deployment_id chain_id network_id
finalized_height finalized_block_hash verified_at_unix_ms
```

Fixed values are `schema=sorafs.final_promotion_receipt_verification.v1`,
`status=verified`, `verification_scope=final_promotion_signer_receipt`,
and `role=final_promotion_provenance`. Finalized height/hash
identify the original completion anchor. Binary identifiers/digests use lowercase
hex; `custody_record_digest` and `policy_digest` retain their defining digest
contracts. No field claims promotion eligibility.

The [Python adapter](../../scripts/sorafs_final_promotion_evidence.py) snapshots
the independently pinned executable, stages exact private inputs, requires
successful bounded execution without stderr, and validates every result field.
Its shared process helper controls the child process group, not escaped sessions.
The [final conjunction](../../scripts/check_sorafs_production_promotion_bundle.py)
also checks archive/replay, statement freshness and provenance bindings described
in the [negative archive specification](negative_promotion_archive.md).

## Exact cosign subject and local verification

`promotion_cosign_subject_bytes` in the final checker constructs one initial
25-root-field subject: the unsigned statement above with `cosign_bundle_sha256`
absent, under `iroha:sorafs:production-readiness:production-promotion-cosign-subject:v1\0`.
It uses the same canonical ASCII JSON codec and binds every other root field and
all nine authentication fields. The final signer statement subsequently binds
the captured cosign bundle hash. No placeholder bundle hash or circular signature
is part of either wire contract.

The [cosign adapter](../../scripts/sorafs_final_promotion_cosign.py) requires
independently pinned executable and trusted-root paths plus both SHA-256 values.
It stages the exact once-captured bundle, subject and bounded reviewed root;
certificate identity and OIDC issuer are independent canonical public HTTPS URLs.
Only `application/vnd.dev.sigstore.bundle.v0.3+json` is admitted: exactly
`mediaType`, `verificationMaterial`, and `messageSignature` at the root; one
P256 leaf certificate; one Rekor 2 `hashedrekord/0.0.2` inclusion-proof entry; and
one RFC3161 signed timestamp. The message digest is canonical base64 SHA2-256.
DSSE, managed-key, certificate-chain and integrated-time-only forms are rejected.
Cosign owns certificate, signature, artifact-digest, transparency and timestamp
cryptography. Its invocation is exactly `verify-blob --bundle <bundle>
--trusted-root <root> --certificate-identity <identity> --certificate-oidc-issuer
<issuer> --use-signed-timestamps <subject>`. Success requires exit zero, empty
stdout and exactly `Verified OK\n` on stderr through the bounded private process helper.

Redundant proof fields must agree with the values cosign consumes: canonical
nonnegative int64 indices must match and be below the checkpoint tree size;
proof size/root must equal the signed checkpoint's first three lines; at most
63 canonical 32-byte proof hashes are accepted. The checkpoint uses a canonical
hostname origin. `canonicalizedBody` must equal the exact compact sorted ASCII
Rekor 2 body reconstructed from the outer digest, signature and certificate.
The pinned `cryptography` DER parser verifies the leaf's actual P256 public-key
algorithm; malformed versions and unsupported algorithms fail without candidate
diagnostics. These are consistency checks, not a replacement signature or Merkle verifier.

The audited cosign v3.1.3 [trust selection](https://github.com/sigstore/cosign/blob/v3.1.3/cmd/cosign/cli/verify/common.go#L174-L200)
uses the supplied local root before default/TUF discovery; its
[new-bundle verifier](https://github.com/sigstore/cosign/blob/v3.1.3/pkg/cosign/verify_bundle.go)
uses the supplied bundle, artifact and trusted material. Independent executable
pins qualify this exact contract; a candidate cannot select another tool or trust.
The [Rekor 2 proof consumer](https://github.com/sigstore/rekor-tiles/blob/v2.3.0/pkg/verify/verify.go)
uses the authenticated checkpoint size/hash and reconstructed leaf, so the adapter
also closes otherwise unused duplicated metadata before invoking it.
The independent Rust statement golden and projection tests check the SoraFS subject
construction. The local pinned cosign v3.1.3 suite passes all 30 actual crypto/fixture
checks, including the positive and altered subject, identity, issuer, certificate,
signature, transparency, timestamp, independent trust and duplicate-field negatives.
The [public fixture](../../fixtures/sorafs/final_promotion_cosign/sources.json) and
[crypto tests](../../scripts/tests/sorafs_final_promotion_cosign_crypto_test.py)
establish this local verification boundary; they do not qualify a SoraFS producer.

## Remaining production work

TODO: Wire the configured software signer provider, authoritative finalized custody and
completed-operation adapters, and a production signing command. The generic
daemon service and offline verification command do not supply those integrations.
Tests use software signatures and local fixtures; they do not establish deployed
finality or production readiness.

The final checker remains blocked until foundational, topology, resilience and
lane-inventory contracts each verify their own signer custody and completion.
An outer signature cannot supply their missing authority verification or
supply 17 ready lanes or production soak.

TODO: Connect and qualify the actual production cosign signing workflow over the
exact SoraFS subject. Public upstream crypto fixtures and mocked checker boundaries
do not supply production signing evidence or a ready deployment. Actual cosign
verification is required in addition to all four inner custody proofs.
