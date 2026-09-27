# BFV registered eight-limb late-coefficient adversary, 2026-09-23

The registered RAM-LFE BFV centered scale-round source uses degree 64 and all
eight RNS limbs. A new crypto test constructs a canonical 64-coefficient source
polynomial with only coefficient 63 populated. It checks that the exact
one-product centered bounds `+B` and `-B` are accepted by both the direct and
target-limb product-sum paths and yield the same result. It then submits
`+(B+1)`, `-(B+1)`, an invalid right-hand product, and an invalid pair that
cancels to zero. Both paths must reject the original left or right source
coefficient at index 63 before any rounded result is accepted. This extends the
earlier degree-two
[source-bound regression](bfv-rns-source-bound-before-target-limbs.md) to the
actual registered eight-limb arithmetic shape.

The test does not implement eight-party BFV: the current `fhe_bfv` API has no
party/share transcript, combiner, or malicious-share verification to exercise.
It also does not prove that arbitrary RNS residues came from the claimed
negacyclic operands, or establish the hidden-trace AIR, parameter search,
lattice security, circuit-derived noise, qROM, or resource evidence. The
production qualification function still returns
`ProductionQualificationUnavailable` pending audited registered evidence.

Validation: `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_crypto
eight_limb_source_rejects_late_out_of_bound_and_cancelling_products --
--nocapture` passed 1/1 in the integrated checkout (2m 31s build). Source was
formatted with `rustfmt --edition 2024`, and `git diff --check` passed for the
edited tracked BFV test module.
