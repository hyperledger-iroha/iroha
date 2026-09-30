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
| Declaration and value semantics | Explicit named/positional modes, named struct patterns, composable Unit, nominal error descriptors and imported value types are implemented. | Fresh compiler tests: signature-only label changes, source-order argument evaluation, one initializer evaluation, field reordering, exhaustive matches, exact package identities. |
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
is at most 64 KiB. The schema hash uses
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
