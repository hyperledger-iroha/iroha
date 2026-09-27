# FASTPQ F07 base-field hiding and degree screen — 2026-09-24

This is a design screen on `optimizations`, not an implemented proof, security
reduction, or production qualification. The 512 KiB segment target, 1 MiB AXT
inner-payload ceiling, complete 923-slot transfer statement, and Core's current
witness-replay admission remain unchanged. It proposes a replacement for the
inactive DEEP candidate, not a second admitted proof format.

## Why the current owners cannot be connected directly

The production-sized [DEEP DTO](../../../crates/fastpq_prover/src/backend/deep_proof.rs)
has 301 base-field committed columns, 64 queries, 8,388,608 LDE rows, five
folds `(16,16,8,8,4)`, a full 128-value constant terminal, and an exact
506,351-byte maximum frame. Its [source polynomial owner](../../../crates/fastpq_prover/src/backend/deep_polynomial.rs)
requires each trace column to have degree `<N`, `N=65,536`, and the quotient
degree `<2N`. Every nonzero subgroup-preserving additive mask has the form
`(X^N-1)M(X)` and raises a degree-`<N` trace polynomial to at least degree `N`.
Leaving the 64 authenticated row disclosures unmasked exposes private values.
The [checked 342-to-301 projection](../../../crates/fastpq_prover/src/backend/compact_public_columns.rs)
only removes 41 verifier-known columns; it does not hide the other 301 or prove
their relation. These are algebraic and privacy failures of a direct producer
connection, even though the DTO is under its byte cap.

The [full transfer relation](../../../crates/fastpq_prover/src/backend/compact_transfer_air.rs)
still derives all 923 numerator slots from the complete statement and 342-cell
reference row. Its seven `PublicIO` fields are `dsid`, `slot`, `old_root`,
`new_root`, `perm_root`, `tx_set_hash`, and `ordering_hash`; the
[AXT segment owner](../../../crates/fastpq_prover/src/backend/compact_axt_batch.rs)
also binds the original whole batch, segment ordinal, AXT binding, mirrors and
remote-spend claims. None of this proves the caller's finalized source roots or
durable spend-nonce authority.

## One concrete replacement candidate to evaluate

Keep the 301 base-field row cells and the complete public-column reconstruction.
For each committed column interpolate its actual physical source polynomial
`C_j`, draw independent **base-field** coefficients `M_j` uniformly from
`F_p[X]_<h`, and commit `C'_j=C_j+(X^N-1)M_j`, with `h=32,768`. The equality
`C'_j|H=C_j|H` preserves the 65,536 physical execution rows; no private cell
is lowered to a public fixture. Source [masking arithmetic](../../../crates/fastpq_prover/src/backend/coefficient_masking.rs)
already expresses this transform but currently handles Fp4 masks and is test
only. The 41 omitted public columns stay reconstructed from the bound statement
at the actual evaluation point.

Commit the full 923-slot AIR quotient only after the row root and independent
constraint challenge. Retain two quotient values per query using
`Q(X)=Q_0(X)+X^N Q_1(X)`, but draw an independent Fp4 polynomial `S` before the
quotient root and commit `Q'_0=Q_0+X^N S`, `Q'_1=Q_1-S`. This preserves the
OOD identity while hiding the separately disclosed halves; it needs exact
degree and zero-remainder checks. Before the DEEP batching challenge, also
sample an independent Fp4 composition mask `R` and bind its value into each
row-oracle leaf. The first FRI word must batch `R` with the DEEP components
under a post-commitment challenge; the query check then reads authenticated
`R(x)` from the same row opening. The independent mask is needed because FRI
folds can exhaust the entropy of subgroup-vanishing trace masks. Haböck and
Al Kindi's [primary analysis](https://eprint.iacr.org/2024/1037.pdf),
Protocol 2 and Lemma 2, makes precisely that distinction. Their canonical
quotient discussion in §4.1 motivates the zero-sum split blinding; neither
paper construction can be imported without an exact protocol mapping.

Raise the sole FRI degree claim from `<N` to `<2N` while retaining the same
8,388,608-point domain and five fold factors. The complete 128-value terminal
must then be checked as a degree-`<2` polynomial, rather than a constant.
The source's [923-slot degree test](../../../crates/fastpq_prover/src/backend/compact_transfer_air.rs)
gives, for uniform committed-column bound `d=N+h`, a worst hash numerator
bound `2d-1+(N-N/512)` and an SMT bound `d+N-1`. Here these are 262,015 and
163,839, so exact division by `X^N-1` yields quotient bound **196,479**.
Its high half has bound `196,479-N=130,943`, still below `2N=131,072`.
The masked trace bound is 98,304; the two-point DEEP divisions and their
`X²`/`X` shifts remain below `2N`, as does a composition mask sampled with
degree `<2N`. This is a degree *screen*, conditional on producing and checking
the complete source numerator and zero remainder. The old
[`<N` source and terminal checks](../../../crates/fastpq_prover/src/backend/deep_geometry.rs)
would have to be replaced coherently.

| Byte/work account | Candidate value |
| --- | ---: |
| Current maximal DEEP frame | 506,351 bytes |
| Append one inline Fp4 `R(x)` to each of 64 fixed row bodies | +2,048 bytes |
| Screened maximal frame / margin below 524,288 | **508,399 / 15,889 bytes** |
| Two maximal children / margin below 1,048,576, before carrier and binding | **1,016,798 / 31,778 bytes** |
| 301 base-field LDE columns, `301*8,388,608*8` | **20,199,768,064 bytes** |
| Row-oracle leaf payload with `R`, `8,388,608*(301*8+32)` | **20,468,203,520 bytes** |
| 301 mask coefficient payload, `301*32,768*8` | **78,905,344 bytes** |

The +2,048 assumes a new fixed inline 2,440-byte row body, with no new
per-value field or Merkle tree: its Norito length-prefix width stays the same
as the current 2,408-byte body. It is a deterministic codec projection, not
an encoded proof measurement. The outer AXT carrier, actual public context,
allocation charges, quotient/FRI arrays, Merkle nodes, and scratch remain to be
measured. The 20.2 GB raw LDE already rules out simply materializing every
column in an ordinary small prover workspace; source-owned streaming or
external-memory proving and its real work/RSS limits are required.

## Security and completion gates

The privacy choice has arithmetic room, but no transferred theorem. As an
intentionally conservative disclosure screen, 64 fibers in each of the five
rounds expose at most `64*(16+16+8+8+4)=3,328` Fp4 values; add all 128 terminal
values to get `n_D=3,456`. With one quartic OOD point (`e=4`, `n_F=1`) and a
two-part quotient, the *stricter FFT-split* mask inequality in Haböck–Kindi
§3, `2d(e n_F+n_D)+n_D <= h` at `d=2`, asks for 17,296 coefficients, below
32,768. Their separate canonical-split condition asks for independent split
randomness `h_p >= n_F+n_D=3,457`; choosing that extent leaves the screened
`<2N` component bounds intact. These are upper-envelope comparisons, not a
privacy proof for this different canonical split, multi-arity FRI, full
terminal disclosure, transcript or adaptive Fiat–Shamir verifier. A simulator
must account for every opened trace, quotient, `R`, FRI and OOD value, including
Frobenius conjugates of extension-field evaluations.

Soundness must be rederived at rate `2N/8,388,608=1/64`, 64 queries, degree-two
terminal, added masked oracle and exact shared multiproofs. The
[DEEP-FRI/DEEP-ALI paper](https://drops.dagstuhl.de/storage/00lipics/lipics-vol151-itcs2020/LIPIcs.ITCS.2020.5/LIPIcs.ITCS.2020.5.pdf)
supplies relevant IOP analyses, but not this AIR, hiding or implementation's
64-query concrete/qROM bound; the
[QROM compiler theorem](https://eprint.iacr.org/2019/834.pdf) requires its own
round-by-round soundness and complete oracle-query accounting. The existing [375-query conditional
profile](../../../specs/fastpq_compact_typed_profile.md) belongs to another
geometry and cannot certify this one. The complete statement, canonical source
order, root/nonce/AXT authority, transcript chronology, challenge distribution,
mask entropy, authenticated low-degree binding of `R` and both quotient halves,
and bounded two-segment AXT wire must all be proved and measured before Core
could replace replay. No Cargo, proof production, or admission test was run for
this note; F07 remains open.

The [deterministic geometry screen](../../../scripts/fastpq/check_compact_geometry_screen.py)
now reads the live test-only DEEP frame bound and fails if its 506,351-byte
baseline, the `<2N` degree arithmetic, the exact 2,048-byte inline row addition,
or either fixed-cap margin drifts. It remains explicitly unqualified. On this
checkout, `python3 scripts/fastpq/check_compact_geometry_screen.py` exited 0
and printed the 508,399-byte candidate frame; `python3 -m pytest -q
scripts/fastpq/tests/test_compact_geometry_screen.py` passed **9/9**, including
source-frame and cap-drift refusals; `python3 -m py_compile
scripts/fastpq/check_compact_geometry_screen.py
scripts/fastpq/tests/test_compact_geometry_screen.py` exited 0.
