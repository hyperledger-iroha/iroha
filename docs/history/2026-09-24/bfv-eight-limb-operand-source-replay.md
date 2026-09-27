# BFV eight-limb operand-to-source-product replay, 2026-09-24

The registered RAM-LFE BFV centered source chain has eight RNS limbs at degree
64. The earlier source-bound check rejects out-of-range residues before
target-limb conversion, but a product can be in range without having been
computed from its claimed operands. Two such product changes can cancel before
the rounded sum, leaving the final output unchanged.

`BfvRnsModulusChain::validate_centered_product_pair_from_ciphertext_modulus_operands_exact`
is a deterministic arithmetic witness check for that gap. It validates exact
two-product source coverage and the four `Z_q` operand shapes, decomposes each
operand with the centered `q` lift, recomputes both negacyclic RNS products, and
compares every source limb and coefficient with the two claimed products before
target-limb conversion or scale-rounding. It rejects malformed claimed RNS
shapes before replay. The method does not add a wire layout, authorize a proof,
or substitute for a hidden-trace AIR relation.

The new registered-shape regression derives two honest products from centered
`-1` operands and accepts the complete eight-limb replay. It then changes the
last coefficient coherently across all eight limbs: one product increases by
one and the other decreases by one. Both remain within the source bound, and
the existing target-limb scale-round helper produces the same final output as
the honest pair. The new replay rejects the first forged product, the second
forged product, and their cancelling pair at the exact source coefficient. It
also rejects a late eighth-limb mismatch, a missing limb, a residue equal to
its limb modulus, and an out-of-range operand.

Validation on the current source: scoped `rustfmt --check` and `git diff
--check` passed; the focused `cargo_fast` crypto selector passed 1/1; the same
compiled test binary passed all three
`fhe_bfv::tests::registered_full_shape_source_bounds` tests. This check does
not establish BFV full-bootstrap hidden-trace soundness, eight-party private
share correctness, governed production keys, audited parameter/lattice/noise/
qROM evidence, or production resource bounds. The eight-party public verifier
and BFV production qualification still fail closed. The separate atomic
40-limb source/materialization/packing/cross-field relation belongs to MKHE,
not this eight-limb BFV source chain.
