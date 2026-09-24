# F07 FASTPQ fixed-row opening budget audit — 2026-09-23

This is a source and retained-artifact audit of the current `optimizations`
checkout, not a new proof construction, fresh full-domain run, security review,
or production admission. The 512 KiB FASTPQ segment and 1 MiB AXT inner-payload
ceilings are unchanged. F07 remains open.

## Current canonical geometry and measured gap

The sole compact V1 profile fixes 65,536 trace rows, 342 columns, 923
constraints, an eightfold 524,288-row LDE, 375 distinct queries, 17 binary FRI
folds, and a four-value degree-one terminal. `SharedProof` authenticates complete
current/next rows and separately committed mixed, quotient, and FRI openings.
`RowValues` already stores each complete row as exactly 342 canonical
little-endian `u64` values (2,736 bytes), with no per-cell framing or row count.
Its schema and decoder reject the earlier variable-row representation.

| Account | Bytes | Consequence |
| --- | ---: | --- |
| Mandatory 375 distinct current-row values, `375 * 342 * 8` | 1,026,000 | Exceeds the 524,288-byte FASTPQ cap by 501,712 before any other field. |
| Mandatory 375 mixed/quotient Fp4 pairs, `375 * 64` | 24,000 | Raises the raw opening floor to **1,050,000**. |
| Combined raw floor versus FASTPQ cap | +525,712 | No framing, root, index, authentication or FRI bytes are included. |
| Combined raw floor versus 1,048,576-byte AXT inner cap | +1,424 | Even one segment exceeds the entire AXT inner budget. |
| Two AXT segments' raw floor | 2,100,000 | Exceeds the inner cap by 1,051,424 before the carrier and binding. |

The exact current `shared_prover_wire_bound` test and producer preflight require
**4,017,376 canonical framed bytes** for the conservative maximal valid
single-segment shape; the repeated internal opening representation is separately
bounded at 7,791,716 bytes. These are structural upper bounds, not measured
actual proofs. Retained full-domain ordinary and AXT segment artifacts are
3,994,619 and 4,015,551 bytes, and retained two-segment bundles are 7,986,384
and 8,011,999 bytes, respectively. Those captures used explicit offline budgets
and do not demonstrate production fit or current-source qualification.

The target-only September 23 geometry screen initially quoted **4,279,877
bytes** as the current maximal segment. That modeled the former variable-row
vector, not the present fixed-row codec. Its corrected calculator now matches
the source test's **4,017,376-byte** value. The difference is **262,501 bytes**:
750 rows save 262,500 bytes of element framing and the enclosing field length
saves one byte. This correction does not change the 1,050,000-byte raw lower
bound or either fixed ceiling. The already implemented fixed-row reduction is
the sound bounded encoding gain; repeating it or applying general-purpose
compression cannot close the remaining worst-case gap.

The same target-only calculator now screens the structurally admissible query
subset `I={floor(i*524288/375): 0<=i<375}`. Each index is a canonical field
coordinate, so a 401-word conditional ideal sampler tape can place these 375
distinct labels first and fill its remaining coordinates canonically. The
subset has positive probability under the stated ideal-sampler premise;
reachability from the concrete six-lane sponge is not established. In the
present binary FRI layout, its
complete group values and exact Merkle frontier alone occupy **272,512 +
875,760 = 1,148,272 bytes**. A dynamic-programming screen over every
power-of-two partition of the 17 fold bits, preserving the same complete-group
and Merkle-authentication model, finds a minimum of **556,496 bytes** at fold
powers `(3,3,3,8)`. This is **32,208 bytes above the 512 KiB segment ceiling**
before other proof fields. It falsifies arity-only tuning as a guaranteed-size
solution for this Merkle-opening model; it says nothing about the soundness of
other fold schedules or the size of a different commitment construction. The
screen's hypothetical 32-column/64-query fixed-row size is **469,093 bytes**,
but its `2^-35`–`2^-34` conditional query-miss term and 2 GiB raw LDE matrix
still reject it as a production substitute.

An independent boundary-mask enumeration now checks all **65,536** ordered
power-of-two fold partitions against the dynamic-programming result; the
unrestricted minimum is unique. The source's test-only fold kernel supports
arities 2, 4, 8 and 16, so the tighter source-supported search admits
**39,648** partitions. Its unique minimum is **558,544 bytes** at powers
`(3,3,3,4,4)`, already **34,256 bytes over** the cap. The five rounds charge
`(96,000 + 132,576)`, `(96,000 + 78,576)`, `(96,000 + 24,576)`,
`(32,768 + 0)` and `(2,048 + 0)` bytes of group values plus Merkle frontier.
An exhaustive small-tree test separately checks the frontier-count identity
against explicit sibling paths for every nonempty subset of 2-, 4- and
8-leaf trees. These checks certify the stated arithmetic inside this opening
model, not another schedule's AIR/FRI security or implementability.

## Required engineering seam

The next representation must replace complete-row disclosures, not just their
encoding. A candidate batched opening must bind every trace column before
Fiat–Shamir challenges, authenticate current and shifted evaluations against
the same commitment, prove all 923 nonlinear AIR constraints and the quotient
equation, preserve terminal degree/proximity soundness, and mask private
evaluations. It must account for authentication as well as values: the current
maximal FRI sibling digests alone are 875,760 bytes. The proposed alternative
needs an independently reviewed concrete/qROM and witness-privacy argument
over the complete two-segment transcript, with exact Norito, memory and work
measurements below **both** segment and AXT carrier limits. A narrowed
64-query geometry is not a substitute: the screened conditional query-miss
term is only between `2^-35` and `2^-34`, before qROM losses, and its raw LDE
matrix reaches 2 GiB before scratch.

The implementation seam is
`crates/fastpq_prover/src/backend/compact_protocol/{profile,shared_openings}.rs`
for one canonical opening relation and transcript, followed by
`compact_v1.rs`, `compact_bundle.rs`, and the bounded public verifier. Replace
the current DTO in the first-release cut; do not add a compatibility decoder,
lower the security target, raise the resource ceilings, or claim that passing
offline fixtures makes Core's witness-replay path a compact verifier. AXT's
authoritative execution/state/nonce work remains a separate F07 blocker.

The working-tree diagnostic is
`scripts/fastpq/check_compact_geometry_screen.py`; its retained target copy
is byte-identical. `python3 scripts/fastpq/check_compact_geometry_screen.py`
completed with all fixed-size and arity assertions, and
`python3 -m pytest -q scripts/fastpq/tests/test_compact_geometry_screen.py`
passed **7/7**; `python3 -m py_compile` passed for the two Python files.
These are byte-arithmetic checks only. No compact proof was
produced or admitted, and F07 remains open.
