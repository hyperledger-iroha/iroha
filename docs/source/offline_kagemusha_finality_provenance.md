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

- the canonical Commit-QC fields and aggregate signature for the anchor's
  block, omitting the validator roster already present in the trust artifact;
- the ordinary-write root plus the anchor leaf index/count and canonical
  balanced-Merkle siblings; and
- the validator-set hash and activation window selected from the cached trust
  artifact.

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

An attacker-supplied validator set is not a trust anchor.
Offline setup downloads a content-addressed consensus-roster artifact containing
the accepted validator-set hashes, public keys, proofs of possession, and
non-overlapping activation/withdrawal heights. Verification requires the proof
height to fall inside exactly one cached window and the QC's recomputed roster
hash to equal that window. A device whose cached roster window has expired
cannot receive new offline cash until it refreshes while online.

## Lifecycle binding

The current first-release Torii operation response returns the finalized top-up
anchor; it does not yet publish `KagemushaTopUpFinalityProofV2`. The proof types
and roster artifact define the future offline-verifiable provenance sidecar,
but sidecar construction, durable publication, and client retrieval remain
unwired. The local `initSpend` proof binds the anchor digest, and redemption
resolves compact anchor references from chain state before crediting value.

`KagemushaRecursiveSpendPeerPaymentV2` intentionally contains only the
recipient bundle. A future finality package must therefore remain a
deduplicated wallet/transport sidecar keyed by compact anchor reference; it must
not be added to the canonical one-field peer-payment wire.

Receiver verification is ordered fail-closed:

1. decode and re-encode every archive canonically;
2. verify the cached roster window and Commit QC cryptographically;
3. verify chain id, height, block hash, and QC `post_state_root` bindings;
4. reconstruct the domain-separated post-state root from the ordinary-write
   root and exact anchor inclusion path;
5. require one proof per top-up reference and exact anchor/ref equality;
6. verify the recursive spend proof, recipient request, scale, amount, hop
   limit, verifier activation, and lineage acceptance; and
7. durably commit the received branch before acknowledging it.

No marker, note, nullifier, commitment, or acknowledgement may be written before
all checks through step 6 succeed.

## Durability and payload gate

The execution witness needed to build inclusion paths must survive restart
before this sidecar can ship. The intended Kura sidecar is immutable and
block-hash-bound, stores the ordinary-write root plus canonical anchor leaves
and paths, and is reproducible from the canonical block. Publication must fail
closed unless the recovered root equals the Commit QC's `post_state_root`.

Current size goldens cover the canonical peer-payment wire at depths 1, 2, 4,
8, and 64 with one or two branch claims. They do not include provenance
packages, roster bitmaps, or top-up-count proofs. A future sidecar transport
must add separate size and retrieval gates without changing the 9,211-byte raw
/ 12-KiB peer-payment limits.
