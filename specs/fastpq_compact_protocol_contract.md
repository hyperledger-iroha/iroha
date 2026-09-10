# Compact FASTPQ V1 protocol contract

Source contract: 2026-09-08. The normal-library offline verifier uses one fixed
six-lane commitment and field-tape construction. Its prover remains test-only.
This is an implementation contract, not production admission, a soundness
certificate or a zero-knowledge claim. Production verification still uses the
raw replay route. The [production goals](fastpq_production_readiness.md) retain
its required replacement by a qualified bounded-opening representation.

## Caller and algebraic inputs

[`FixedAir`](../crates/fastpq_prover/src/backend/compact_protocol.rs) supplies the
ordered columns, ordered numerator slots, relation identity, complete public
statement and evaluation function. Proof bytes cannot choose these inputs.
[`profile::check_geometry`](../crates/fastpq_prover/src/backend/compact_protocol/profile.rs)
accepts exactly 65,536 trace rows, 524,288 LDE rows, 342 base-field columns,
923 numerator slots, blowup eight, binary FRI with 17 folds, four terminal values
and exclusive terminal degree bound one. There are exactly 375 query positions.

Base cells are canonical elements of `F_p`, `p=2^64-2^32+1`. Aggregation and FRI
use `K=F_p[u]/(u^4-7)`, with coefficients in the declared basis order. The
[domain check](../crates/fastpq_prover/src/backend/fixed_domain.rs) verifies exact
root orders, subgroup agreement and a disjoint nonzero evaluation coset. Write
`D={a*g^i : 0<=i<L}`, `H=<g^8>`, `N=65,536`, `L=524,288` and `Z_H(X)=X^N-1`.
Every numerator must have degree below `3N` after substituting degree-below-`N`
trace columns; honest division by `Z_H` then gives degree below `2N`. The Rust
interface does not prove this obligation for an arbitrary evaluation function.

The [public-transfer constructor](../crates/fastpq_prover/src/backend/compact_public_transfer.rs)
derives SMT ports from validated public facts and compares all seven independently
expected `PublicIO` fields: `dsid`, `slot`, `old_root`, `new_root`, `perm_root`,
`tx_set_hash` and `ordering_hash`. The ordinary and AXT offline entry points have
separate complete public contexts. AXT also checks its binding, original metadata,
mirrors and remote-spend preimages. Caller equality does not establish execution
authority, permission membership or source finality. Touched-balance roots are not
thereby authenticated consensus roots; AXT expiry, handle/replay, amount-resolution
and post-proof commitment checks remain caller obligations.

## One complete context and hash owner

[`compact_v1::Context`](../crates/fastpq_prover/src/backend/compact_v1.rs) uses
`fastpq_isi`'s canonical six-lane Digest384 owner for every leaf, parent,
transcript-chain commitment and field-tape block. Its typed catalog is
`iroha-privacy-exact12-v1`; its protocol is `FASTPQ_FINAL_V1.name`,
`fastpq-state-transition-stark-v1`. A geometry identity belongs inside the complete
profile-context frame, not in a second protocol tag. All six canonical field
coordinates are retained as 48 little-endian bytes.

The [framing contract](fastpq_compact_v1_framing.md) specifies every domain and
body field, the 256 KiB complete-context ceiling and immutable prefix reuse.
There is no SHAKE or prototype selector at this compact boundary. Canonical
Norito framing fixes schema, layout, lengths, checksum and exact field order.
The caller's statement and geometry are present in every logical hash input;
short metadata IDs or context digests do not replace them.

The prover interpolates columns on `H`, evaluates them on `D`, and commits
complete LDE rows. Verification does not reconstruct the trace or its FFTs.

| Object | Canonical leaf payload | Oracle code |
| --- | --- | --- |
| Row oracle `A` | 342 base cells in column order, 2,736 bytes | 1 |
| Mixed oracle `T` | One Fp4 value, 32 bytes | 2 |
| Quotient oracle `Q` | One Fp4 value, 32 bytes | 3 |
| FRI round 0..16 | Binary strided pair, 64 bytes | 4, exact round |
| FRI round 17 | Complete four-value terminal vector, 128 bytes | 4, round 17 |

Leaf bodies bind position and exact oracle/round. Parent bodies bind exact tree
level and position plus separately ordered full child digests. The singleton
terminal tree duplicates its leaf in the parent hash; its authentication frontier
is empty. These commitments use the same context owner during proving and checking.

## Whole-message transcript and query decoder

The transcript starts at the six-word zero predecessor. Each message is one
fixed-length tape of complete `F_p^6` blocks. Its serialized output alphabet is
not uniform binary strings. Coefficients read canonical groups of four field
coordinates directly. Before any decoding, every coordinate in the entire tape
must be canonical, including all unused suffix coordinates.

| Message | Decoded result | Tape blocks / bytes | Root bound in the following chain |
| --- | --- | --- | --- |
| 1 | Dummy message | 1 / 48 | Row root |
| 2 | 342 column coefficients | 228 / 10,944 | Mixed root |
| 3 | 923 constraint coefficients | 616 / 29,568 | Quotient root |
| 4 | Joint coefficients `rho`, `sigma` | 2 / 96 | FRI root 0 |
| 5..21 | One `beta` per message | 1 / 48 each | FRI roots 1..17 |
| 22 | Exactly 375 sorted distinct positions | 67 / 3,216 | None; transcript complete |

Every one of the 21 chain hashes binds the complete preceding tape and the
next root. A tape's unused words are retained in that chain. In particular,
message 1 has positive length despite its empty mathematical result. A successful
schedule expands 931 digest blocks, totaling 44,688 tape bytes. Challenges and
commits enforce `Ready -> Pending -> Ready`; the final query result completes
the transcript. Decode/hash failure permanently aborts it. An out-of-phase call
returns an error without enabling retries or advancing the state. Transcript
completion alone is not proof acceptance.

The query tape has 402 canonical coordinates. The decoder considers exactly the
first 401, rejects the sole residue `p-1` because `p mod L=1`, maps accepted words
modulo `L` and retains the first 375 distinct positions in ascending order. It
never requests an extra block or returns a partial set. Fewer than 375 distinct
positions aborts the transcript. Coordinate 402 is decoding padding, but it still
must be canonical. Conditional uniformity and an honest-abort probability require
the stated ideal field-product model; no uniform-byte argument applies.

## Shared openings and checked equations

The sole compact wire DTO is
`fastpq_prover::compact_v1::SharedProofV1`. Its fields, in order, are `row_root`,
`mixed_root`, `quotient_root`, `fri_roots`, `rows`, `queries`, `row_siblings`,
`mixed_siblings`, `quotient_siblings`, `rounds`, `terminal_values`. Ordinary and AXT
bundles have distinct `compact_v1::OrdinaryTransferBundleV1` and
`compact_v1::AxtTransferBundleV1` schemas. Old compact schema names are rejection
controls, never accepted alternatives. The expanded `SinglePhaseProofV1` is an
internal test-prover diagnostic and converts into this same shared verifier.

For the sorted transcript set `I`, verification derives all supplied indices:

- Complete rows at `I union {(i+8) mod L : i in I}`; mixed and quotient values at
  exactly `I`.
- At round length `L_r`, group indices reduced modulo `L_r/2` and deduplicated.
  Group `j` contains positions `j` and `j+L_r/2`, not neighboring evaluations.
- Exactly one complete terminal vector in natural order.

Each table and minimal sibling frontier must equal the caller-derived plan.
The wire supplies neither node coordinates nor unused siblings. Every value
is authenticated before evaluating these equations at `x=a*g^i`:

```
T(x) = sum_j lambda_j * A_j(x)
Q(x) = sum_k alpha_k * C_k(x, A(x), A(g^8*x)) / (x^N - 1)
V_0(x) = Q(x) + (rho + sigma*x^N) * T(x)
```

Public masks are evaluated at `x`; LDE indices do not select base-row opcodes.
For a strided pair `(v_+,v_-)` at `(x,-x)`, the fold is
`(v_++v_-)/2 + beta_r*(v_+-v_-)/(2x)`. The domain generator and offset square
at every fold. Each original query keeps its own value/index carry when groups
merge. Every carry must match the selected pair member, computed fold and final
entry. The exclusive degree bound `2N` halves exactly to one. Verification checks
the degree of the whole authenticated four-value terminal vector.

## Resources, evidence and remaining obligations

The [codec](../crates/fastpq_prover/src/backend/compact_protocol/shared_openings/codec.rs)
checks the frame-byte ceiling before header/CRC work. Canonical header, schema,
layout, checksum, exact consumption, geometry-derived sequence/element ceilings,
depth and cumulative allocation charges are checked before the transcript.
Canonical re-encoding comparison does not allocate another whole frame. Norito
allocation charges are not a peak-RSS measurement. Explicit diagnostic policies
cannot change the cryptographic geometry or the production defaults.

The current DTO alone requires at least
`375*342*8 + 375*64 = 1,050,000` raw row and mixed/quotient bytes. This exceeds
the 512 KiB compact proof target and the 1 MiB AXT ceiling, even before framing,
indices, roots, frontiers or FRI data. The separate maximum row shape has 750 rows
and 2,052,000 raw row bytes. It is not a lower bound on every proof. Explicit
offline budgets do not close the production resource gap. Replay's derived
payload/frame limits are separate policies and do not admit this compact profile.

Normal-library quantity producers use the same sole compact V1 owner. Before
private columns or transforms, they require exact public context and geometry,
per-segment output capacity of 4,279,877 framed bytes, and explicit trace,
private-tree and decode budgets. The temporary repeated-opening representation
has a separate 7,791,716-byte bound. Row and AIR evaluation partitions own at most
32 independent workspaces, with deterministic row and error order. Segments run
sequentially, and the complete artifact passes the public verifier before being
returned. These structural charges do not measure peak RSS or establish
production authority.

Tests cover exact framing and independent known answers, owned/borrowed/streaming
prefix equality, full-width indices and concurrent context reuse, fixed tape
lengths, suffix canonicality, state/abort order, canonical codec errors and
terminal degree checks. The fixed-column test AIR is a codec/transcript/opening
fixture; it is not a release witness or a substitute for actual public-transfer
relation tests. The original 512-row HashDigestAir and its row-407 public-bit
constraint remain a separate algebra boundary. Test declarations and compiler
success are not proof execution or security qualification.

The conditional [row-linkage](fastpq_compact_row_linkage.md),
[AIR degree ledger](fastpq_compact_air_degree_ledger.md),
[interactive AIR bound](fastpq_compact_air_bound.md),
[round-by-round](fastpq_compact_round_by_round.md),
[typed compiler](fastpq_compact_typed_compiler.md) and
[adaptive-context](fastpq_compact_adaptive_context.md) analyses retain their exact
hypotheses. An internally reviewed conditional block-to-whole-tuple reduction
uses two ideal group queries per encoded digest query; it does not model field
words as uniform bytes. Its complete-context/injective-framing, fixed-block,
finite auxiliary-domain and every-prefix premises must be mapped to this code.
It does not establish concrete six-lane or public internal-permutation security.
No original SHAKE theorem transfers merely because geometry is unchanged.

TODO: Complete that mapping, external cryptographic and AIR review, witness
privacy, a representation that meets the production resource targets, release
hardware parity and authenticated production integration. Core/AXT/CLI/SDK and
persistence consumers must transition together; no compact activation or final
public raw-wire cutover is established by this offline implementation.
