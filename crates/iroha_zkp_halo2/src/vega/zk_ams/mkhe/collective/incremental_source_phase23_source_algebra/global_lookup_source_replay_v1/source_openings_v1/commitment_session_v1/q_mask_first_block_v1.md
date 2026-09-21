# Original private-uniform S stream and sole opening ownership

This private producer consumes the original verified materialized-source/pair
owner. It keeps that same source session, original RNG, canonical public T256
table, sole commitment inventory and actual proof/storage ledgers through S
sampling, point production, whole-file sealing and authenticated block replay.
It does not supply native40 source qualification, qPCS or composite admission.

For each `(limb, repetition, block)` in 40×5×8 order, the one shared sampler draws
16,384 coefficients with existing `sample_below(q_l, original_random)`, except
the final ring coefficient is fixed zero without a draw. The original buffer is
erased/refilled in place. Its eight16 KiB u64LE records are written in order before
four original rho draws and four actual digit commitments. Each point is
`sum G_i * ((S_i >> (15*h)) & 32767) + H*rho_h`; the immutable original canonical
basis and full-width nonzero rho kernel are unchanged. The same evaluator serves
first and later blocks. No caller supplies S, rho, bases, randomness or a root.

The point tickets occupy exactly physical 27176..33576 in
`4*((limb*5+repetition)*8+block)+digit` order. One exact 6,400-scalar allocation is
reserved before the first rho and initialized four entries per block. No second
point or rho inventory is constructed. The source cursor reaches complement
ordinal 0 only after all 6,400 genuine calls complete. Identity points fail the
consuming operation; they do not trigger resampling or value substitution.

Before each continuation, admit its four concrete evaluations and named staging
on the original proof ledger. Only pre-operation Capacity preserves the complete
original valid prefix, including existing file, S allocation, initialized rhos,
phase and RNG counters. The whole-stream driver can return such a prefix after
other blocks have completed; retry resumes its exact next block and repeats no
accepted draw. After admission, entropy/allocation/write/point/order failures or
unwind consume it. Failure cannot return a partly admitted block or a new RNG.
The distinct original mask and rho byte counters retain their existing limits.

The sibling S file has12,800 records,209,920,000 actual bytes and 419,840,000 reserved
write/seal I/O bytes. Its original reservation precedes file creation and the
first S draw; no new ledger is accepted during continuation. Full completion
requires exact final source/rho/ticket/file coordinates, erases the last in-memory
S values, consumes the actual complete leaf seal and retains its authenticated
snapshot. The same 128 KiB allocation remains as an erased replay buffer; the
200 KiB original rho allocation remains owned. File/key destruction precedes
release of live-spool credit. This is an unlinked process-local spool, not
restart persistence, secure deletion or physical-memory qualification.

Each replay block authorizes all eight real record reads on the same storage
ledger before any read or new byte allocation. Capacity restores the same sealed
owner and leaves the buffer/cursors unchanged. Actual authentication, coordinate,
shape, value or incomplete-read failure consumes the owner. Replayed values must
be canonical residues with exact zero top coefficients. The original S snapshot
came from the same private payload that produced each corresponding original
ticket/rho; replay retains that custody and checks exact original coordinates.
There is no caller-provided validity cache, replacement snapshot or raw-secret
callback. Only the named complement consumer can advance this read pass. It
erases the loaded block after all four complement commitments succeed; source,
file and both rho sets remain together. The former unconsumed generic replay,
advance and completion facade is removed, with no compatibility alias.

Each leaf read now returns one borrowed owning chunk. Its exclusive parent-read
lifetime prevents another read, finishing or dropping that parent, and escaping
a plaintext borrow before the actual zeroizing chunk is destroyed. An explicit
destructor preserves drop-check through normal scope-end destruction after the
last plaintext use; `PhantomData` alone would let that borrow end earlier under
NLL. The wrapper exposes only length and a borrowed plaintext view, with no raw
owner extraction or clone. This is ordinary Rust borrow/destruction custody, not
a claim of leak-safe accounting under deliberate `mem::forget`, whole-process
RSS admission, or complete proof qualification.

Local storage binding uses only the existing original source/materialization
context, original pair identity and canonical profile/basis/layout. It adds no
public transcript frame. The later actual initial qPCS root must precede the
existing S-point root and relation challenge. This producer does not construct
that root or mask P/H itself. The completed private owner retains its file/key,
original scalar allocation and source. Its named complement phase computes the
integer `q_l-1-S_i` before extracting four15-bit digits, then commits each with
the same original basis and newly sampled original-session full-width rho.
These6,400 tickets occupy33576..39976; the next purpose is `Multiplicity`, ordinal0.
The final S coefficient remains zero, but its complement is `q_l-1`. No per-digit
`32767-d` shortcut, caller mask or forced zero complement is permitted.

The complete complement scalar allocation adds204,800 retained payload bytes.
Its owner reservation precedes allocation and survives scalar destruction; four
existing kernel admissions and concrete staging coexist before each block read.
Only pre-I/O Capacity returns the original unchanged block owner; after a read
or entropy request, all failure and unwind paths consume its retained phase.
The complete file read pass charges209,920,000 bytes on the original storage
ledger. No new S file or mask entropy is created. The terminal owner retains
both rho sets and immutable S file for future separately admitted P~/H~/qPCS
reads. None of the still-closed proof/provider seals is inhabited here, and
actual commitment production is not a same-opening or linear-relation proof.

Named-buffer accounting excludes original uncharged source/control allocations,
allocator/Arc overhead, dependency-private scratch, compiler frames, page cache
and RSS. T256 and Goldilocks operations still lack a reviewed common whole-proof
work accounting. The fixed512 MiB/16 GiB/64 GiB/128B ceilings and serial T256 hardware
policy are unchanged; full S production alone cannot qualify proof resources.
The existing qPCS initial-tree construction independently exceeds the work cap.
The upstream38-limb source is not relabelled as native40 by this continuation.

TODO: complete original P~/H~/qPCS consumers, multiplicity/inverse producers,
exact same-opening/proof/transcript linkage, governed native40 source/parameters
and security evidence, whole-session accounting, full S and complement
6,400-MSM/resource/hardware validation and production
qualification. Source-boundary tests with synthetic prior inventories must remain
explicitly unqualified; they cannot replace an authenticated-source pipeline run.
