# F04 final-promotion interface regeneration inventory — 2026-09-24

This is a read-only inventory of the current `optimizations` source, not a
regeneration or a release qualification. The proposed immutable operation-row
origin is the actual network entry hash and index, distinct from the
deployment-history ordinal and from a block's self-referential hash. The
[operation-source audit](sorafs-final-promotion-operation-source-audit.md)
explains why the present request digest and ordinal cannot identify which of
two successful same-block Reserve envelopes allocated the row.

## Canonical Norito and schema owner

[`FinalPromotionOperationRecordV1`](../../../crates/iroha_data_model/src/sorafs/final_promotion_authority.rs)
is the public `Encode`/`Decode`, JSON and `IntoSchema` row. Its `execution` and
`reserved` fields use `FinalPromotionExecutionV1`; its `Check` subjects carry
the complete row. The current row is `#[norito(deny_unknown_fields)]`.
`FinalPromotionExecutionV1` is also used by the distinct custody record, so
placing operation-only origin coordinates in that shared struct would change
custody wire layout as well; keep the reviewed origin owner on the operation
row unless the custody protocol itself supplies a separate reason to change.
[`iroha_schema_gen::build_schemas`](../../../crates/iroha_schema_gen/src/lib.rs)
exports the row, and its
[final-promotion schema tests](../../../crates/iroha_schema_gen/src/tests/final_promotion.rs)
require the canonical nested owner and exclude runtime capabilities and
unreleased signatures. Kagami generates
[`specs/references/schema.json`](../../../specs/references/schema.json) from
that compiled source. After the row source freezes, regenerate and check:

```bash
bash scripts/tests/consistency.sh --update schema
bash scripts/tests/consistency.sh schema
```

The generator is `kagami advanced schema` inside that script. Before any
publication, add DataModel and Core tests for a complete origin hash/index
roundtrip, omitted/unknown JSON fields, truncated or old Norito frames, a
wrong same-block network index/hash, and exact signed-envelope-to-row
matching. The origin must be mandatory and captured at mutation time; do not
add an optional/default origin, an old-row decoder, a second schema, a V2
variant or a readback guess. Retain the sole V1 instruction wire ID in
[`wire_ids.rs`](../../../crates/iroha_data_model/src/isi/registry/wire_ids.rs)
and the existing action discriminants unless their one canonical layout is
intentionally revised with matching tests. The existing prohibition on a
record carrying its own `block_hash` remains distinct from an entrypoint hash.

## SDK fixture owner

The current
[`fixtures/norito_rpc/transaction_payloads.json`](../../../fixtures/norito_rpc/transaction_payloads.json)
has no final-promotion instruction sample. The
[`schema_hashes.json`](../../../fixtures/norito_rpc/schema_hashes.json)
exporter currently targets signed transactions, transaction payloads and SNS
types, not the F04 row; a passing existing fixture run therefore cannot claim
F04 parity. If the final V1 operation is exposed to SDK transaction builders,
add a valid direct signed operation to the canonical descriptor and consumer
roundtrip tests first. The
[Norito binding playbook](../../../specs/norito_binding_regen_playbook.md)
and [`generated-files.toml`](../../../generated-files.toml) require two
independent, create-only external publications and byte-identical path sets,
entry types, modes, completion manifests and file bytes. After source freeze:

```bash
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /absolute/absent/private/root-one
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /absolute/absent/private/root-two
cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify
scripts/check_norito_bindings_sync.sh
```

Apply only the reviewed identity-relative output after comparing both roots.
The one owner publishes the canonical fixture corpus and managed Java, Python
and Swift mirrors; Kotlin and JavaScript read the canonical corpus, and C#
links its descriptors and blobs into tests. No SDK-specific generator, fixture
alias or compatibility decoder should be introduced.

## OpenAPI provenance

The authored [Torii OpenAPI](../../../artifacts/openapi/torii.json) and
[`openapi.rs`](../../../crates/iroha_torii/src/openapi.rs) contain no
final-promotion route or row schema. They do not derive fields from
`specs/references/schema.json`. A row-only change must not invent an endpoint
or rewrite the authored spec. The authored spec, its `versions/current` alias
and the Torii package mirror must remain byte-identical. Its five release JSON
outputs include source-bound manifests/versions, so they need candidate-specific
regeneration after one clean frozen source commit, using the private staging
procedure in the [OpenAPI owner README](../../../tools/openapi/README.md):

```bash
bash ci/run_openapi_generator.sh --output-dir "$OPENAPI_STAGE" --unsigned-manifest
node tools/openapi/scripts/verify-openapi-versions.mjs \
  --output-dir="$OPENAPI_STAGE" --allow-unsigned
npm --prefix tools/openapi test
bash ci/check_openapi_spec.sh
```

`OPENAPI_STAGE` must be an empty owner-private absolute directory below the
required `/private/tmp` artifact root; the README gives the accompanying
artifact-root and cancellation-path setup. Unsigned output is development
metadata, not signed release evidence. The final signed artifact and source
replay require operator authorization. No generated schema, SDK fixture or
OpenAPI output was changed for this inventory.
