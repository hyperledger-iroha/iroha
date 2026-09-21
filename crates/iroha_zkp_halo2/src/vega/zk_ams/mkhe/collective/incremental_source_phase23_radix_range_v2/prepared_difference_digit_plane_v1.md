# Prepared centering-difference digits

`Phase23RadixWitnessMaterializedV2::into_difference_digit_preparation_v1`
consumes the original materialized source only at logical comparator ordinal
688 and the exact completed `bD`/`bS` commitment-session position 12,728. The
retained session rejects an earlier stage, a repeated delta pass, and an
occupied first delta slot before source I/O. Materialization record, original
source lineage, mapping, seal and compact snapshot context are checked before
allocation or I/O.

The driver moves the only source evidence into the existing strict canonical
cursor. It reads each of the 43 × 512 source blocks once. One group retains
16,384 canonical T256 source coefficients in source `(block, coefficient)`
order and three authenticated compact comparator lanes. Every bit of all three
lanes, including reserved high bits and the final coefficient, must equal the
witness freshly derived from the original source. Source scalar projection
applies `v = coefficient × 64 + block` exactly once; packed lanes already use
that order.

For `B = 2^15` and `K = (pT + 1)/2`, the next private cursor chooses
`group = ordinal / 17`, `h = ordinal % 17`. Values are

- `delta[0] = D[0] + B × beta[0] − K[0]`;
- `delta[h] = D[h] + B × beta[h] − K[h] − beta[h−1]` for `h > 0`.

The integer intermediate is at most 65,535 and remains in the existing
zeroizing `u16` scratch owner. Checked subtraction and a strict `< B` predicate
reject malformed borrow data rather than reducing it in the T256 field.
Only then is each digit converted to a T256 scalar.

The borrowed prepared statement binds original replay/receipt digests and the
exact source cursor position after the group's 64 reads. It has no constructor
outside its source module and exposes neither values nor entropy. Before any
value chunk can be emitted, the original retained session samples its bounded
nonzero canonical rho, executes the existing deterministic secret MSM over
`G[0..16384]` plus `rho H`, and inserts only that actual nonidentity point into
the next empty shared inventory slot. The shared native descriptor is checked
for all 5,848 delta positions 12,728..18,576. Prior D/S/top blindings and the
single D/S append permit move into the delta owner without root rebinding or
session replacement. Delta blindings use one exact zeroizing allocation of
187,136 bytes within the existing complete-inventory reservation.

Each prepared plane emits the existing 32 ordered chunks of 512 canonical
big-endian scalars. Wrong, repeated, skipped, extra or incomplete chunks
consume the source and values. Entropy/MSM/adoption errors and unwinds also
consume the original session. Completing a plane returns only the private
driver; completing all 5,848 planes returns the original source only after the
strict authenticated source schedule matches the original materialization
record and the same session is exactly at beta inventory ordinal 18,576.
There is no reset, external point injection, caller-selected ordinal or new RNG.

The incremental payload retained for one group is 524,288 source bytes,
49,152 packed-lane bytes and 524,288 prepared-value bytes, plus the returned
16,384-byte chunk. These numbers exclude the existing MSM workspace, session
inventory, source owners and allocator overhead; they are not an RSS result.
This pass performs 180,707,328 authenticated source-read bytes and 16,924,800
compact-slot read bytes. Eventual whole-proof lifecycle accounting must include
this additional pass; the 512 MiB resident, 16 GiB spool, 64 GiB I/O and
128-billion work caps are unchanged. No complete plane spool is allocated and
the existing 9,288-plane comparator/sign storage descriptor is unchanged.

Tests cover independent integer vectors at zero, radix, threshold, top-bit
and modulus boundaries; exact coordinate order; sealed-lane/source mismatches;
canonical scalar chunk order; original read/origin binding; actual first/last
sparse MSMs and retained rho; stage/cursor rejection; and entropy error, zero
and unwind paths. Synthetic earlier inventory points in transition tests are
explicit fixtures, not authenticated source evidence or an actual complete
5,848-MSM execution. Later-stage setup samples every original prior rho and
validates the same final owner/root KATs while avoiding repeated D/S token
checks already covered by that stage's tests.

The completed delta owner now feeds the existing sealed-value comparator
preparer through a private beta/m continuation. It retains this owner's
blindings and prior material while filling inventory 18,576..25,112, then stops
before signed values. See [comparator preparation](prepared_comparator_plane_v1.md).

TODO: connect signed production after beta/m, consume retained rho
in the required stored-opening tail/writer, inhabit the authenticated production
scratch-sink and proof authorities, finish verifier-linked opening proofs, and
qualify full-size source/commitment/resource/hardware execution. No production
admission or release qualification gate is relaxed by this transition.

Local validation on 2026-09-20: the original delta candidate's 12 focused tests
passed with no failures or ignored tests in 849.52 seconds. The local evidence
is `target/first-release-mkhe-delta-tests.log` with the 16-file source capture
`target/first-release-mkhe-delta-source.sha256.txt`. The later prior-fixture
setup optimization and beta/m admission are subsequent source changes and
require their own rerun. This result does not qualify full authenticated-source
execution, 5,848 actual MSMs, stored-opening tails, composite proofs or resources.
