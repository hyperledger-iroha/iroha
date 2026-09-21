# MKHE native40 migration boundary

This is a source-coupled continuation of the
[September 20 integration evidence](../2026-09-20/first-release-integration.md).
The production native40 source and composite remain unqualified. The parameter
migration is a reviewed inventory and design; its profile switch is not applied.

## Applied packing evidence guard

[`packing.rs`](../../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/packing.rs)
now explicitly binds the existing fixed packing KAT observations to their
recorded full profile digest:
`bfdc09d79ca711cd02aa1a79bff55d6c1bea4eb5871c58b412cd403f898b082c`.
Both certificate construction and validation use this guard. Matching-profile
acceptance and rejection of changed ID, distribution, operation policy and
40-prime geometry pass in two focused tests. The existing all-KAT-axis
corruption test also passes. There are no failures or ignored cases.

The two new cases run in 0.02s and the existing axis case in 0.13s. Root executed:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib release_packing_evidence_ -- --nocapture
target/first-release-mkhe-packing-profile-guard-native/iroha_zkp_halo2-tests release_packing_certificate_binds_every_kat_axis --nocapture
```

The copied binary and identity are in
`target/first-release-mkhe-packing-profile-guard-native/`; logs are
`target/first-release-mkhe-packing-profile-guard-tests.log` and that packet's
`certificate-axis-tests.log`. The binary SHA-256 is
`e0d56bf3dfd0d97e4cf2bb090eec745d5d262c4bb3171c1cad450759fb1a178f`;
the applied `packing.rs` SHA-256 is
`ba6596596497a2b6fcf3db304c7332db8331faa250156cf50a95bc6aad7aaf73`.

This is explicit identity hardening before changing parameters. The prior
fixed schedule digest already rejected a changed profile; no present bypass was
demonstrated. The new guard prevents a later schedule-only update from relabeling
old observations. It does not establish the truth of an audit, run new packing
measurements, change parameters or admit a governed key/source.

## Required complete migration

The detailed original inventory is retained in
`target/first-release-mkhe-native40-parameter-migration-design.md` and
`target/first-release-mkhe-native40-parameter-inventory.json`: 146 files and 1,061
lexical matches, with per-file hashes. Their original design identity and
unapplied patch artifacts remain historical records; the packing guard's later
application is evidenced separately above.

The smallest coherent profile switch must address these connected owners:

1. Introduce one neutral owner of the exact ordered 40 primes/primitive roots,
   used by the release and native consumers. Retire the 38-limb production
   selection. Full `BgvProfile` identity includes distribution and operation
   ceilings, so sharing only arrays does not produce a single profile. Keep the
   stricter operation budgets and outer proof budgets as their specified scopes;
   neither can be increased to make the new source fit. Preserve the native
   proof schema/hash binding when reconciling its current profile ID.
2. Change governed roster, CPK/RKG/Galois/decryption, immutable source receipts,
   publication counts and every profile-bound identity together. Preserve their
   equations and authority requirements. The public native corpus requires
   3,520 real objects; the current 3,344-object source leaves 176 absent objects.
   Existing fixture `Csrc` points do not supply this authority.
3. Retire the declared 38-to-40 basis-extension, tail-publication and assembler
   path in `collective/incremental_source.rs`. Its descendants contain actual
   pre-transcript, numeric-origin, claimed-source and qPCS ownership consumed by
   native proof components. Move those obligations under the sole original
   full40 publication owner before deleting the obsolete prefix/tail structure.
   Do not introduce a second profile, decoder fallback or compatibility alias.
4. Rebuild source algebra from the original admitted key and all 43 ciphertext
   records over all 40 limbs. The relation is still ordinary polynomial
   multiplication followed by the exact negacyclic quotient equation,
   `T + p_l E + delta M - C = (X^N + 1) H`. There are 3,440 source equations,
   200 aggregate coordinates and 400 masked product/quotient rows. Consume the
   actual native aggregation sampler and original message/randomness/opening
   owners; do not copy the old 190-pair schedule. Keep the real 1,376-owner
   same-opening relation and sole opening inventory.
5. Regenerate actual full40 packing outputs, derive the corrected execution and
   noise schedule, and run independently reviewed security estimation for the
   exact new candidate. The existing estimator and packing certificates must
   reject the changed profile until their replacement evidence exists.

## Numerical and authority blockers

The two added modulus/root pairs have independently checked primality and root
order, and the full product has 2,400 bits. Those integer checks are not lattice
security or noise evidence. The old noise schedule yields 2,115 residual bits;
native requires 2,287. Changing 38 to 40 does not explain the difference because
both have the same ceil-log2 limb-count term. Witness widths, q-min-dependent
integer-lift bounds and per-limb soundness accounting need their actual role by
role derivations.

The canonical public corpus alone is 3,691,001,600 bytes. Its write/seal/readback
passes and later authenticated reader require separately charged I/O under the
unchanged limits; arithmetic subtotals are not measured whole-session bounds.
The ordered-storage budget tests do not close the remaining source/qPCS sharing
of one memory, spool, I/O and execution owner.

Governed full40 key/source admission, live source/context correspondence,
production Q-mask construction, complete packing/global/source bindings and
consuming composite authority remain closed or unfinished. The 75 earlier
reopen-removal controls and these three packing cases do not qualify them.
Required acceptance still includes all eight parties, every real source
coordinate, wrong-key/epoch/profile rejection, no reuse and refusal custody,
authenticated rereads, exact transcript order and whole-proof resource
measurements on the same candidate. Independent security review and release
qualification remain separate requirements.
