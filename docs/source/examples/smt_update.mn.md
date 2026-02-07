---
lang: mn
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 788902cfafc6c7db6d52d4237b46ffe78193efd57852bc3427a16d7f3cda2f9c
source_last_modified: "2025-12-29T18:16:35.954370+00:00"
translation_last_reviewed: 2026-02-07
---

# Sparse Merkle Update Example

This worked example illustrates how the FASTPQ Stage 2 trace encodes a
non-membership witness using the `neighbour_leaf` column. The sparse Merkle tree
is binary over Poseidon2 field elements. Keys are converted to canonical
32-byte little-endian strings, hashed to a field element, and the most
significant bits select the branch at each level.

## Scenario

- Pre-state leaves
  - `asset::alice::rose` -> hashed key `0x12b7...` with value `0x0000_0000_0000_0005`.
  - `asset::bob::rose`   -> hashed key `0x1321...` with value `0x0000_0000_0000_0003`.
- Update request: insert `asset::carol::rose` with value 2.
- The canonical key hash for Carol expands to the 5-bit prefix `0b01011`. The
  existing neighbours have prefixes `0b01010` (Alice) and `0b01101` (Bob).

Because there is no leaf whose prefix matches `0b01011`, the prover must provide
additional evidence that the interval `(alice, bob)` is empty. Stage 2 populates
the trace row across the columns `path_bit_{level}`, `sibling_{level}`,
`node_in_{level}`, and `node_out_{level}` (with `level` in `[0, 31]`). All values
are Poseidon2 field elements encoded in little-endian form:

| level | `path_bit_level` | `sibling_level`             | `node_in_level`                      | `node_out_level`                     | Notes |
| ----- | ---------------- | --------------------------- | ------------------------------------ | ------------------------------------ | ----- |
| 0 | 1             | `0x241f...` (Alice leaf hash) | `0x0000...`                          | `0x4b12...` (`value_2 = 2`)          | Insert: start from zero, store new value. |
| 1 | 1             | `0x7d45...` (empty right)     | Poseidon2(`node_out_0`, `sibling_0`) | Poseidon2(`sibling_1`, `node_out_1`) | Follow prefix bit 1. |
| 2 | 0             | `0x03ae...` (Bob branch)      | Poseidon2(`node_out_1`, `sibling_1`) | Poseidon2(`node_in_2`, `sibling_2`)  | Branch flips because bit = 0. |
| 3 | 1             | `0x9bc4...`                   | Poseidon2(`node_out_2`, `sibling_2`) | Poseidon2(`sibling_3`, `node_out_3`) | Higher levels continue hashing upward. |
| 4 | 0             | `0xe112...`                   | Poseidon2(`node_out_3`, `sibling_3`) | Poseidon2(`node_in_4`, `sibling_4`)  | Root level; result is the post-state root. |

The `neighbour_leaf` column for this row is populated with Bob's leaf
(`key = 0x1321...`, `value = 3`, `hash = Poseidon2(key, value) = 0x03ae...`). When
verifying, the AIR checks that:

1. The supplied neighbour corresponds to the sibling used at level 2.
2. The neighbour key is lexicographically greater than the inserted key and the
   left sibling (Alice) is lexicographically smaller.
3. Replacing the inserted leaf with the neighbour reproduces the pre-state root.

Together these checks prove that no leaf existed for the interval `(0b01010,
0b01101)` before the update. Implementations generating FASTPQ traces can use
this layout verbatim; the numerical constants above are illustrative. For a full
JSON witness, emit the columns exactly as they appear in the table above (with
numeric suffixes per level), using little-endian byte strings serialized with
Norito JSON helpers.
