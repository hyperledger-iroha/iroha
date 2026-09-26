# FASTPQ F07 fixed FRI fibers — 2026-09-24

The recorded complete offline ordinary and AXT quantity artifacts remain roughly
7.48 and 7.50 MB. Their fixed 375-query shared opening has a 1,050,000-byte
raw row/mixed/quotient floor before authentication and framing, so a wire-only
change cannot make that proof fit the 512 KiB FASTPQ target or the 1 MiB AXT
inner envelope. Production Core still verifies by witness replay.

The inactive DEEP replacement candidate has a narrower, independent wire
inefficiency. Its five FRI rounds have fixed arities `[16,16,8,8,4]`, but each
of 320 maximal linked fibers previously stored a `Vec<Fp4>` sequence count and
framed every 32-byte extension-field value. The sole candidate DTO now stores
one canonical arity byte followed by exactly the complete ordered Fp4 fiber in
raw canonical little-endian form. The decoder accepts only 4, 8, or 16, checks
every field limb and exact consumption, and the proof preflight enforces the
round's expected arity. The old vector frame has no decoder or fallback. This
does not alter the values authenticated by FRI leaves, query positions, folds,
roots, public statement, transcript, or any security parameter.
The one DEEP profile identity now includes `fixed-fri-fiber-wire:v1`, so the
transcript context for these roots is distinct from the earlier candidate;
the canonical `ProofV1` schema also changes with the field type. No old-vector
proof can be interpreted under the new layout.

At the fixed maximum shape, 3,328 Fp4 values formerly carried 3,328 one-byte
element lengths, 320 eight-byte sequence counts, and no arity tags. The new
layout carries 320 arity bytes: `3,328 + 320*8 - 320 = 5,568` bytes saved.
The exact DTO maximum fell from 506,351 to **500,783 bytes**,
leaving 23,505 bytes under 512 KiB. Two maximum child frames alone would use
1,001,566 bytes of AXT's 1,048,576-byte inner ceiling, leaving 47,010 bytes
before carrier, public context, and any further privacy construction. These
are byte-arithmetic screens for a test-only candidate, not a produced private
proof or AXT fit claim.

The decisive next work remains a reviewed source-derived private producer and
opening construction. It must mask every private retained column with fresh
entropy, bind the mask and quotient/composition blinding without revealing
private rows, include the required authenticated OOD relation and `R(x)`
opening, and prove the complete 923-slot zero-remainder AIR under the doubled
degree and FRI progression. It must fit the unchanged segment and two-child AXT
envelopes including Norito framing and public context, use bounded
external-memory LDE/FRI/Merkle ownership, and receive concrete/qROM,
terminal-degree, and witness-privacy review. No offline fixture, size screen,
or DTO roundtrip closes production admission or the release gate.

Validation on the combined `optimizations` source:
`scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p
fastpq_prover --lib deep_ -- --nocapture` passed **55/55**. The maximal linked
canonical frame encoded at exactly 500,783 bytes and matched its independent
wire-length calculation. The bounded decoder measured 5,415 sequence elements
and 3,799,149 cumulative allocation-charge bytes, below the unchanged 8 MiB
cap. The selected suite includes raw-fiber roundtrip under every supported
Norito layout, old-vector rejection, wrong-arity/truncated/trailing and
noncanonical-limb rejection, whole-frame/shape/resource bounds, transcript
binding, FRI linkage and full terminal checks. `rustfmt --edition 2024` and
scoped `git diff --check` passed. No complete private proof was produced.
