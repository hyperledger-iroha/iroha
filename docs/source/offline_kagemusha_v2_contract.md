# Kagemusha wire and proof contract

Kagemusha is the only offline-cash protocol. Suffixes on Norito types, proof
envelopes, and artifact manifests are internal schema versions; they do not
select another lifecycle.

## Exact amounts

`KagemushaScaledAmountV2` contains positive atomic `u128` units and the
authoritative on-chain asset scale. Conversion from `Numeric` is exact and
rejects excess precision, overflow, zero, negative values, and unsupported
scale. The same scaled value is bound into the top-up debit, every recursive
transition, and the redemption credit.

A transition consumes one or two strictly ordered parent bundles and creates
one recipient branch plus optional sender change. Checked arithmetic enforces:

```text
sum(parent atomic units) = recipient atomic units + change atomic units
```

All non-zero output commitments and nullifiers are distinct. Recipient and
change are independently spendable and redeemable. Parent replay,
ancestor/descendant overlap, conflicting transition choices, duplicate
nullifiers, duplicate commitments, duplicate delivery, and duplicate
redemption fail closed.

## Top-up

`KagemushaRecursiveSpendTopUpRequestV2` binds the signed device authorization,
exact amount, authoritative confidential-tree witness, verifier identity,
top-up shield proof, and stable operation id. Torii accepts the typed request
only at `POST /v1/offline/top-up` with parameter-free
`Content-Type: application/x-norito`.

The chain verifies the live asset scale, device authorization, current tree
root and leaf index, active verifier window, proof public inputs, and operation
uniqueness before debiting the public balance and committing the finalized
anchor. Recursive initialization consumes that finalized anchor and its typed
finality proof; caller-created roots or placeholder finality are invalid.

## Peer transfer

The receiver first signs a short-lived `KagemushaRecipientPaymentRequestV2`
that binds chain, asset, exact amount, recipient output, device identity,
request nonce, and expiry. The sender verifies it before reserving inputs or
proving.

Append binds the ordered parent bundle digests, receiver request digest,
recipient output, optional change output, exact split, operation id, artifact
manifest, and recursive transition. It returns a
`KagemushaRecursiveSpendSplitResultV2`; the peer envelope carries only the
recipient branch. Each branch carries a depth-bounded path and the exact
proof-bound transition history needed to reject overlapping ancestry without
growing with unrelated wallet state.

The receiver verifies the signed request, artifact generation, verifier
activation, scale, amount, recipient commitment, hop limit, recursive proof,
and redeemability. It persists the branch atomically before signing the durable
acknowledgement. The acknowledgement binds the operation, request, accepted
bundle digest, recipient commitment, device key reference, and one captured
acceptance time. Duplicate delivery returns the same bytes. The sender commits
its reserved inputs and change only after verifying that acknowledgement.

## Redemption

`KagemushaRecursiveSpendRedeemRequestV2` binds the selected live branch,
unshield-v3 evidence, signed device authorization, operation id, and optional
proof-bound change branch. Torii accepts it only at
`POST /v1/offline/redeem` with the typed Norito media type. Full redemption has
no change; partial redemption produces exactly one recursively spendable
change branch. Chain execution verifies finalized top-up provenance, recursive
proofs, active unshield and recursive verifier windows, branch conflicts,
value conservation, and operation uniqueness before crediting the exact
scaled public amount.

## Artifacts and native bridge

Bridge ABI 19 exposes one Kagemusha capability record. It must report manifest
schema `kagemusha.offline.recursive_spend.artifact_manifest.v3`, backend
`halo2/ipa-pasta-cycle-v1`, the fixed transition/state circuit identifiers,
and an exact proof-backend availability flag.

The authenticated V3 manifest contains exactly two role profiles—transition
Eq and state Ep—and exactly three content-addressed files per profile:
parameters, proving key, and verifying key. It binds source revision, chain,
asset and scale, activation window, proof-size limit, benchmark evidence,
cryptographic review, release attestation, and top-up-finality roster. A wallet
must verify the release envelope, manifest digest, every file size and SHA-256,
role, circuit, ABI, and purpose before atomically installing the set.

Wallets must require `proof_backend_available == true`, an empty native
missing-gate list, and a readiness response with all five active verifier
records before enabling Kagemusha. Symbol presence or locally well-formed
artifacts are not readiness evidence.

## Public Torii surface

The complete first-release route set is:

- `GET /v1/offline/readiness`
- `POST /v1/offline/top-up`
- `POST /v1/offline/redeem`
- `GET /v1/offline/operations/{operation_id}`

Readiness is a closed snapshot-bound object. It carries bridge ABI 19, maximum
hop count, canonical asset and scale, evaluated block height/hash, active
transfer, top-up-shield, unshield, recursive-transition, and recursive-state
verifier records, proof availability, recursive-lineage support, readiness,
and blockers. Each verifier role must have the exact backend/name/circuit and
must not share a registry id, key commitment, or public-input schema hash with
another role.

Top-up and redemption accept no JSON body or encoded-byte wrapper. A canonical
lowercase 64-hex `Idempotency-Key` equals the signed operation id. Identical
retries return the same operation; reuse with different bytes conflicts. The
wallet retains pending state until the operation-status route reports final
chain finality.
