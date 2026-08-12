# zk-X509 fixed compile-time assets

`sha_fixed_algebraic_child_digests_v1.bin` contains the five-by-four matrix of
32-byte release-seal digests for the fixed algebraic SHA compiler. The matrix
is serialized in shape-major, segment-major, byte order and reconstructed by a
fixed-size `const fn`; there is no runtime parser, allocation, or lookup change.

The compiler descriptor, child atom counts, typed composite digest binding,
and all fail-closed checks remain in `../fixed_algebraic_sha.rs`. Existing
non-tautological coverage includes
`descriptor_shape_set_and_invalid_shape_are_fail_closed`,
`success_only_shape_cache_is_stable_and_invalid_shapes_do_not_poison_it`,
`native_query_coset_and_output_shape_negatives_fail_closed`,
`release_shape_atom_counts_and_digests_are_reported_and_bounded`, and
`composite_children_are_pinned_and_reject_order_width_and_substitution_attacks`.
`manifest.json` pins the asset and its pre-extraction Rust declaration.

`rfc5280_grammar_rules_v1.bin` contains the verifier-owned closed grammar as 86
fixed-width 26-byte records. A specialized fixed-size `const fn` reconstructs
the same `[ZkX509Rfc5280GrammarRuleV1; 86]`; there is no runtime parser,
allocation, generic dispatch, or index-order change. The
`grammar_asset_preserves_every_rule_and_index` test reserializes every typed
field and pins the complete asset SHA-256.
