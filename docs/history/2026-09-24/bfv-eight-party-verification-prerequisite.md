# BFV eight-party verification prerequisite, 2026-09-24

This current-source audit found no genuine eight-party BFV transcript on which to
run a malicious-share regression. The registered profile has **eight RNS modulus
limbs**, not eight protocol participants. The existing
`eight_limb_operand_product_rejects_late_eighth_limb_mismatch` test in
`crates/iroha_crypto/src/fhe_bfv/tests/registered_full_shape_source_bounds.rs`
checks a source-chain residue bound. It does not authenticate a party or verify
a share, and the test's own evidence record says so.

The native API currently has one `BfvSecretKey { s }`, one `BfvPublicKey { b,
a }`, and a monolithic `keygen_from_seed` returning a secret key, public key,
and relinearization key. The exact-residual proof input embeds that single
secret key. `decrypt` and `decrypt_bounded_noise` also take one complete secret
key. The `BfvEvaluationKeyBundle` contains evaluation keys, but no frozen
participant roster, per-party public commitment, share or combiner. A search of
`crates/iroha_crypto/src/fhe_bfv.rs` and its BFV test modules found no BFV
participant/share transcript type or share verifier. The artifact-bound
`bfv_full_bootstrap_diagnostic_execution_v1` takes a bootstrap key, governed
artifacts, Galois keys, ciphertext and bound; it has no participant input and
confers no production qualification.

Consequently, constructing eight labels around this single-key API would not
exercise a malicious-party property. A late-limb residue mutation is useful
for the existing arithmetic bound, but cannot stand in for a wrong-party,
duplicate-share, missing-share, or participant-equivocation test. No new
eight-party test was added on that false premise.

The next implementation cut must define the exact distributed BFV operation
being qualified, freeze its roster and threshold policy, bind each share to a
participant key, session, statement, and limb/phase, verify each contribution
before deterministic combination, and specify accepted-dropout and retry
semantics. Tests can then construct eight independently generated shares and
attack a late limb, duplicate identity, cross-session replay, invalid share,
and missing participant through the real verifier. The full BFV-RNS proof
relation, independent parameter/lattice/noise/qROM evidence, full-size resource
measurements, and `require_ram_lfe_bfv_production_qualification_v1` remain
open. This note changes no production gate or wire layout.

Validation: read-only source inspection; no Cargo test was run because no
executable BFV code or test changed in this audit.
