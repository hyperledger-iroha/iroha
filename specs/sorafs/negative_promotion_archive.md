---
title: SoraFS Negative-Promotion Archive
summary: >-
  Payload-free qualification receipts for the fixed six-case promotion
  rejection matrix.
---

# SoraFS negative-promotion archive

After the exact reviewed promotion input set passes
`scripts/run_sorafs_production_readiness.py`, run the negative archive beside
the positive result:

```text
python3 scripts/run_sorafs_production_readiness_negative_archive.py \
  @/runtime/evidence/sorafs-negative-promotion-archive.args
```

Start from
`scripts/examples/sorafs_production_readiness_negative_archive.args.example`.
`--promotion-args-file` names the same reviewed response file used by the
positive runner. It must provide the signed topology summary/envelope tuple,
externally signed resilience qualification, signed L1 lane-evidence inventory
and its explicit external-software Ed25519 trust tuple, externally signed
foundational envelope, exact 17 ready lane summaries, explicit clock, freshness
bound, production deployment context, the separately reviewed Ed25519 public
keys, release sequence, and predecessor digest. It must not request `--dry-run`.
The output configured
inside that positive response file is not used by this runner.

The runner snapshots the bounded top-level `scripts/*.py` tool inventory,
installs those exact bytes read-only in a private temporary directory, and
binds the inventory with a domain-separated digest. Direct child invocations
use that snapshot and the recorded Python executable with `-I -B`; nested
positive-runner verifier invocations inherit a sanitized environment and use
the same snapshot checker path. The runner then copies the topology summary and
envelope, resilience summary, signed inventory, foundational envelope, and 17
lane summaries into the private directory. These are the ordered 22 top-level
baseline inputs. It invokes the positive runner
over those copies and requires both aggregate executions to be byte-identical,
`status=ready`, and 17/17 with every row valid. It creates a separate isolated
copy for each closed mutation:

1. re-encode the `ai_prescreen` summary as different, semantically equivalent
   bounded JSON, proving that the signed envelope binds the exact
   lane-summary bytes;
2. advance only the explicit checker clock beyond the accepted freshness
   window;
3. omit the `ai_prescreen` summary;
4. supply a second distinct copy of the `ai_prescreen` summary;
5. substitute the operator-reviewed predecessor expectation; and
6. alter one nibble of the foundational Ed25519 signature.

Every negative invokes the bundled aggregate checker and must return exit code
1, emit a schema-valid `status=blocked` aggregate, and produce exactly its
expected diagnostic class. An exit-code-2 preflight failure, an unexpected
diagnostic, an accepted mutation, a changing source input or tool inventory, a
changing Python executable, or a failed positive replay aborts publication.
The runner never mutates the reviewed inputs.

The new `--archive-out-dir` parent must already exist, be owned by the current
user, and not be group- or world-writable; the destination must not exist.
The runner captures the parent identity before qualification, then opens and
revalidates that directory before publication. Publication is an exclusive
atomic directory rename relative to the opened directory. This publication
path requires Darwin `renameatx_np` or libc `renameat2`; other platforms fail
closed. Run this qualification archive on a controlled Darwin or Linux host.
The archive contains exactly six numbered receipt files and
`negative-promotion-archive.json`. Receipts contain only:

- the fixed mutation ID;
- the domain-separated digest of the ordered 22-input baseline set: topology
  summary and envelope, resilience qualification, signed lane inventory,
  foundational envelope, and 17 lane summaries;
- the bundled checker and complete child-toolchain digests;
- the expected rejection and observed diagnostic class; and
- SHA-256 hashes of the blocked aggregate, its canonical semantics, stdout,
  and stderr.

The archive manifest binds the bundled runner, checker, complete child
toolchain, Python executable hash and public runtime version, positive
aggregate/replay/manifest output hashes, ordered six-case inventory, and each
exact receipt digest. It contains no topology, resilience, envelope, lane
summary, signature, diagnostic text, private evidence, credential, or payload.
The temporary positive and mutated copies are deleted after the run.

The manifest deliberately emits `status=locally-qualified`,
`attestation_scope=local-execution-receipt`,
`externally_authenticated=false`, and `promotion_eligible=false`. These files
are unsigned local execution receipts, not standalone proof. The final checker
requires `signing_provider=authenticated_external_signer` provenance,
authenticated by the purpose-specific
[final-promotion receipt V1](final_promotion_receipt_v1.md). The signed statement
binds cosign/OIDC provenance and the exact SHA-256 of
`negative-promotion-archive.json`, the archive inventory, this negative-archive
runner, and the Python runtime environment.
Hashing the executable alone does not bind its dynamic libraries or operating
system. Authenticated software providers and optional hardware providers share
the same authorization contract; an HSM is not required. V1 carries no backend
or hardware/software qualification claims and does not attest key origin or
exportability.

The fail-closed final conjunction is
`scripts/check_sorafs_production_promotion_bundle.py`. It is read-only and
accepts the two aggregate files, their deterministic replay manifest, the
negative-archive directory, one externally signed promotion-provenance statement,
the exact cosign JSON bundle named by that statement, an explicit clock, and
independent signer and deployment trust. Its required signer evidence inputs
are five paths (native verifier, signer policy, custody trust, signed completed
state observation, and operation receipt), three independently reviewed SHA-256
pins (verifier, policy, and trust), and chain/network/deployment identifiers.
It reuses the positive runner's aggregate/replay
validators and this runner's manifest/receipt validators. It also reopens all
six exact receipt files, rejects extra archive members, and requires the
positive replay and negative baseline to share the same ordered 22-input digest
and the same aggregate, replay, and replay-manifest hashes.

The external statement uses the sole 24-root-field schema
`sorafs.production_readiness.production_promotion_provenance.v1`, closed over
chain/network/deployment context, `status=verified`,
`attestation_scope=production-promotion-bundle`,
`signing_provider=authenticated_external_signer`, a fresh explicit timestamp,
the exact negative-manifest SHA-256, the six full ordered manifest receipt
rows, baseline input count and digest, runner/checker/toolchain hashes, the
closed Python-runtime object, and the four positive hashes (both aggregates,
their canonical semantics, and the replay manifest). It also binds the exact
cosign-bundle SHA-256, a canonical public HTTPS certificate identity and OIDC
issuer, and exact `verified` OIDC and cosign statuses. The external
signer authenticates these claims. The checker also verifies the exact non-circular
cosign subject through an independently pinned local verifier and trusted root;
signed status or identity text alone cannot satisfy the cryptographic boundary.

Its `authentication` object has a closed external Ed25519 shape:
`kind`, `algorithm`, distinct
`service_id` and `administrator_id`, positive key and policy revisions,
non-zero policy SHA-256, public-key fingerprint, and signature. Every value is
matched to independent command-line trust. The signature covers the ASCII
canonical JSON object with only `authentication.signature_hex` removed,
prefixed by the domain
`iroha:sorafs:production-readiness:production-promotion-provenance:v1\0`.
Unknown fields, including backend or qualification claims, stale or future
provenance, ineligible signer authorization, failed cosign/OIDC status, receipt
reordering, digest substitution, or a bad signature blocks promotion. The native
verifier must independently authenticate the role-14 receipt, signer
authorization, current finalized custody and exact original completed operation.

The Python adapter snapshots the pinned verifier executable, supplies exact
private inputs to `iroha app sorafs toolkit final-promotion-receipt`, and checks
its complete 25-field `final_promotion_signer_receipt` result. The statement
signature alone, caller-supplied verifier JSON, a foundational-purpose receipt,
or a changed policy/trust/context cannot satisfy that boundary.

TODO: Integrate independently verified signer custody and completed-operation
proofs for all four inner contracts: foundational, topology, resilience and
lane inventory. Their current aggregate/replay schemas still carry signatures
and digest summaries. `validate_inner_approval_chain`
therefore blocks production unconditionally. A valid outer receipt cannot
upgrade those contracts, create the 17 lane qualifications or establish production
soak. Configured signer/state authority and
the production signing command also remain open as recorded in the receipt spec.

The [cosign contract](final_promotion_receipt_v1.md#exact-cosign-subject-and-local-verification)
uses the same unsigned body with only the bundle hash absent, under its own
subject domain. The final signed statement binds the resulting bundle hash.
One canonical Sigstore v0.3 leaf-certificate/message-signature profile requires
Rekor 2 inclusion proof and signed RFC3161 timestamps; old formats, DSSE and key
profiles are rejected. The exact captured bytes go to pinned cosign v3.1.3 with
local trust, independent identity/issuer and mandatory signed-timestamp verification.
Its real public upstream crypto tests establish local cryptographic delegation.
TODO: Qualify the production signing workflow and actual signed SoraFS subject;
these public fixtures do not create deployment, custody or completed-operation evidence.

The complete final-checker invocation now includes the signer evidence and
context arguments below. This is an interface example; it currently returns a
blocked result even when the outer receipt verifies:

```text
python3 scripts/check_sorafs_production_promotion_bundle.py \
  --first-aggregate /runtime/evidence/aggregate.json \
  --second-aggregate /runtime/evidence/sorafs-production-readiness-replay-summary.json \
  --replay-manifest /runtime/evidence/sorafs-production-readiness-replay-manifest.json \
  --negative-archive-dir /runtime/evidence/sorafs-negative-promotion-archive \
  --promotion-provenance /runtime/evidence/production-promotion-provenance.json \
  --cosign-bundle /runtime/evidence/production-promotion.sigstore.json \
  --provenance-verification-public-key-hex <REVIEWED-RAW-ED25519-PUBLIC-KEY> \
  --provenance-signer-service-id <REVIEWED-SERVICE-ID> \
  --provenance-signer-administrator-id <REVIEWED-INDEPENDENT-ADMIN-ID> \
  --provenance-signer-key-revision <POSITIVE-REVISION> \
  --provenance-signer-policy-revision <POSITIVE-REVISION> \
  --provenance-signer-policy-digest-hex <NONZERO-SHA256> \
  --provenance-certificate-identity <REVIEWED-PUBLIC-HTTPS-IDENTITY> \
  --provenance-oidc-issuer <REVIEWED-PUBLIC-HTTPS-ISSUER> \
  --provenance-receipt-verifier /runtime/tools/iroha \
  --provenance-signer-policy /runtime/evidence/final-promotion-policy.norito \
  --provenance-custody-trust /runtime/evidence/final-promotion-trust.norito \
  --provenance-completed-operation-state /runtime/evidence/final-promotion-state.norito \
  --provenance-operation-receipt /runtime/evidence/final-promotion-receipt.norito \
  --provenance-receipt-verifier-sha256 <REVIEWED-VERIFIER-SHA256> \
  --provenance-signer-policy-sha256 <REVIEWED-POLICY-SHA256> \
  --provenance-custody-trust-sha256 <REVIEWED-TRUST-SHA256> \
  --provenance-cosign-verifier /runtime/tools/cosign \
  --provenance-cosign-verifier-sha256 <REVIEWED-COSIGN-SHA256> \
  --provenance-cosign-trusted-root /runtime/evidence/sigstore-trusted-root.json \
  --provenance-cosign-trusted-root-sha256 <REVIEWED-SIGSTORE-TRUST-SHA256> \
  --provenance-chain-id <REVIEWED-CHAIN-ID> \
  --provenance-network-id-hex <REVIEWED-NETWORK-ID-HEX> \
  --provenance-deployment-id <REVIEWED-DEPLOYMENT-ID> \
  --now-unix <REVIEWED-UTC-SECONDS>
```

The current inner-contract block yields exit code 1 and a schema-closed summary
with `status=blocked`, `externally_authenticated=false`,
`promotion_eligible=false`. Missing required
arguments or another preflight failure returns exit code 2. No current outer
receipt or locally qualified negative archive can produce promotion eligibility.

The receipts attest that the fixed rejection paths were exercised against one
already-ready baseline only within that enclosing externally authenticated
provenance. They do not create lane evidence, replace purpose-specific custody
verification, or authorize Taira or Minamoto cutover.

Collection policy must also require process exit code 0. If publication reports
failure, quarantine any newly visible destination before retrying; an archive
left visible after a directory-sync failure is not accepted.
