# RAM-LFE byte-layout capacity experiment

Date: 2026-09-29. Status: internal test-only primitive, 5/5 native controls pass.
The complete execution relation remains unavailable. No public API, relation
identifier, proof admission or resource limit changed.

## Exact operations and negative controls

`zk/ram_lfe_byte.rs` uses six advice columns, only current-row queries and two
lookup arguments sharing one finite table. A byte XOR is two nibble XOR lookups;
its high nibbles and derived low nibbles are exact table integers. Addition uses
copied, range-proven input bytes and an exact radix-256 carry equation. A table
bounds each new output byte and carry to 0..2. The first carry is zero and subsequent
carries are connected by equality constraints.

Nonbyte rotations use two tables: `(input, low, high)` and
`(high, output, next_low)`. Every component is an integer by membership, and cyclic
copies bind `next_low` to the next byte's `low`. A coordinated inverse-two split
fails lookup constraints without a public-output or downstream range check.
Whole-byte rotations copy the existing byte cells. The general table has 4,865
rows and supports all seven residual rotations. The hash-control subset includes
bits 1, 4, 7 and has 2,817 rows; 1 is retained for the fractional attack, while 4/7
cover BLAKE3/Blake2b. Other rotations are explicitly rejected for this subset.
These configuration choices are internal experiments, not developer options.

Controls cover all bit rotations, 32-/64-bit overflow, direct assignment changes,
carry-chain and terminal-carry substitutions, fractional inputs/high nibbles,
reserved words, source-cell substitution, incorrect public output and actual
owned-word clearing on success/error/unwind. The clearing scope excludes Halo2
private buffers and compiler copies. Native proofs reject altered public inputs,
changed proof bytes and trailing bytes.

## Immutable evidence

Ordinary offline Cargo compiled copied exact Core byte/test/Halo2 adapter sources
with the actual vendored backend, in an isolated optimized test harness. Source
before/after guards found no drift. This is primitive qualification, not a normal
Core package build; that build remains blocked by unrelated incoming Sumeragi
migration errors recorded by the parent task.

Artifacts are retained under
`dist/zk-remediation/2026-09-29/ram-lfe-circuit/`:

- `byte-20260929T055224Z`: build passed; 4/5 controls passed. One negative fixture
  set the low byte of 3 × 255 to its existing value 253. The failure was retained;
  the test now uses 254 and explicitly asserts that this changes the witness.
  All four native proof/degree comparisons and the fractional rotation control
  passed on that predecessor. No constraint repair was required.
- `byte-20260929T055541Z`: 5/5 pass, 0 ignored, no source drift. Binary SHA-256:
  `946b709771ab6686458d357663630ca885da00fd4a7b08f91ac20c0567492855`.
  Total build/test time 27.111 s; whole-process peak RSS 53,788,672 B.

The current 48-row sample uses k = 12, six advice columns, two lookups, 2,817 table
rows and eight permutation columns (six advice, constant and instance). There
are six fixed queries before four selectors become fixed columns. Processed VK
size is 586 B in all four configurations.

| Minimum degree | Proof bytes | Keygen ms | Prove ms | Verify ms |
| ---: | ---: | ---: | ---: | ---: |
| 5 | 3,072 | 1,200.995 | 627.337 | 22.391 |
| 8 | 3,040 | 1,196.705 | 729.956 | 22.768 |
| 11 | 2,976 | 1,197.691 | 871.275 | 22.798 |
| 20 | 3,264 | 1,198.186 | 1,139.067 | 22.580 |

Increasing degree reduces permutation products but increases quotient work.
Degree 20 reverses the sample size improvement. The sample does not qualify the
20 ms soft verification budget or maximum-profile resources.

## Full-profile estimates and comparison

These estimates describe composing the tested operations, not synthesized
Blake2b or a maximum proof. A BLAKE2b G uses 72 rows: four 8-row additions,
four 8-row XORs and one 8-row nonbyte rotation. Compression takes 7,040 rows before
loads. The two natively confirmed 75,187 B maximum ciphertext frames require 1,176
compressions: 8,279,040 rows, or at least 127 lanes at k = 16. The advice and lookup
transcript subtotal is 138,176 B. Adding permutation products gives these bounds:

| Degree | Permutation products, at least | Bytes, at least | Space left in 192 KiB |
| ---: | ---: | ---: | ---: |
| 5 | 254 | 170,656 | 25,952 |
| 8 | 127 | 154,400 | 42,208 |
| 11 | 85 | 149,024 | 47,584 |
| 20 | 43 | 143,648 | 52,960 |

All omit fixed evaluations, quotient commitments, non-advice permutation columns,
IPA, CRC64, private BLAKE3, BFV, routing/ranges, loads and the outer envelope.
A straightforward byte CRC sketch alone needs roughly 1.35 million more rows before
its remainder lookup; this is a sketch, not a synthesized lower bound. The full
budget therefore remains unresolved despite the improvement over the nibble
layout. `capacity_byte.py/json` retain exact counts and source hashes.

The existing `pasta_sha256_table8/table8/spread_table.rs` proves 16-bit dense/spread
values with seven advice cells and two byte lookups. It is not a smaller direct
replacement for six-column byte XOR. A purpose-built packed 64-bit spread layout
might fuse XOR with nonbyte rotation, reducing a G from 72 to 64 rows before extra
accumulation. To improve the current advice/lookup subtotal, its corresponding
per-lane cost must stay below 1,224 B (versus 1,088 B now), including any additional
queries. No such packed layout or soundness/native-proof controls exist yet.
The present measurements justify further design work, not full relation admission.

## Exact current sources

- `crates/iroha_core/src/zk/ram_lfe_byte.rs`: `8b9d554e67dd474f1c789c1ec3f174abd8c58de5d27c525ea28c1009ccd30007`.
- `crates/iroha_core/src/zk/ram_lfe_byte_tests.rs`: `4c183203f5bdd3cb48674ce21382c4613c211bb30b90da8f6c0d58f51a536262`.
