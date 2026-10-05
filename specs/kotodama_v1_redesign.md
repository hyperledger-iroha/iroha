# Kotodama V1 redesign implementation and acceptance

This is the execution ledger for the approved first-release syntax and usability
redesign. The canonical rules live in [the grammar](kotodama_grammar.md),
[numeric V1](kotodama_numeric_v1.md), and the IVM ABI schema/syscall sources.
Only the final ABI V1 design ships: no compatibility parser, call aliases,
edition switch, or alternative retired runtime path. Japanese branding remains
`seiyaku`/`誓約`, `kotoage`/`言挙げ`, `hajimari`/`始まり`, and `kaizen`/`改善`.

## Active implementation milestones

| Milestone | Implementation | Required closure evidence |
| --- | --- | --- |
| Declaration and value semantics | Explicit named/positional modes, named struct patterns, composable Unit, shared declaration includes, explicit local/package exports, imported value types and authenticated static error text are implemented. | Fresh compiler tests: signature-only label changes, source-order evaluation, exact per-file diagnostics, stable local/package nominal identities, unchanged schemas after error wording edits, and identical artifacts after declaration splitting. |
| Collections and arithmetic | Checked/fallible list mutation, Result obligations, fused rounding, cursor types, bounded list loops, STATE_SCAN and seek-based hosts are implemented. | Four-validator execution; compiler/VM/Core tests cover rollback, unchanged fallible mutation, no implicit Result loss, rounding modes, representable fused results with large intermediates, pagination bounds and gas. |
| Shared boundaries and rejection | Unit/error schemas, signed descriptors, exact abort propagation, explicit rejection selectors and maintained SDK record consumers are implemented. Cursor schema propagation is implemented. | State/argument/return/nested-call roundtrips and malformed identity/schema/code rejection on the final ABI; wrong-stage test failures and restoration. |
| Authoring and onboarding | Immutable editor snapshot, semantic LSP, rich diagnostics, packaged editor client and staged standalone-test validation are implemented. | Fresh editor, CLI/LSP, offline-project and updated tutorial checks; network invocation remains part of release qualification. |
| Final release assets and qualification | Final V1 artifacts and the packaged editor client are implemented. | Fresh installed-artifact admission/runtime checks, combined Core/native SDK consumers, four-validator integration, then workspace validation. |

No milestone is closed by source implementation or by a test result from an
earlier schema revision. Tests requiring node consensus use four validators and mandatory
signed RS16 availability.

## Qualification boundary

Implementation milestones do not qualify the current release candidate. Compile
and run the affected compiler, VM, Core and maintained SDK consumers against one
canonical ABI V1 artifact set, including four-validator execution, restart and
installed native artifacts. The [native publication contract](../docs/norito_bridge_release.md)
owns release source and artifact provenance requirements.

## Private sum payload addressing

`Option<T>` and `Result<T, E>` carry one heap handle through the V1 call table.
Their allocation holds the tag and the larger flattened payload; only the active
payload is written or read. Private product payloads can cross the 32 KiB signed
instruction-offset boundary: lowering computes the full address before the same
checked load/store. This does not change the 8,192-word call-table limit or the
existing public record, schema-node and schema-depth limits.

## Complete callable schemas

`EmbeddedCallableV1` carries an argument forest and one result tree, each a flat
preorder `CallSchemaV1` tape. Inline children preserve nominal products, finite
error descriptors, both Result branches, List capacity and element type, cursor
key kinds, and private numeric roles. There are no child references or erased
Sum/List roles. Table counts are derived from the schema; Unit and empty named
products occupy one zero word. Private tapes permit 250,000 nodes and depth 256;
public records retain their existing 256-node limit. Each table remains bounded
to 8,192 words, independently of a handle's checked payload allocation size.

The sole callable wire body is `CS1\0`, a fixed little-endian `u64` node
count, then complete preorder nodes with one-byte tags. Struct names and ordered
field names use canonical Norito String and Vec<String>; nominal errors retain
the complete length-prefixed canonical descriptor. Tuple arity is `u32`, List
capacity and public/cursor kinds are `u8`, and internal/private pointer types are
`u16`. Child length prefixes follow the enclosing advertised Norito flags. The
node vector has no per-node enum-field wrappers or retained sequence span plan.
Decoding charges cumulative elements and the real node Vec backing before
allocation, preserves charged canonical string/error decoding, then validates
the complete forest. The former generic-vector wire body is rejected directly.

The JavaScript artifact boundary reads this same complete tape, including
reserved query-view shapes, nominal error catalogs, cursor key kinds and private
resource restrictions. Its bounded iterative traversal keeps the private limits
separate from public-record limits and rejects the retired shallow-role layout.
Structural validation of compiler output does not replace native artifact
admission or execution-proof verification.

Validation derives subtree ends and widths without allocating, and can fill
caller-funded traversal storage. Runtime traversal charges one gas before each
visited schema node, eight before each occupied scalar or handle word, eight
for a sum tag or sixteen for a List header, followed by existing staged pointer
validation charges. It checks the full reserved aggregate footprint and exact
List capacity, then visits only the active branch or logical List elements.
Inactive payloads and unused List capacity are not interpreted as values.
Authenticated callable lookup and these complete scans still require integration
with the production execution-proof relation; native validation is not proof
qualification.

## Bounded live scan contract

`STATE_SCAN = 0x010038` consumes `r10` canonical NoritoBytes(StatePath map),
`r11` canonical NoritoBytes(StateCursorV1) or zero, and raw `r12` limit 1..64.
`r13`, `r14`, and `r15` are zero. It returns `r10` canonical
NoritoBytes(Vec<StatePath>), `r11` next cursor or zero, `r12` selected count,
and `r13` examined count. Every input/output is public under the strict pointer
ABI policy. Admission marks this as a durable-state read; proven map prefixes
produce wildcard read metadata, while unresolved accesses serialize.

A cursor binds the authoritative host instance, map name, exact key/value schema
hash, key kind and last examined canonical map path. Its canonical Norito frame
is at most 64 KiB. Every cursor node in the complete callable schema retains its
declared key kind in `CallTypeNodeV1::StateCursor(EntrypointValueKindV1)`, including
nodes below `Option`, `Result` and `List`. Argument and result validation reject
a canonical frame carrying a different key kind; `Json` keys are invalid.
The schema hash uses
`KOTODAMA_STATE_MAP_CURSOR_SCHEMA_V1\0` plus the complete canonical Norito
EmbeddedStateType::StateMap frame. Type identity and map position do not confer
authorization. Hosts validate the current declaration and read permissions on
every call. Production instance binding comes from the authenticated contract
address namespace; isolated local hosts receive a deterministic instance context.

Backing storage and overlays seek strictly after the cursor and merge in canonical
path order. Each distinct merged position consumes one candidate, including a
tombstone. The scan stops immediately after 64 candidates or N live keys; it
neither counts the map nor probes for another position. A bounded page carries
its last examined position even when its continuation will be an empty terminal
page. Deleted cursor keys remain valid. Later calls read current invocation
state and overlay; insertions before the cursor are not revisited. Values are
materialized only for selected keys, at most N, using their signed schemas.
