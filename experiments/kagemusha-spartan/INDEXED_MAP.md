# Exact indexed-map insertion and search

`src/indexed_map.rs` supplies insertion and search components for future monetary
relations. It is not yet connected to Send or Receive and does not authorize
a payment independently.

The component uses the same native Poseidon framing and domains as
`crates/iroha_plonk_gadgets/src/imt.rs`: `kgwimlf1` hashes a leaf's key, value
and next key; `kgwimnd1` hashes an ordered pair of children. An empty slot is
the raw field zero. The zero-key, zero-value sentinel belongs only at index
zero. Production depth is 32; smaller depths exercise the same constraints
in adversarial tests.

`append_insert` returns cells for the old root, inserted key and value, and
new root. Its caller must equate those cells with the authenticated predecessor,
operation effect and successor. A locally consistent arbitrary tree is not an
authenticated wallet state.

`append_search` returns the authenticated root, query, opened leaf value and
a constrained `present` bit. The result is zero only for a strict authenticated
gap and one only for the queried leaf. An unrelated valid leaf or a malformed
path cannot produce absence. The opened value is the queried value only when
`present` is one; an absent query exposes its bracketing leaf's value.

The relation constrains:

- Canonical field-integer ordering, using bounded byte decompositions and
  explicit subtraction borrows. A field difference or low-limb comparison
  cannot substitute for full integer order.
- Nonzero inserted key and value; sentinel key/value/index equivalence;
  and a strictly increasing next key, or zero at the end of the list.
- The authenticated low leaf and its unique strict gap: `low < key` and
  either `key < next` or `next = 0`. Existing keys have no insertion witness.
- Every direction bit and path index. Depth 32 includes all `u32` indices.
- Relinking the low leaf first, then authenticating the destination as empty
  against that intermediate root, and finally inserting the new leaf.

Hash inputs, outputs, selectors and equality ports are circuit constraints.
Host computations construct witnesses; invalid witnesses retain the same
relation and fail it. The caller supplies the shared byte range table.

Ten focused tests passed on 2026-11-12:

- The full depth-32 insertion matches the existing G1 fixture's sentinel,
  empty root, intermediate root and final root, and satisfies the relation.
- A depth-3 proof verifies; changing any of its four public join ports fails
  verification, and changing those private cells fails the relation.
- Duplicate and zero keys, zero values, wrong roots and paths, occupied
  destinations, invalid indices and malformed leaf ordering are rejected.
- Values with differing high halves and the canonical field maximum cannot
  wrap into the authenticated insertion gap.
- Malformed sentinel/value/order cases still fail after their old trees and
  intermediate paths are coherently rehashed, isolating the shape constraints.
- Insertion at `u32::MAX` succeeds at depth 32; changing direction bit 31 fails.
- Different valid keys and destination slots use identical complete matrices.
- Search proofs for membership and absence verify under identical matrices;
  changing the returned membership bit fails proof verification.
- Unrelated but authenticated leaves, zero queries, malformed paths and
  coherently rehashed malformed leaves cannot supply a search verdict.
- Empty-sentinel and terminal-leaf gaps both return authenticated absence.

Run from this experiment with its existing target directory:

```sh
ZKS_SCHEDULE=lean RAYON_NUM_THREADS=4 cargo test --locked --offline --release \
  indexed_map::tests -- --test-threads=2
```

Conditional insertion, record-preserving updates, removal and composition of
the authenticated search result into Receive's complete burn predicate remain
open. This implementation preserves a valid
authenticated map; it does not establish global sortedness of an arbitrary
starting tree. Production codecs, wallets and ledger state are unchanged.
