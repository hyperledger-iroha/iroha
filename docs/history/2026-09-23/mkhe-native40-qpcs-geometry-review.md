# Native40 qPCS commitment geometry review — 2026-09-23

This is a read-only design review of `optimizations`, not a replacement proof,
resource certificate, or release qualification. No Cargo command or Rust edit was
made. The fixed 512 MiB resident, 16 GiB spool, 64 GiB authenticated-I/O,
128,000,000,000 tracked-work, 30,740,352-byte qPCS and 41,943,040-byte
three-section envelope ceilings remain unchanged.
The current release contract separately fixes the shared six-lane native
proof hash for qPCS payloads, authentication nodes and roots. A different
Merkle hash is not an available optimization under that contract.

## Exact current obstruction

The [tree constructor](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_qpcs_tree.rs)
charges the [shared six-lane hash](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_proof_hash.rs)
at 909 Goldilocks multiplications plus 782 Goldilocks additions per lane
per permutation. The initial oracle has 524,288 leaves and 524,287 binary
internal nodes. Its [payload/index/node frames](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/rns_native_qpcs_leaf.rs)
have 950/96/94 words per lane, so their exact respective charges are
4,819,350 / 487,008 / 476,862 field add/mul operations per hash.

The earlier **505,349,817,048** figure is the explicitly diagnostic case that
hashes **one** repeated 6,000-byte payload, then still hashes all 524,288
index-bound leaves and 524,287 nodes:

`4,819,350 + 524,288 × 487,008 + 524,287 × 476,862 = 505,349,817,048`.

The actual constructor charges every payload, giving
`524,288 × 4,819,350 = 2,526,727,372,800` for payload hashes alone and
`3,032,072,370,498` for the complete initial tree. The payload subtotal
alone exceeds the 128-billion gate by more than 19 times. An arity change,
index-cache change, memory layout, or hardware acceleration cannot make this
six-lane full-payload construction admissible under the same field-operation
meter. The diagnostic one-payload case cannot be used as the admission charge
for distinct source-derived codewords.
Even if a revised schedule materialized only the initial tree and omitted
all quotient and FRI-0 tree work, its **2,526,727,372,800-operation initial
payload subtotal** would remain. Therefore no tree arity, indexing, cache,
storage or root-schedule geometry change that still hashes each current
6,000-byte initial leaf with the required six-lane owner can fit the 128-billion
cap. A fundamentally different commitment/evaluation proof could change
what must be hashed, but that is a new cryptographic protocol, not a geometry
optimization established here.

## Nonconforming research hypothesis

A hypothetical new commitment protocol could retain the **same
outer three-section envelope and 20 ordered 48-byte qPCS transcript root
slots**, while changing what two slots mean and how the remaining trees bind
values. This is **outside the current release contract** and must not be
implemented or admitted without an authorized specification and security
change:

1. Commit the 524,288-point, 400-coordinate initial codeword with an ordered,
   position-binding Merkle tree. Keep 160 unique common queries and their
   complementary pair positions, all 400 canonical Fq2 coordinates per opened
   initial leaf, and the 18 binary FRI folds/four-leaf terminal check.
2. After the initial and q-mask roots fix the 200 relation points, bind the
   exact ordered 400 claimed evaluations *before* deriving the batching seed.
   The existing verifier binds their 3,200 bytes only inside the later qPCS
   section; that chronology is insufficient for a virtual quotient root.
   Derive the quotient slot as a domain-separated native48 transcript tag over
   the initial root, complete relation schedule, evaluation bytes, and source
   commitment identities. At any domain point outside the relation points,
   the quotient value is uniquely `(f(x)-y)/(x-z)` per row; the verifier
   computes it from the authenticated initial opening.
3. Derive the FRI-0 slot as another domain-separated native48 tag after the
   batching seed. The FRI-0 value at a queried point is the existing public
   batch expression of the authenticated `f(x)` and derived quotient value.
   Its fold-0 seed follows this tag. FRI-1 through FRI-17 remain actual
   committed codewords, opened in the existing correlated query schedule.
4. A faster, separately domain-separated 48-byte node/leaf hash is a research
   possibility; BLAKE3 XOF with canonical Goldilocks-lane rejection sampling
   illustrates its shape. Include profile,
   role, layer, domain length, tree level, ordered node position, and exact
   leaf length in each hash input. Position comes from the authenticated path;
   no separate per-leaf index hash is needed. Keep the existing six-lane
   transcript/challenge hash and strict native48 root representation. This
   would be a **new qPCS hash protocol and parameter identity**, not a
   reinterpretation of current roots or a decoder fallback. The present
   [wire specification](../../../specs/crypto/zk_ams_rns_native_wire_v1.md)
   explicitly requires the shared six-lane owner for native proof roots and
   authentication nodes. An unchanged outer envelope does not authorize
   replacing that inner construction. This hypothesis is nonconforming until
   the contract itself is deliberately changed and independently reviewed.

The virtual slots are nonzero, distinct, reproducible transcript *tags*, not
Merkle commitments. Their meaning and the missing quotient/FRI-0 multiproofs
must be explicit in a single revised qPCS section parser. The verifier must
derive both tags itself and reject any different encoded root; query indices
must still be sampled only after all 18 FRI slots have been absorbed. No
prover-chosen payload cache, claimed tag, or new commitment owner may bypass
the original source/session binding.

## Conditional byte geometry, not an admissible resource budget

Only the initial and FRI-1..17 codewords would be materialized. Their exact
leaf count is `2^19 + sum(2^(19-i), i=1..17) = 1,048,572`; 18 binary trees
have `1,048,554` internal nodes. At 6,000 canonical bytes per leaf and 48
bytes per stored digest, retaining every value and tree digest takes
`6,291,432,000 + 100,662,048 = 6,392,094,048` bytes of qPCS spool. If the
existing 3,691,001,600-byte public corpus and 5,026,665,600-byte ordered
snapshot also stay resident on spool, the named subtotal is 15,109,761,248
bytes, leaving 2,070,107,936 under 16 GiB for every other live spool object,
framing and authenticated storage overhead. The complete lifetime plan is
unfinished; this inequality is not a spool certificate.

A qPCS write, complete seal/readback, and later full replay of that entire
6,392,094,048-byte allocation would cost 19,176,282,144 authenticated I/O
bytes. Adding the ordered snapshot's already reserved write/seal charge of
10,053,331,200 gives 29,229,613,344, leaving 39,489,863,392 under 64 GiB
for corpus publication, source rereads, query openings and every other stage.
This is a scenario, not measured I/O. A possible qPCS resident target is
128 MiB: 100,662,048 tree-digest bytes, two 8 MiB row/tile buffers and one
8 MiB I/O buffer total 125,827,872 bytes, leaving 8,389,856 bytes for
qPCS control/scratch. Real NTT, transpose, allocator and concurrently retained
source buffers still need measured whole-session admission below 512 MiB.

With 48-byte authentication nodes, deleting the quotient and FRI-0 tree
openings saves up to `2 × 2,082,816 = 4,165,632` bytes from the existing
29,312,055-byte qPCS wire upper bound. A corresponding hypothetical bound is
25,146,423 bytes, leaving 5,593,929 beneath the unchanged qPCS cap. This
assumes the exact current 320-leaf/3,392-frontier-node bound for each removed
tree; new root/tag framing and the early evaluation binding must be counted
before accepting a revised format. The outer section kinds and envelope cap
need no extra section.

**No tracked-work budget is established for this hypothesis.** The existing
ledger charges Goldilocks field adds/muls for the six-lane hash. BLAKE3's
32-bit additions/XORs/rotates, codeword generation, virtual quotient, FRI
folds and all source work have no reviewed charge in those same tracked units.
Counting BLAKE3 as zero, equating unlike integer and field primitives by fiat,
resetting the source ledger, or using a device timing measurement in place of
operation counts would evade the 128-billion gate. Until a common additive
operation ledger and implementation-derived bound demonstrate
`W_source + W_qPCS + W_other <= 128,000,000,000`, it cannot pass resource
admission. Even such a bound would not remove the independent six-lane
contract blocker. The present constructor must continue to reject before
source access.

## Required proof and next reviewable slice

The mathematical case must prove that committed initial `f`, early-bound
evaluations `y`, and the deterministic quotient/batch functions preserve the
opening argument's soundness when the quotient and FRI-0 trees disappear. In
particular, prove the degree gap for false `y`, challenge independence from
`f` and `y`, 400-row batching soundness over the 40 distinct Fq2 fields, and
the 160-query/18-fold error bound at the required 128-bit target. Reject
relation points on the LDE domain or define the safe denominator rule. A
separate review must establish BLAKE3-derived native48 collision binding,
domain separation from six-lane Fiat–Shamir challenges, canonical-lane sampling,
and the tree's exact position/length binding. Current scoped verifier tests
and qPCS wire upper bounds do not provide these proofs.

The next reviewable qPCS slice is a specification-level comparison that keeps
the current six-lane contract explicit: quantify any proposed shorter-object
commitment in the same tracked field-operation units, define its evaluation
opening equations and transcript chronology, and obtain independent soundness
review before writing a new qPCS verifier or prover. The existing fail-closed
resource admission remains the only conforming execution path for today's
tree. Separately, the governed full40 source and source/context handoff can
advance without treating qPCS as solved. If a future authorized protocol
change selects a different hash, it additionally needs a common additive
work ledger, exact live spool/I/O/RSS accounting and full40/eight-party
measurements before any production composite or readiness switch.
