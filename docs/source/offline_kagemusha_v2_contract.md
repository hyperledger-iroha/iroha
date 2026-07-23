# Retained V2 primitives and the V4 lifecycle

Kagemusha is the only offline-cash protocol. ABI 21, manifest V4, and the V4
top-up, recursive, verification, and redemption carriers are the sole
lifecycle. The `V2` suffix survives only on unchanged leaf primitives that V4
intentionally embeds: scaled amounts, note descriptors and openings,
authorization, recipient requests and acknowledgements, membership paths, and
top-up finality proofs. A V2 or V3 lifecycle archive is never upgraded to V4.

## Exact amounts and notes

`KagemushaScaledAmountV2` contains positive atomic `u128` units and the
authoritative on-chain asset scale. Conversion from `Numeric` is exact and
rejects excess precision, overflow, zero, negative values, and unsupported
scale. `KagemushaSpendableNoteDescriptorV2`, note openings, and membership
witnesses keep their established commitments and tree semantics; V4 binds
their exact canonical bytes into its public operation vector.

Initialization has no recursive parent. Append consumes one or two
canonically ordered parent bundles and creates one recipient branch plus
optional sender change. The fixed EqAffine/Vesta transition and EpAffine/Pallas
wrapper prove:

```text
sum(parent atomic units) = recipient atomic units + change atomic units
```

The same key material and layout handles initialization, one parent, and two
parents. Parent-presence selectors, ordered state, commitments, nullifiers,
roots, branch claims, output membership, release identity, and current/result
instances are circuit constraints. Host-checked equality or an accumulated IPA
opening is never an acceptance decision.

## V4 top-up and initialization

`KagemushaRecursiveSpendTopUpRequestV4` binds the signed V2 authorization leaf,
exact amount, authoritative confidential-tree witness, shield proof, V4 release
identity, and stable operation id. Torii accepts the typed request only at
`POST /v1/offline/top-up` with parameter-free
`Content-Type: application/x-norito`.

Core verifies chain, asset, scale, authorization, replay state, authoritative
root and leaf index, active shield verifier, proof public inputs, and the
currently issuing V4 release before any balance, tree, escrow, or receipt
mutation. Local V4 initialization then authenticates the finalized anchor and
the unchanged V2 finality proof against the manifest-bound roster.

## V4 peer transfer

The receiver signs the unchanged `KagemushaRecipientPaymentRequestV2` leaf.
V4 append binds that request to one or two ordered V4 parents, recipient and
optional change outputs, output membership, exact conservation, operation id,
and the release-bound recursive verifier pair. The V4 peer envelope contains
only the recipient bundle and its secret-free membership witness.

The receiver verifies the V4 proof pair, signed recipient leaf, finality
origins, artifact generation, chain, asset, scale, amount, commitment, hop
limit, and branch disjointness before persisting the branch. The unchanged V2
acknowledgement leaf is signed only after durable receipt. Duplicate delivery
returns the same bytes, and the sender consumes reserved parents only after the
acknowledgement verifies.

## V4 redemption

`KagemushaRecursiveSpendRedeemRequestV4` binds the selected V4 branch,
unshield evidence, V2 authorization leaf, operation id, and optional
proof-bound V4 change branch. Full redemption has no offline change. Partial
redemption produces exactly one recursively spendable V4 change branch and is
therefore new issuance.

At and after `withdrawal_height`, top-up, init, append, peer payment, and
redemption with offline change are rejected. Terminal verification and full
redemption retain the parent release indefinitely unless it is explicitly
revoked, so a legitimate branch is not stranded when issuance closes.

## Artifacts and native bridge

Bridge ABI 21 exposes the current Kagemusha artifact contract. It reports
manifest schema `kagemusha.offline.recursive_spend.artifact_manifest.v4`,
backend `halo2/ipa-pasta-cycle-compact-v5`, transcript
`kagemusha-pasta-cycle-poseidon-compact-v5`, and the exact StepEq/StepEp circuit
identities. There is no product-mode selector.

The authenticated V4 manifest contains exactly two profiles in Eq-then-Ep
order. Each profile contains exactly four external, content-addressed files in
this order: `ParamsIPA`, processed proving key, processed verifying key, and
the final-key selector-zero bootstrap witness. The complete external inventory
is therefore exactly eight files. Each profile's bounded
`KagemushaStepCircuitParamsV4` configuration is authenticated inline in the
manifest and digest-bound into every artifact header; circuit parameters are
not a ninth or tenth streamed file. The manifest also binds source revision,
chain, asset and scale, activation window, proof-size limit, benchmark
evidence, cryptographic review, release attestation, and the canonical top-up
finality roster. A wallet must verify the release policy and attestation,
manifest digest, every framed and payload size and SHA-256, role, circuit, ABI,
and purpose before atomically installing the exact eight-file set.

The V4 cryptographic review is a canonical Norito signed envelope, not a text
report accepted by hash alone. Its signed payload binds the immutable candidate
and release identity, a nonzero retained-report digest, the exact eight artifact
roles, and the fixed ordered six-check matrix. Production requires an approved
decision, all checks passed with distinct nonzero evidence digests, canonical
ordered signatures from policy-authorized review keys, the configured review
threshold, and exact reviewer-set equality with the cryptographic-review role in
the release attestation. Native finalization, release-directory verification,
and consensus release-record admission all enforce the same data-model
validator.

Authenticated artifact installation and backend construction are necessary
but not by themselves sufficient for complete node readiness. Torii carries a
required nullable `artifact_set`: it is present only with the atomic V4
recursive verifier pair and contains the generation, manifest, policy and
attestation digests, issuance window, proof-pair bound, and asset scale. A null
value requires both recursive verifier records and backend construction to be
unavailable together with exactly one `recursive_v4_registry_unavailable` or
`recursive_v4_registry_malformed` blocker. A present value forbids both
registry blockers. `proof_backend_available` reports exact backend
construction independently. `recursive_lineage_supported` is true only when
that artifact set, distinct active Eq/Ep records, and the backend are all
present. `ready` is true exactly when the complete blocker set is empty, and
`recursive_lineage_unavailable` is present exactly when lineage is false.

Transaction admission authenticates the exact release binding against both
consensus records and the immutable startup catalog. Top-up and redemption
change require a currently issuing release. Full redemption authenticates its
parent release for the longer redemption lifetime and remains valid after that
release's issuance window closes.

## Public Torii surface

The complete first-release route set is:

- `GET /v1/offline/readiness`
- `POST /v1/offline/top-up`
- `POST /v1/offline/redeem`
- `GET /v1/offline/operations/{operation_id}`

Readiness is a closed snapshot-bound object. It carries exact bridge ABI 21,
maximum hop count, canonical asset and scale, evaluated block height/hash,
active transfer, top-up-shield, unshield, recursive StepEq and recursive StepEp
verifier records, the required nullable authenticated `artifact_set`, backend
construction state, recursive-lineage support, readiness, and blockers. The V4
recursive roles are exactly
`kagemusha_recursive_step_eq_v4_verifier_record` with circuit
`kagemusha-recursive-spend-step-eq-compact-layout-v5` and
`kagemusha_recursive_step_ep_v4_verifier_record` with circuit
`kagemusha-recursive-spend-step-ep-compact-lineage-v5`. Both use registry
backend `halo2/ipa`, appear or disappear atomically with `artifact_set`, and
bind the same activation window and proof-size limit as that artifact set. No
verifier role may share a registry id, key commitment, or public-input schema
hash with another role. Missing or malformed release material, Eq/Ep records,
or backend construction emits typed blockers and keeps admission fail-closed.

Top-up and redemption accept no JSON body or encoded-byte wrapper. A canonical
lowercase 64-hex `Idempotency-Key` equals the signed operation id. Identical
retries return the same operation; reuse with different bytes conflicts. The
wallet retains pending state until the operation-status route reports final
chain finality.
