# Kagemusha V2 top-up finality provenance

Kagemusha spendable cash must not trust an HTTP server's observation that a
top-up was applied. Every initial recursive note is instead accompanied by a
compact proof that the exact finalized anchor was a write authenticated by the
block's consensus commit certificate.

## Authenticated statement

For a top-up operation `operation_id`, execution records the dedicated witness
leaf

```text
key   = 0xD1 || operation_id
value = anchor_digest
leaf  = H(0x00 || H(key) || H(value))
```

Kagemusha leaves are removed from the ordinary write set and sorted by
`operation_id` into a canonical balanced block-local commitment tree. For a
block containing at least one Kagemusha top-up, execution commits

```text
post_state_root = H(
    "iroha:kagemusha:v2:post-state-root" || 0 ||
    u32(anchor_count) || ordinary_writes_root || topup_anchor_root
)
```

Blocks without a Kagemusha top-up retain the ordinary execution root. This
separation is consensus-critical: a proof carries one ordinary-root sibling
and `ceil(log2(anchor_count))` anchor siblings, so unrelated writes cannot make
an offline payment unbounded. The anchor binds the chain, payer, exact scaled
amount, confidential roots, note commitment/nullifier, active transfer
verifier, artifact generation, operation id, finalized height, and transaction
hash.

The proof package contains:

- every non-roster field of the live Sumeragi-v2 `HeightContext`, including
  chain id, epoch data, parent Commit QC, DA layout, leader seed, and the typed
  context id;
- the exact live Sumeragi-v2 Commit `QuorumCertificate`, including its subject,
  execution commitment, signer indexes, and aggregate signature; and
- the anchor leaf index/count and canonical balanced-Merkle siblings.

The roster is deliberately not repeated in each peer proof. It comes only from
the content-addressed release artifact. Native verification reconstructs the
complete `HeightContext` from that authenticated roster window, validates it,
and requires its recomputed identifier to equal the context id signed by the
QC. The QC execution commitment supplies the ordinary-write root, top-up root
and count, and consensus post-state root; the proof never carries a parallel
root that could disagree with the signed certificate.

The peer payload already carries the compact `(operation_id, anchor_digest)`
reference. The initial recursive proof binds that digest to the complete anchor
and first note, so repeating the full anchor in every later hop is unnecessary.

Anchor siblings are encoded from leaf to root. The leaf index must be in range,
the sibling count must equal the depth derived from the committed leaf count,
and odd levels use the protocol's fixed empty-node hash. Duplicate operation
ids, alternate leaf ordering, extra siblings, and non-canonical tree shapes are
rejected. Consensus enforces a maximum top-up count per block derived from the
peer-envelope proof budget.

## Trust and rotation

An attacker-supplied validator set is not a trust anchor. Offline setup
downloads the canonical V3 release manifest and its content-addressed consensus
roster artifact. Each non-overlapping roster window contains the exact ordered
`ValidatorPower` entries, consensus mode, aligned BLS proofs of possession, and
inclusive/exclusive activation bounds. The expected manifest SHA-256 must come
from the authenticated release envelope; the manifest then selects the exact
roster byte length and SHA-256.

Verification requires the proof height to fall inside exactly one manifest
release window and exactly one roster window. It reconstructs the full height
context, validates signer quorum and ordering, verifies every roster proof of
possession once per bounded exact-digest cache entry, and verifies the aggregate
over the live `Vote::signature_preimage`. Chain, asset definition, scale,
artifact generation, height, operation id, and complete anchor digest are bound
before pairing work begins. A device whose cached release or roster window has
expired cannot receive new offline cash until it refreshes while online.

## Lifecycle binding

The first-release Torii applied top-up result returns both the typed finalized
anchor and the typed `KagemushaTopUpFinalityProofV2`. It never reports an
applied top-up from a height and anchor alone. Before either the block log or
WSV advances, Kura durably stages the bounded witness-derived top-up tree and
paths. After the exact cryptographically verified `V2FinalityArtifact` is
durable, Kura promotes that stage to an immutable final sidecar bound to the
artifact hash. Publication uses no-clobber atomic files, directory sync, stable
file-identity checks, bounded decode limits, and idempotent exact retries;
conflicting or missing crash-recovery state fails closed. Kura then builds the
canonical proof for the exact `(height, operation_id)` while serving the
canonical operation-status resource.

There is no parallel finality-proof retrieval route and no base64 wrapper. If
the local durable artifact or sidecar cannot produce the exact proof, Torii
returns `503 offline_topup_finality_proof_unavailable`; the wallet retries the
same operation status URI and must not run `initSpend`. The local init proof
binds the verified anchor digest, and redemption resolves compact anchor
references from chain state before crediting value. The native verifier remains
fail-closed until the authenticated release-envelope trust root is available.

`KagemushaRecursiveSpendPeerPaymentV4` intentionally contains only the
recipient bundle and its proof-bound membership witness. A future finality
package must therefore remain a deduplicated wallet/transport sidecar keyed by
compact anchor reference; it must not be added to the canonical two-field
peer-payment wire.

Receiver verification is ordered fail-closed:

1. decode and re-encode every archive canonically;
2. require the authenticated manifest digest and its exact roster descriptor;
3. validate the complete anchor and require exact proof anchor/ref equality;
4. reconstruct the full height context from the selected roster window and
   verify the exact Commit vote aggregate;
5. reconstruct the domain-separated post-state root from the QC execution
   commitment and exact anchor inclusion path;
6. require one proof per top-up reference;
7. verify the recursive spend proof, recipient request, scale, amount, hop
   limit, verifier activation, and lineage acceptance; and
8. durably commit the received branch before acknowledging it.

No marker, note, nullifier, commitment, or acknowledgement may be written before
all checks through step 7 succeed.

## Durability and payload gate

The execution witness projection needed to build inclusion paths is persisted
before WSV commit. The final Kura sidecar is immutable and block-hash-bound,
stores the ordinary-write root plus canonical anchor leaves and paths, and is
promoted only against an exact durable finality receipt. Publication and reads
fail closed unless the reconstructed root equals the Commit QC's signed
`post_state_root`; a sidecar path swap cannot reuse a successful cryptographic
cache entry.

The C/Swift verification boundary accepts five logical inputs: the canonical
proof, canonical roster artifact, complete canonical top-up anchor, canonical
V3 manifest, and the manifest's expected non-zero SHA-256. Native ingress
applies separate proof, roster, anchor, and manifest byte caps before copying
or decoding. The public symbol currently returns the unavailable error before
decoding otherwise valid inputs: a content address is not a release trust root,
and recursive init does not yet consume a verified-finality capability. Both
gates must be implemented before this boundary can become callable.

Current size goldens cover the canonical peer-payment wire at depths 1, 2, 4,
8, and 64 with one or two branch claims. Provenance remains a deduplicated
sidecar and is not counted against that peer-payment wire. Any future Torii
transport must add separate response-size and retrieval gates without changing
the 32-KiB protocol archive cap. The 12-KiB text envelope is an independent
transport sub-cap and can carry at most 9,211 raw bytes with a six-byte prefix.
