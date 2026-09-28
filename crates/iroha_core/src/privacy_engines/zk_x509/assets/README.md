# zk-X509 fixed compile-time assets

The fixed algebraic SHA compiler and its digest checks live in
`../fixed_algebraic_sha.rs`. Its former child-digest binary has no compile-time
consumer and is retired; the descriptor and composite digest checks operate on
the current typed schedule directly.

`rfc5280_grammar_rules_v1.bin` contains the verifier-owned closed grammar as 86
fixed-width 26-byte records. A specialized fixed-size `const fn` reconstructs
the same `[ZkX509Rfc5280GrammarRuleV1; 86]`; there is no runtime parser,
allocation, generic dispatch, or index-order change. The
`grammar_asset_preserves_every_rule_and_index` test reserializes every typed
field and pins the complete asset SHA-256.
