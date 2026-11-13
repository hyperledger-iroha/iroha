# Ironwood successor runtime contract

Implementation: `crates/iroha_core/src/runtime/ironwood.rs`. This is a source-coupled
contract for the candidate, not a deployment or provisioning record. The baseline
selection and every definition in `runtime_baseline_catalog.json` remain frozen.
The separately pinned `runtime_ironwood_catalog.json` commits the successor
selection, migration descriptor and complete canonical successor descriptor list.

## Grammar and immutable identities

The original public protocol, statement, proof and limits enums remain the frozen
Exact12 wire language. Their recursive schemas and baseline instruction payload
identities remain pinned. Ironwood has three separate additive wire identities:

- `iroha.privacy.bootstrap_ironwood_pool.v1`
- `iroha.privacy.submit_ironwood_proof.v1`
- `iroha.privacy.schedule_ironwood_limits_update.v1`

The dedicated proof envelope contains a concrete Ironwood statement and proof
bytes; the limits update contains concrete action limits. Neither carries a
caller-selected Exact12 protocol tag. Successor storage uses separately named
`PrivacyRuntime*V1` models. Conversion from a frozen public value preserves its
original encoding and signed digests; conversion back refuses Ironwood. The
runtime protocol's Ironwood ordinal is 12 and its action role is 13. They do not
extend the accepted language of the original public types. Ironwood retains its
own bootstrap domain, namespace and cryptographic profile identities.

`extended_codec` names this fixed recursive extension using the canonical encoded
predecessor `SchemaV1`. Its identity hashes the domain
`iroha:runtime:ironwood:v1\0`, then each component preceded by its little-endian
u64 length. The components are `recursive-codec-extension` and the encoded
predecessor schema. The `semantic:iroha.ironwood.v1.<digest>@major.minor.flags`
codec commits these semantics and the explicit authority-registry Norito layout.
This successor storage contract includes every enclosing option, sequence, map,
executable, trigger and native multisig proposal record containing the additive
instructions. It is not a nominal alias to an evolving Rust type. Further extensions require further definitions.

The twelve replaced storage definitions own:

- The seven `world.privacy_` tables: `protocol_policies`, `pgc_accounts`,
  `pgc_pool_invariants`, `nullifiers`, `commitments`, `roots`, `root_heads`.
- The four typed trigger stores: `triggers.data`, `triggers.pipeline`,
  `triggers.time`, `triggers.by_call`.
- `world.smart_contract_state`, whose native multisig proposal, terminal and
  terminal-execution rows semantically contain instruction lists inside bytes.

Storage invariants, services reference closures, the data-event schema, instruction
interfaces and native bundle get new identities. Unaffected physical stores,
permissions, parameters, kernel, host profile and commitment profile retain their
identities. All baseline interfaces are republished because their native bundle
declares the complete storage/services closure. Original interface payload
schemas remain unchanged. The three additive interfaces join that successor bundle. This is selection-based dispatch, with no local
configuration switch or fallback decoder.

## Byte-preserving cutover and replay

The native migration `iroha.ironwood.identity-cutover.v1` is an atomic, twelve-owner
identity substitution. Existing records are already valid under the successor
grammar; no key, value, note tree, pool, nullifier, commitment or undo bytes are
rewritten. The descriptor declares exactly the predecessor and successor storage
IDs and bounded twelve-owner work. It is published as the cutover certificate,
not retained as an active migration that would require retired storage to remain
installed. There is no row-processing cursor or partially migrated state.

The first transition requires the exact published descriptor and all twelve exact
predecessor and successor owners, and the exact reviewed services profile. Partial
replacement, duplicate physical owners
and reinterpretation under the predecessor are refused. Later governed definitions may replace services or implementations while retaining
the archived Ironwood storage contracts. Recognizing an unchanged codec does not
implement another storage retirement: every other migration or invariant cutover
remains refused by this release's transition-support gate. Grammar
recognition checks these explicit published contracts for every owner; it does
not compare services or storage identities with one complete compiled selection.
After committed activation, later change sets that remove an affected owner, duplicate
it or reinterpret it under its known predecessor codec refuse at admission/approval.
The projected predecessor is checked under the same transition-support rules as
the committed predecessor. Unknown future definitions that require no unsupported
transition may be proposed under ordinary closure and obligation rules; a proposal
requiring an unimplemented storage retirement or migration is refused. A fully attested unknown
successor reaches `UnsupportedRuntime` on this release, never a locally invented
lapse. Recognition of a codec alone does not authorize execution. A future codec
extension needs an explicitly qualified new contract, not a guessed decoder.
The accumulator maps compiled storage definitions to their declared physical owners
and refuses duplicates. Its existing selection-schema fold commits the storage IDs;
row element hashing is unchanged. Tests compare incremental and cold successor roots and reconstruct
the predecessor root from the original undo under the original selection.

`activation_change_set` constructs the explicit publication, restriction,
deactivation, storage replacement and binding transitions. Existing governance
provenance, notice and **all-validator** support-attestation checks apply. No
selection changes merely because this binary is installed. The approved change
activates at its committed height; a missing validator attestation lapses it.
Constructing a change set is not approval. Existing obligations and composition
closure checks remain mandatory.

The baseline convenience constructor names the exact frozen baseline predecessor.
The merged release’s current-selection constructor also supports the exact compiled
Time predecessor, preserving its service. Both orders reach the same combined native
bundle; see `ironwood_activation_runbook.md`. Qualification must compare the complete
current and scheduled selection with the proposed predecessor. Unknown compositions,
restrictions and obligations must not be silently discarded.

Read-only preflight: export the operator-selected network's `/v1/runtime/upgrades`
response to a local file, then run (from this clone, with its selected toolchain):

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$PWD/target" cargo run --offline --jobs 6 -p iroha_core --features node,dev-tools \
  --bin ironwood_activation_preflight -- runtime-upgrades.json H
```

The command validates `selection_id_hex` against the complete selection and compares
it with the current-selection Ironwood constructor. The baseline identity remains
`11bc31dcbf492f25ce99e3f96ebdd4bd5f2d7afab68d123b147cf87e62a396d9`;
the compiled Time predecessor is also accepted by this merged release.
It validates the exported scheduled-record identities, projects every scheduled
change set due before the supplied activation height H, checks the projected
predecessor as well, and rejects another change set already due at H. Missing record
inventories refuse. Time activation must have committed governance provenance. The preflight also
includes the exact compiled definitions referenced by the reported current closed
selection. It never assumes an unused compiled successor is already
published. Authenticating the selection remains the operator's independent obligation.
A mismatch reports the unsupported projection and exits unsuccessfully; it never
submits anything or substitutes another predecessor. The explicit emission option
constructs JSON for the exact validated projected selection. This is an observation, not independent finality.
`check_activation_predecessor_at` additionally checks the exact selection cell
preimage in an independently authenticated complete World snapshot. Neither form
establishes decoder readiness, governance approval, or a stable future predecessor.
No Taira observation or preflight was performed during milestones 3 or 4.

## Typed proofs across activation

Replacing twelve storage owners changes the complete World schema commitment even
for unchanged reserve accounts, stream-token custody, provider records and aliases.
`State::native_world_schema_hash_v1()` remains the frozen installation anchor. It
must not pin the interpretation of all future proof heights. The compiled native
schema family qualifies both the baseline and the original Ironwood storage set;
installation metadata retains its existing wire shape. A foreign independently
qualified installation anchor remains a singleton.

Typed client reads choose only a candidate in their independently qualified schema
policy, then run the unchanged complete-snapshot and typed-preimage verification
against the independently selected certified block at the proof's own height. The
root commits that schema; changing the response label or pairing it with a block
from another height fails authentication. Knowing a schema is not finality and a
latest-tip schema cannot interpret historical proofs. Unknown storage schemas
require explicit release qualification. Certified component tests retain real
reserve-account and stream-token proofs on both sides of a fully attested native
cutover and reject baseline-only pins and cross-height substitutions.

## Inclusion and stored-payload boundary

Before selection of the successor, queue admission and block validation refuse
the three additive wire IDs before execution/result recording. An Ironwood
payload cannot be committed merely as a rejected transaction. Frozen Exact12
payload decoders themselves refuse Ironwood; selection checks cover dedicated
instructions inside mixed batches, proved overlays and nested trigger bodies. The bytes are not rewritten. Opaque
Custom JSON remains includable under its predecessor wire grammar; the selected
native multisig parser retains its historical nonmatch and execution rejection
for proposals containing new typed instructions. Authority, routing, sponsorship
and deferred proposal classification use that same selected parser. Stateless
routing returns the existing `Ok(None)` needs-State result before interpreting opaque
proposals, including an Ironwood proposal containing FX. Queue/Torii/gossip then use
the committed selection: before H the predecessor opaque nonmatch is preserved;
after H the proposal is routable. A stateless-only helper still cannot classify
Ironwood without State, but its error cannot bypass this admission fallback.
An IVM host without query state likewise refuses typed Ironwood instructions and
triggers. At CREATE_TRIGGER's native SCCP preflight boundary, a stateless host uses
the explicit compiled Ironwood grammar so forbidden outbound SCCP records cannot
hide inside new opaque multisig proposals (including nested proposals). A host with
query state always uses its exact committed selection, preserving predecessor
classification. Nested Custom JSON remains opaque at ordinary wire admission.
State-aware opaque IVM decoding retains predecessor VM error classes. Local decoder exhaustion
remains a retryable deferral.

Native proposal/trigger creation and durable overlay publication use the same
selected grammar. World restore checks current and retained undo of all typed
trigger stores and the three native proposal row families. Unrelated opaque byte
strings are not searched for tag-like patterns. Malformed opaque proposal rows
retain their predecessor treatment; existing semantic validators remain in force.

After activation, governance may bootstrap a **new Global Ironwood pool** using
the existing permission/asset/pool validation path. Old pools keep their namespace
and cryptographic identities. Core applies Ironwood effects only to a matching
Ironwood namespace and enforces the same committed public-balance scope checks.

## Mandatory decoder rollout gate

The chosen compatibility mechanism is **rebuild all decoders before activation**.
`wire_schema_hash` is `072924c3239057c3d6134405483512c98cf4e63cca5f6a109b45bf7578b82e8f`;
its opaque instruction roots do not bind the privacy payload registry. It moved from
`8c6e5380…` when the execution commitment's World roots became `KeyedStateRoot`
(`sumeragi.md` §16.6): LtHash roots keep their bytes, but a reader built before that change
refuses a keyed root whose marker bit is clear. IVM ABI V1
is `bdd5188fba28f664a4c672746934450ef1aa4a2e53fe3f3a5433ea0ff73dd849`.
Neither hash is a sufficient Ironwood support signal.

Before approving activation, governance qualification must record a candidate
that passes this contract and deployment evidence that **every** block/snapshot
consumer has the expanded decoders: all validators, observers, BOI Core,
Rust/Python/Kotlin/Swift SDK decoders, archive/indexer/recovery services, typed
proof consumers and operator tools. Readiness covers the dedicated Ironwood instruction and envelope codecs,
successor storage types, instruction allowlists, stored proposal and trigger codecs,
selected runtime definitions and the complete World schema interpretation. Old
Kotlin `InstructionWireIdsV1` and other allowlists must be rebuilt and qualified as
part of that separate rollout. BOI Core and mobile consumers must identify their
actual artifacts, not claim support based on the unchanged wire hash.

The evidence must name artifact/revision digests and demonstrate replay of a
pre-activation checkpoint through height H and later Ironwood blocks, historical
Orchard decoding, and typed proof reads before and after H. Base-tree observers
stop at H with `UnsupportedRuntime` because they do not implement the successor
definitions, even before the first Ironwood payment. All four validators supporting
a selection does not imply that a single off-chain consumer can read it. The qualification digest is committed in the
change set. Validator support is checked on-chain for the exact successor;
external-consumer readiness is an operational governance gate, not something
the chain can infer or enforce from an unchanged wire hash. Do not approve until
that evidence exists. Milestone 3 implements the in-tree wallet/Python and typed-proof paths; it does not
rebuild or qualify BOI Core, Kotlin/Swift, external consumers, or any deployed binary.
Activation remains blocked until this operational gate is evidenced.

The frozen baseline instruction inventory and recursive payload layouts remain
pinned. Separate additive inventory tests cover all three Ironwood wires alongside
the canonical release's other additions; dispatch tests require their handlers.
No baseline golden is replaced to accept the new schema.

## Milestone 4 operational interfaces

Compiled owner classification is indexed once by physical namespace and codec
contract; per-selection hot checks use bounded binary searches over installed IDs.
The ordinary-instruction cheap grammar check runs first. The index contains only
immutable compiled definitions and is not a node-local activation cache or toggle.

Private-settlement and SCCP-origin readers accept the qualified native schema family and
retain full certified-height authentication. `OneCallPrivateSettlementRequest::new`
now expands a known native installation anchor by default and binds that policy into its
local durable request digest. `with_world_schemas(vec![anchor])` opts into an exact pin
and preserves the original pre-family SDK request digest D0 for journal continuation.
Expanded policies bind their sorted final family to D0, independently of setter order;
foreign anchors remain exact. The portable family is measured from Core's immutable
selections and checked by `ironwood_native_schema_family_matches_portable_client_policy`.
A future schema is not accepted automatically. The SCCP CLI's existing schema flag uses
the same installation-anchor policy.

The accumulator caches compiled cutover validation by content-addressed selection ID
in a bounded 64-entry process-local cache. A cache hit changes no validity result.
Actual selection transitions still check the exact published migration certificate;
that World-dependent check is never cached across different registries. Ordinary blocks
with an unchanged selection reuse the compiled validation for all three accumulator passes.

Current-selection composition remains a structural stand-in, not Time integration.
`activation_change_set_for_selection` preserves qualified additive native services.
`service_change_set_for_selection` now takes the complete reference selection and
refuses non-service effects explicitly. `register_service_compositions` accepts
only an explicit qualified reference list (empty in this release); unsupported
references are skipped, never inferred from arbitrary native bundles.
The stand-in tests model committed Time `3be9b7af7b`, apply realistic publication
lists and pin both orders and the compiled-profile/closure predicate. They do not
execute Time. The two sibling combined constructors are incompatible; the merged
job must choose one content-addressed construction and qualify it through real
capture, output-capacity, support attestation and SDK continuation tests on all
five swapped trigger-related owners. The exact gate is in the activation runbook.
Baseline-derived proposals cannot be reused after another successor commits;
preflight includes all scheduled predecessors. New physical owner layouts or
non-additive services require a reviewed migration, not inferred composition.

See `ironwood_activation_runbook.md` for the precise governance/bootstrap sequence,
and `ironwood_boi_integration.md` for anonymous authenticated-evidence discovery, required app
calls and the currently blocked four-peer scenario qualification. Merchant receipt
verification can use an incoming viewing credential without spend authority; callers
persist consumed receipt identities and unique invoice memos themselves.

## Merged lifecycle decoder qualification

The M8 candidate also requires all selected consumers to authenticate Time V2
execution outputs and native lifecycle activation/continuation witnesses. The SDK
exact-pair table is generated from the final merged catalog and includes Time-only
and combined profiles. Its baseline and Ironwood-only behavior stays unchanged.
The same off-chain readiness evidence must cover both activation orders; the wire
and IVM hashes remain insufficient support signals. No live rollout is established
by the component tests or by the table alone.
