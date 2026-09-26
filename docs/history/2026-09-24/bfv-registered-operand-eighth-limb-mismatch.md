# BFV registered operand product and eighth-limb mismatch, 2026-09-24

The registered RAM-LFE BFV source and evaluator chains both use eight RNS
limbs at degree 64. The crypto regression
`eight_limb_operand_product_rejects_late_eighth_limb_mismatch` derives two
centered `-1` operands from the registered parameters, multiplies them in the
source chain, and reconstructs the expected single nonzero product coefficient
at index 63. Both direct and target-limb product-sum admissions accept two
valid copies and return the same rounded result.

The adversary changes only the eighth source limb of the second product at
coefficient 63, keeping the complete eight-limb shape and the altered residue
strictly below its modulus. CRT reconstruction moves that product outside the
registered one-product centered bound. Both admissions reject the *right
product* at coefficient 63 before target-limb conversion or rounding can
accept it. This complements the September 23 registered source-bound test,
which mutated all limb residues coherently beyond the bound.

This is source arithmetic admission, not the full BFV-RNS proof relation.
The current source-bound APIs consume caller-supplied product residues; they do
not prove those residues came from claimed operands. The current `fhe_bfv` API
also has no party/share transcript, combiner, or malicious-share verifier, so
this test makes no eight-party qualification claim. Hidden-trace soundness,
audited parameter/lattice/noise/qROM evidence, full-size resource measurements,
and the production qualification gate remain open and fail closed.

Validation: `rustfmt --check --edition 2024
crates/iroha_crypto/src/fhe_bfv/tests/registered_full_shape_source_bounds.rs`
and edited-file whitespace checks passed. The exact offline locked selector
`CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_crypto --lib
eight_limb_operand_product_rejects_late_eighth_limb_mismatch -- --nocapture`
passed 1/1. The test is deterministic and uses no entropy or synthetic party
identities.
