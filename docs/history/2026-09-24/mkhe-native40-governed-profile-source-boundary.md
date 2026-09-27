# MKHE native40 governed profile source boundary — 2026-09-24

This checkpoint uses the existing ordered 40-prime and primitive-root values; it
does not introduce or infer parameters. `rns_native_profile.rs` now owns all 40
literal pairs. The current 38-limb `RELEASE_MODULI_V1` and
`RELEASE_NEGACYCLIC_ROOTS_V1` arrays derive their ordered prefix from that one
table, so the values cannot diverge while the release cutover is unfinished.
The native `candidate_profile_v1` is the owner of the complete `BgvProfile`
identity, including profile ID, ring, plaintext and error parameters, hybrid
digits, and resource ceilings.

The native public-polynomial publisher now requires a typed source-profile
claim before reading source identity or calling the CAS publisher. The claim
constructor checks the complete `BgvProfile` digest against that native40
candidate; it rejects the existing 38-limb release profile and a 40-limb
profile with changed resource policy. The selected digest also enters the
publisher's source-stream digest, and the source must repeat the same claim
after the complete traversal. This is a profile preflight, not authentication
that live coefficients came from the claimed profile. The production source
adapter remains uninhabited, and no 3,520-object production publication exists.

The release profile still selects 38 limbs and its established readiness checks
remain closed overall. Switching `release_profile_v1` to native40 now would
leave compiled 38-limb consumers inconsistent: the streaming collective key
and 43 ciphertexts publish 3,344 rather than 3,520 objects; Phase-23 source
algebra, RNS-Link wire geometry, direct RKG, active-binding replay, and
decryption equality retain 38-limb counts. The private basis-extension/tail
assembler remains non-authorizing and is not a first-release migration path.
The frozen estimator certificate, release KATs, and resource evidence still
belong to the old profile. Those consumers and evidence must be migrated and
reviewed before the release selector can become native40. Source/profile
correspondence, the missing 176 real coefficient objects, complete composite
verification, and the qPCS work-ceiling obstruction from the
[2026-09-23 audit](../2026-09-23/mkhe-native40-source-gap-audit.md) remain open.

Focused validation on the shared working tree:

- An independent source-value comparison against the staged pre-cutover tables
  confirmed that all 38 prefix pairs and both native tail pairs are unchanged.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_zkp_halo2 full_profile_seal_rejects_release_prefix_and_changed_resource_identity --lib -- --nocapture` — 1 passed.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_zkp_halo2 rns_native_profile::tests --lib` — 14 passed.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_zkp_halo2 rns_native_public_polynomial_publisher::tests --lib` — 19 passed.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_zkp_halo2 manifest::tests --lib` — 8 passed.

These focused tests are source checks, not release qualification, a full
workspace run, or measured production resource evidence.
