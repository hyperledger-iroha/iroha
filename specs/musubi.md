---
title: Musubi V1 Kotodama Package Ecosystem
---

# Musubi V1 Kotodama Package Ecosystem

Musubi is the first-release package ecosystem for Kotodama source packages.
It provides Cargo-style manifests, workspaces, dependency resolution,
lockfiles, packaging, fetching, testing, publishing, ownership, and registry
operations while keeping every consensus-visible identity and decision
deterministic.

This document specifies V1. The earlier pre-release Musubi registry, wire
types, CLI aliases, lock formats, cache layout, and Torii upload workflow are
retired. Nodes must refuse launch when legacy Musubi registry records remain;
operators reset disposable pre-release state explicitly. No compatibility
decoder or automatic cache deletion is provided.

## Identity and namespace binding

A canonical public namespace is registered once as a
`MusubiNamespaceBindingV1`. The record binds:

- canonical namespace text;
- a stable home `DataSpaceId`;
- either dataspace-root or domain scope; and
- the namespace generation used by delegations.

Bindings are immutable. A package has the structural identity
`MusubiPackageIdV1 { home_dataspace, scope, name }`; changing an SNS or domain
owner does not change an existing package or its governance. User interfaces
render and accept `namespace/package`, resolving the namespace through the
binding registry before an identifier enters an instruction or lockfile.

Package and namespace names are canonical lowercase portable names. Global
aliases are a distinct convenience layer. Alias text is lowercase ASCII kebab
case, one through 32 bytes, and never appears as package identity in a
published manifest or lock.

## Versions and requirements

`MusubiVersionV1` stores `major`, `minor`, `patch`, and structured prerelease
identifiers. It does not store a version string. Parsing rejects:

- build metadata;
- integer overflow;
- leading zeroes in core or numeric prerelease identifiers;
- empty or non-ASCII identifiers; and
- any spelling that does not round-trip to the canonical display form.

`MusubiVersionReqV1` is a canonical comparator AST. V1 accepts Cargo-style
bare/caret, tilde, wildcard, explicit exact, and comma-separated comparator
requirements. Bare versions have caret semantics; exact versions require
`=`. Equivalent accepted input produces identical Norito bytes: sorting and
deduplication happen before encoding, and a deduplicated singleton equality
uses `Exact` rather than a second comparator-list representation. Caret and
tilde matching compares compatible core prefixes directly, so a `u64`
component at its maximum retains Cargo's conceptual upper bound instead of
becoming unbounded. Requirement strings reject surrounding whitespace across
all SDKs; ASCII space around comma-separated comparator items is discarded
before the canonical AST is built, while alternate Unicode whitespace is
rejected. A prerelease candidate is eligible only when a comparator in the
requirement names a prerelease with the same major, minor, and patch tuple.

Published normal dependencies retain version requirements. Exact selections
belong to consumer locks and to the bounded verification proof supplied for a
publication.

## Archives and releases

`MusubiArchiveCommitmentV1` binds all material needed to reproduce and verify
a package:

- canonical SoraFS root CID and chunker handle;
- ordered chunk-plan digest and PoR root;
- full uncompressed bundle payload length (the descriptor separately binds source-only bytes);
- CAR digest and size;
- semantic bundle, normalized source-tree, and typed descriptor digests; and
- bounded file and chunk counts.

`ArchiveId` is the BLAKE3-256 digest of the domain separator followed by the
canonical Norito encoding of this commitment. Every zero, substituted, or
out-of-bounds commitment is rejected before state mutation.

The bundle contains `MusubiSemanticReleaseManifestV1`, which commits to every
semantic release field except the archive storage binding. Its domain-separated
digest is carried by the staging receipt, artifact descriptor, and provider
attestations. After the bundle and archive commitment have produced an
`ArchiveId`, `MusubiReleaseManifestV1` combines that semantic manifest with the
`ArchiveId`; the immutable registry release digest covers this complete
manifest. This two-stage commitment avoids circular hashing without weakening
the binding between release content and its selected canonical archive.

Archive location records are separate from archive commitments. A location
binds an `ArchiveId` to one renewable SoraFS pin and replication order. A
release never depends on one expiring pin. A location is selectable only after
the pin is finalized and approved and the replication order has distinct,
validated provider completions satisfying registry quorum.

`MusubiReleaseManifestV1` is immutable and contains:

- the exact release id;
- Kotodama edition;
- IVM ABI version 1 and exact ABI hash;
- bounded normal dependency requirements;
- typed-interface digest;
- immutable release metadata;
- `ArchiveId`; and
- the digest of the normalized publication verification lock.

The publisher supplies a bounded exact resolution proof. Core validates that
every selected node exists at the claimed finalized registry snapshot, every
edge satisfies the published requirement, every digest and ABI binding
matches, each parent-local alias occurs exactly once, the graph is acyclic, and
the graph remains within 1,024 nodes and depth 64. Multiple versions of one
package therefore use distinct aliases at any shared parent. This proof does
not become a global resolution for consumers.

The snapshot's height, block hash, and resolver-index revision are one
authenticated tuple. Core retains an unpruned ordered checkpoint store keyed by
resolver revision. Genesis records the block-final revision; later blocks add
one activation anchor only when their final revision is new, so unchanged
blocks remain sparse and intermediate revisions created within one block are
not claimable. Publication exact-loads the claimed revision, verifies that
activation anchor against the canonical block-hash log, requires its height not
to exceed the claimed height, and requires the first higher checkpoint (when
present) to activate strictly after the claimed height. The claimed tuple and
that successor's own anchor are canonical-checked independently. Resolver rows
also canonical-check their archive-availability anchor at its own finalized
height. Comparing rows only with the current revision is not sufficient proof
that they existed at a historical snapshot.

Release content and its release digest are immutable. The following mutable
states are independent:

- reversible owner-authorized yank state;
- derived storage/quorum availability; and
- Parliament-authorized artifact takedown.

Fresh resolution excludes yanked, below-quorum, and governed-unavailable
releases. A lock may continue fetching a yanked or degraded release while at
least one valid location remains. Takedown makes the artifact unavailable.

## Package governance

The first publication may claim an absent package only when the transaction
authority owns the bound namespace or presents a generation-bound namespace
delegation. The claim creates package-scoped governance with that authority as
the first accepted owner.

Later namespace ownership changes do not grant package authority. Package
members have accepted roles. Owner is a role and at least one owner must
remain. Maintainer capabilities independently cover publish, yank, metadata,
and archive-location management. Before an archive is referenced by any
release, its registrant may establish the initial location. Once releases
reference it, that bootstrap authority ends: every archive-location mutation
requires the current archive-location capability for every referencing
package, so removal or Parliament recovery cannot leave a former registrant
with latent storage authority.

Invitations are explicit and must be accepted by the invited account. Invite,
accept, role, removal, metadata, and ownership operations use compare-and-set
governance or metadata revisions. Stale revisions fail without mutation.
Every package-governance revision advance deterministically rebases all other
still-pending, unexpired invitations to the successor revision; the invitation
being accepted or revoked becomes terminal in the same mutation. A client that
races another governance mutation must therefore retry acceptance with the new
package revision, while concurrent invitations remain independently usable.

Sora Parliament recovery consumes an enacted decision bound to the exact
action digest. Core verifies the existing enactment delay and records the
decision as consumed so it cannot be replayed. The three exceptional recovery
actions cover package ownership, alias retargeting, and artifact takedown. A
separate Parliament action may prospectively replace the registry admission
and alias-pricing policy. Ordinary owners cannot invoke any Parliament-only
action. For an artifact takedown, the authoritative height relation is
`decision.enacted_at_height < decision.execute_after_height <=
consumption.consumed_at_height == takedown.applied_at_height ==
event.finalized_height`; the persisted takedown never relabels the earlier
Parliament enactment height as the later application height.

## Permanent global aliases

Alias registration requires:

- an unregistered canonical alias;
- authority as an accepted package owner;
- at least one active release;
- the exact expected pricing-policy revision; and
- atomic payment in XOR.

Genesis prices, in whole XOR, are 1,000 for length one, 200 for length two, 40
for length three, 8 for length four, and 1 for lengths five through 32.
Parliament may change prices for future registrations only. An alias is never
recycled. Normal package governance cannot retarget it. A Parliament recovery
retarget appends immutable history and preserves every prior target.

## Registry state and queries

Authoritative namespace, package, release, metadata, member, and invite
records live in typed ordered `mv::Storage` keyed by their stable home
dataspace identity. The universal dataspace contains only compact resolver
rows, archive/location records, reverse references, the public directory, and
the alias registry.

Publication, yank changes, metadata projection, and availability changes
update home and universal records atomically through Native AMX. No Musubi
state is encoded into `smart_contract_state`, stored in a global vector, or
recovered by scanning unrelated state. Exact resolution reads only the
universal sparse index.

The universal `musubi_replication_shortfall_releases` cell is the exact sum of
archive reverse-reference lengths for every archive whose authoritative
availability is not `Selectable`. It therefore deliberately includes yanked
and Parliament-taken-down releases while they remain bound to a non-selectable
archive; this is a storage-risk aggregate, not a fresh-candidate count. Checked
deltas apply only when an archive crosses the selectable boundary, snapshot
validation recomputes the invariant while validating reverse references, and
the process gauge is seeded directly from the persisted cell and synchronized
only after the world-state commit succeeds.

A source-level routing regression pins release publication to the universal
coordinator with the package home dataspace as its exact participant. Snapshot
validation fault-cut regressions reject one-sided release, reverse-reference,
resolver-row, and public-directory projections while accepting exact replay. A
four-peer below-quorum queue-journal crash/restart smoke is defined to queue a
structurally valid publication for an unavailable archive, restart every peer,
require one canonical finalized-replication-quorum rejection, and assert that
the package, release, resolver row, public-directory row, and archive reverse
reference remain absent on every peer at one finalized snapshot. The smoke
controls the durable queue boundary only; it does not pause a publication after
PrepareQC, after CommitQC, or immediately before world commit. The non-shipping
adversarial-test daemon now has source-bound, one-shot abort hooks at those
three exact Core insertion points. Each hook fsyncs and reads back a canonical
Norito acknowledgement before aborting, and the acknowledgement suppresses an
identical cut after restart. The integration gate now drives a selectable
three-replica publication through every hook without synthetic storage state:
each cut receives a fresh four-peer DA/RBC network, a canonical paid pin,
three admitted providers and governed completion authorities, one canonical
three-assignment replication order, three completions bound to a real finalized
chain anchor, and provider-owner signatures over the exact parsed bundle. The
PrepareQC and CommitQC cuts target the deterministic autonomous universal-lane
author derived from the durable lane frontier; the other three peers must keep
the release absent until that sole author restarts. The pre-world-commit cut
instead requires the other three peers to finalize while the target is down.
After restart, a finalized barrier requires every peer to expose the same
complete package, release, resolver, directory, location, and retention tuple
with exactly one successful publication occurrence. The gate has source
implementation coverage but still requires an execution receipt once the
unrelated workspace compilation failures are cleared.

Snapshot loading reserves the complete generic `musubi` state-path namespace:
the bare name and `_`, `/`, `.`, or `:` descendants are rejected as legacy
pre-release state. This prevents an unenumerated retired record shape from
bypassing the reset check.

V1 exposes twelve typed query families: exact package, exact release, exact
provider-bundle attestation, resolver index, versions, maintainers, archive
locations, archive retention, alias, alias history, ordered prefix, and search.
Resolver pages default to 50 entries. A continuation cursor binds finalized
height and hash, the canonical query hash, the last returned key, index
revision, and caller when authorization affects output. A changed anchor,
query, revision, caller, or boundary is an explicit stale-cursor error.

Finalized cursor keys are bounded by their canonical producer, not by a legacy
fixed text guess. V1 permits up to 1,102 bytes for a maximal structured SemVer,
320 for an ordered package selector, 129 for an archive/location pair, and 53
for an alias-history key. Maintainer keys retain their parity-stable
`hex(account)|accepted` or `hex(account)|pending-<invite>` form; the bounded
8 KiB canonical account identity gives the shared conservative cursor ceiling
of 16,457 UTF-8 bytes. Rust, Kotlin, Java, Swift, and the OpenAPI source use the
same ceiling. Ordered-prefix input remains independently bounded to the
canonical 320-byte selector maximum. A maintainer cursor's raw account payload
is an exact equality token, not a sortable byte encoding: clients decode and
re-encode its canonical lowercase-hex `AccountId` payload and reject the
boundary if it recurs anywhere in the response, while page items themselves
remain ordered by the structural `AccountId` and invitation key.

Every resolver-index, version, maintainer, alias-history, ordered-prefix, and
search page echoes the exact bounded request object that produced it, including
its structural selector or terms and page controls. This response context is
required even for an empty first page, so clients compare canonical fields
directly instead of attempting to reproduce a Norito query hash or infer an
identity from result rows. Page validation requires every row to match the
echoed package, requirement, alias, or complete prefix; enforces the requested
effective limit; and binds a continuation cursor to the exact final row. A
resolver-index page may stop before its requested item limit when another
canonical Norito JSON row would exceed the 24 MiB resolver-items budget; its
cursor binds that nonempty short page and the next request resumes strictly
after its final structured version. Every other page family permits a next
cursor only on an exactly full page. Version and resolver cursor advancement
uses structured SemVer, including prereleases, rather than lexical version
text. Ordered-prefix text is canonical `namespace/package-prefix` syntax and
remains present when no package matches. Search echoes the original bounded
query text, preserving first-page identity before a search cursor exists.

Cache pruning uses a separate exact archive-retention query, bounded to 100
sorted, distinct `ArchiveId` values. The first response establishes the chain,
genesis hash, and finalized registry snapshot; every later request carries that
snapshot as `expected_snapshot`, and the client rejects any deployment, anchor,
or response-identity mismatch before changing the cache. The response also
carries the consensus-committed creation time of that exact finalized block;
publication may use it only with an exact `RetainUnknown` decision and never as
a local-clock substitute. The query performs
only direct archive, reverse-reference, release, and availability lookups. An
unknown archive is retained fail-closed because the cache is not chain-scoped.
A governance-available active or yanked release retains its archive regardless
of replication health. Only a registered archive with no release references,
or one whose every reference has an enacted Parliament takedown, receives an
explicit prune disposition.

Description and keyword search is a rebuildable projection of finalized
package claims and metadata-change events. Startup and lag recovery rebuild it
from one finalized state view; steady state applies only finalized events. The
bounded query is normalized into sorted exact Unicode-lowercase tokens, with
ASCII hyphenated terms also contributing their component words. Results are
ordered by structural package ID and use a search-specific cursor bound to the
finalized height/hash, canonical query hash, last package, and process-local
projection revision. Rebuilds therefore make old search cursors explicitly
stale. Projection anchors never regress; multiple search events in one block
must carry the same finalized hash and each distinct change advances the local
revision. Prefix, edit-distance, fuzzy, or partial matches are not performed, and
neither search results nor projection revisions ever affect resolution.

## SDK mutation framing

Typed SDK mutation builders encode the concrete Rust instruction payload with
the default `COMPACT_LEN` layout and frame it under the exact registered Rust
type-name schema. A transaction embeds that concrete frame in the dynamic
`InstructionBox` pair `(wire_id: String, payload: Vec<u8>)`; it does not add an
outer `InstructionBox` header inside the executable. APIs that return a
standalone `InstructionBox` frame use the exact tuple schema name
`(alloc::string::String, alloc::vec::Vec<u8>)`.

The Rust-owned fixture at `fixtures/musubi/instructions_v1.json` locks the
semantic value, wire id, schema name/hash, bare concrete payload, concrete
frame, inline pair, and standalone frame. Its nineteen cases cover namespace
registration; maintainer invitation, acceptance, revocation, role replacement,
and removal; global-alias registration; exact release-digest assertion;
archive registration, immutable provider-bundle-attestation registration, and
location addition or renewal; archive-location retirement; release publication
and reversible yank state; package metadata replacement; and Parliament-enacted package ownership recovery,
permanent-alias retargeting, artifact takedown, and registry-policy replacement
across Rust, Kotlin, Java, and Swift. An already-framed generic instruction
wrapper is not typed field-to-Norito support.

Swift additionally embeds all nineteen fixture cases in one real signed batch
and extracts each inline pair at the executable boundary. That regression locks
the compact `ChainId` and `TransactionSignature` tuple wrappers while retaining
fixed-width sequence and byte-vector counts required by Norito V1.

## `Musubi.toml` and workspaces

Every manifest declares `manifest-version = 1`. Unknown or duplicate fields
are errors at every nesting level. A package manifest supports package
metadata, a configurable library directory, explicit exports, optional local
contract targets, tests, readme, license, repository, keywords, and positive
include additions.

A declared test target may name one `.ko` file or a directory. Directory
targets expand to a bytewise-sorted, portable set of direct `.ko` roots under
the package, bounded by the V1 file/source limits. The runner reads each stable
regular source once and passes its text through the structured compiler API;
it never reopens the diagnostic path or discovers ambient siblings. Symlinks,
reparse points, hardlinks, special files, portable-name collisions, sensitive
paths, and generated/VCS/config roots follow the same fail-closed positive-set
policy as packaging.

A workspace root may be virtual or also contain a package. It supports
portable member, default-member, and exclude paths plus
`[workspace.package]` and `[workspace.dependencies]`. A workspace has exactly
one root `Musubi.lock`. Commands discover the nearest ancestor manifest and
then the owning workspace.

V1 dependency kinds are registry, path, and development. A dependency may be
renamed and may inherit with `{ workspace = true }`. V1 rejects git, optional,
feature, build, and target-specific dependencies.

A path dependency points to a validated local package manifest. A publishable
normal path dependency also declares its canonical registry package and
version requirement. Packaging removes the path, resolves the registry
release, and compiler-checks again from the clean packaged tree. Development
dependencies apply only to selected workspace roots and never propagate.

Clean publication validation reads only the immutable `PackagePlan`: it
recomputes the typed interface of every authenticated exact registry node,
validates the library graph, and compiler-checks every declared contract and
test root from the packaged bytes. It never reopens a workspace path. Tests are
compiled in test mode but are not executed during packaging, and their sources
do not affect the library interface digest. Because the V1 verification lock
contains only the release's normal dependency graph, a packaged test that
requires a development-only import fails with an explicit non-propagation
diagnostic instead of consulting local development state.

Focused manifest edits preserve comments and unrelated formatting. Paths are
portable, relative to their defining root, and cannot escape that root.

## `Musubi.lock`

The only accepted schema begins with:

```toml
schema = "musubi-lock"
version = 1
```

It records chain/genesis identity and the finalized registry height/hash and
index revision at which the graph last changed. Nodes contain exact structural
package id and version, immutable release digest, `ArchiveId`, source and
interface digests, and ABI binding. Parent-local edges contain the import
alias, child node, and dependency kind. Aliases are unique within each parent;
multiple versions of one package remain valid when they use distinct aliases.

Unsigned integer identity and snapshot fields are decimal strings encoded as
`"0"` or an ASCII digit sequence whose first digit is non-zero; signs and
alternate spellings are rejected rather than normalized while parsing.

The lock contains no cache paths, source plans, timestamps, credentials,
provider URLs, bearer tokens, or process-local data. Parsing any pre-release
lock format returns a regenerate instruction. `--locked` never rewrites an
invalid or stale lock.

Lock writes use a same-directory private temporary file, flush and fsync the
file, atomically rename, and fsync the parent directory. The previous complete
lock remains visible after every failed write phase.

## Resolver

Resolution is deterministic backtracking over ordered registry snapshots. It
uses this preference order:

1. an already-selected compatible version;
2. a still-valid selection preserved from the lock; and
3. remaining candidates in descending SemVer order.

Input and query ordering cannot affect the result. Parallel versions are
allowed when constraints cannot converge. The resolver backtracks across
parent candidates, detects dependency cycles, and reports the minimal stable
conflict chain.

Every still-valid locked selection remains fixed unless explicitly targeted,
including a yanked locked release. `update -p PACKAGE[@VERSION]` unlocks the
target and only nodes forced to change; `--precise VERSION` adds the exact
target constraint to every prior parent-local occurrence of that locked node.
The constraint remains binding when satisfying it forces backtracking to a
different parent release; an unrelated parallel occurrence cannot satisfy it,
and it never degrades into a candidate preference. Terminal enforcement replays
the prior target-bearing structure over the completed graph, including exact
old parents reached through renamed incoming edges and descendants of an
already-selected replacement. A prior occurrence unreachable from every
selected workspace root makes the targeted request invalid. Precise conflicts
name the minimal selected/current root-to-terminal branch rather than an
unselected prior parent. Fresh candidates exclude yanked, takedown, and
below-quorum rows.

## Command contract

The first-release command groups are:

- Project: `new`, `init`, `add`, `remove`, `metadata`, `tree`.
- Build: `fetch`, `check`, `build`, `test`, `package`.
- Registry: `publish`, `search`, `info`, `versions`, `yank`, `unyank`.
- Governance: `owner invite|accept|list|set-role|remove` and
  `alias register|resolve|info|history`.
- Maintenance: `update` and `cache verify|repair|prune`.

The retired `install`, `pack`, short-alias `set`, cache `import`, and public
Torii upload workflows are not aliases.

`fetch`, `check`, `build`, and `test` resolve and atomically update the lock
when permitted, then fetch missing archives. `--locked` forbids graph changes,
`--offline` uses only cached index and archives, and `--frozen` combines both.
Workspace selection follows default members, `--workspace`, `--exclude`, and
`-p`.

Offline resolver snapshots are canonical, bounded, and committed under a
domain-separated digest after all captured pages agree on one chain, genesis,
finalized block, and index revision. Within one deployment, cached index
revisions must be nondecreasing as finalized height advances; a higher block
with a lower revision is rejected and can never satisfy a lock freshness check.
At the same finalized height and block hash, lock compatibility requires the
exact same index revision. Until finalized query responses carry portable
consensus inclusion proofs, the cache remains rooted in the validated online
read and private cache identity.

Local and read-only commands do not load a signer. Mutation credentials come
only from explicit or platform Iroha configuration. Secrets and stream tokens
are rejected on argv and are never persisted. Human output has deterministic
stdout/stderr separation; JSON output is one document with a stable schema and
error code.

Rust, Kotlin, Java, and Swift public-query clients apply a Musubi-specific
32 MiB response ceiling. An exact-release JSON response repeats the bounded
dependency vector in the authoritative home record and the universal resolver
row, so a consensus-legal publication admitted by the default 10 MiB
transaction corridor can exceed the retired 8 MiB client limit. The 32 MiB
ceiling retains headroom for the escape-heavy full-256-dependency exact-release
fixture while remaining below the shared Rust HTTP client's 64 MiB default.
Resolver-index production separately caps the canonical JSON array payload at
24 MiB, validates the complete serialized page against the 32 MiB ceiling, and
uses the final returned version as a continuation when byte budgeting produces
a short page.

The JSON envelope is `schema = "musubi-cli-output"`, `version = 1`, and carries
one command result. Publish success data is a discriminated union. A detached
result has `status = "detached"`, the operation id, `phase = "seed-ingress"`,
the canonical namespaced `namespace/package@version` release label, and its
separately named `structural_release`. A completed result has
`status = "complete"` and retains both release forms, then exposes the operation
id, chain and genesis identity, finalized snapshot, immutable release digest,
`ArchiveId`, both canonical projection digests, checkpoint digest, and the AMX
instruction digest, payload transaction hash, and applied height. Digests and
hashes are lowercase fixed-width hexadecimal. Heights and revisions are exact
unsigned JSON integers. The complete result is identical for an ordinary
publish and a completed resume.

The completed JSON receipt is the compact local checkpoint projection; it does
not copy the potentially large home-dataspace or universal-index records to
stdout. Its checkpoint digest is a commitment to the exact response already
validated by the publisher, not an authenticated finalized-query receipt or an
independent chain proof. `status = "complete"` proves the immutable publication
claim completed, but does not assert that a later yank, governed takedown, or
replica degradation left the release fresh-selectable. Runtime configuration,
credentials, authorization material, service endpoints, journal paths, and CAR
paths are absent from both success variants.

## Packaging and cache

Packaging starts from a positive set: canonicalized `Musubi.toml`, a generated
verification lock, declared library/contract/test roots, declared readme and
license files, and explicit include additions. It never starts by recursively
including the project and subtracting a denylist.

Generated, VCS, and configuration roots are always excluded. Packaging rejects
symlinks, hardlinks, special files, non-UTF-8 paths, traversal, portable
reserved names, Unicode/case collisions, and known credential/private-key
paths or contents. SoraFS CAR portable-path and chunk-plan validation applies
before commitments are calculated.

The package reader retains and revalidates the opened final-file identity, so a
leaf replacement cannot change the bytes admitted to the immutable plan.
Ancestor-directory confinement still uses path-based checks before and after
that open, however. A deliberately timed ancestor ABA replacement therefore
remains a release gate until package traversal is handle-relative and enforces
no-follow/open-beneath semantics for every component on each supported host.

The canonical bundle contains the semantic release manifest, typed artifact
descriptor, normalized source tree, and verification lock. Provider validation
attests successful parsing and verification of that bundle, not storage of an
opaque byte string. The shared verifier rejects a lock that contains the root,
an unreachable node, or a propagated development edge, and requires every
published direct dependency to match one normal root edge by alias, package,
requirement, and exact selected node. Semantic manifests, exact proof nodes,
resolver rows, and consumer lock roots all reject duplicate parent-local
aliases instead of depending on map overwrite behavior.

Authenticated fetches reject oversized JSON nesting, token inventories,
strings, and unquoted scalar literals before constructing a Norito JSON DOM.
On an online cache miss, an exact locked fetch also binds the finalized
archive-location page to the lock's chain id, genesis hash, and snapshot floor
before contacting a provider. Equal finalized heights require the exact lock
snapshot; later heights must not regress the resolver-index revision. A valid
global content-addressed cache hit remains registry-independent because the
exact bundle is revalidated against every locked node that consumes it.
Online resolution anchors the selected `client.toml` path once, then reads one
bounded, singly linked image through a nonblocking, no-follow stable descriptor
and parses both the public registry context and the secret-free fetch
configuration from those exact bytes. Provider token files, DNS answers, and
HTTP clients are loaded only after an immutable-cache miss; the raw
configuration image is not retained in the graph.
Fresh publication retains only process-local, nonserializable provenance for
that anchored image. After clean-package validation it securely rereads the
file, compares a domain-separated digest before parsing any signer or mutation
runtime fields, and constructs the registry reader, signer, and publication
services from the matching bytes. Resume has no earlier resolver image to bind
and constructs those three boundaries from one current secure read; neither
path persists or reports configuration bytes, paths, or digests.
The canonical CAR bridge has a four-frame queue, accounts the consumer-owned
frame and a producer frame blocked on that queue, and closes and joins its
worker on success, error, or reader drop. Cache admission separately bounds
retained plan allocation and SoraFS ingest estimates. These are deterministic
logical-allocation controls; deployment-equivalent HTTP/TLS, JSON-DOM, cache,
allocator, thread-stack, and operating-system RSS qualification remains a
release gate.

The user cache path is derived only from the trusted root and archive id:

```text
registry-v1/<archive-id>/src
```

Extraction streams into a private sibling with no-follow/create-new file
creation, verifies every commitment, fsyncs files and directories, then
renames into an absent immutable destination. Repair validates the finalized
commitment and file plan before classifying any local entry as corrupt, and
quarantines only structurally validated descendants; invalid registry inputs
leave the cache untouched. Lock-controlled deletion, arbitrary replacement,
and cache import do not exist.

On Windows, cache roots, archive directories, source-tree ancestors, and
regular files are protected by stable file identities and retained
non-delete-sharing handles, so existing entries can be read and verified
without admitting path substitution. Publication, quarantine, and prune remain
fail-closed on Windows: safe Rust currently exposes no handle-relative,
no-replace directory rename, and dropping the pin to use a path rename would
reopen the substitution race. Those mutation paths may be enabled only after a
safe workspace-owned primitive preserves the retained-handle invariant.

Unix install has a subprocess abrupt-exit matrix at the payload write and
sync, source chunk and file sync, payload removal, source-tree verification and
sync, directory publication, and archive-directory sync boundaries. Reopen
must either find no published `src` or fully reverify it, and retry must
converge. This is process-crash evidence, not yet the complete power-loss,
disk-full, or crash-at-every-write campaign. Unix mutation also retains one
release blocker: replace its advisory-lock plus path-based `rename` with a safe
descriptor-relative no-replace directory primitive. A second path absence
check does not close the same-UID destination-planting race.

`musubi cache prune` inventories canonical cache identities, obtains the
bounded finalized decisions above, and passes only explicit prune identities to
the point-targeted cache deletion primitive. It never treats absence from a
workspace lock or retained set as deletion authority, so an archive installed
concurrently after inventory cannot be removed accidentally. All batches are
validated before the first deletion. `--dry-run` reports the same decisions and
candidates without renaming or deleting any cache path.

## Publication state machine

Production publication is resumable and idempotent:

1. Validate and compiler-check the clean package and exact proof graph.
2. Stage the CAR through admitted authenticated SoraFS seed ingress and obtain
   a signed expiring receipt bound to chain, publisher, broker/provider,
   manifest, archive, body digest and length, nonce, and expiry.
3. Register or reuse the exact archive and permanent registry-grade pin.
4. Wait for finalized approval and distinct provider validations/completions;
   each provider registers its one signed parsed-bundle attestation under the
   immutable `(archive, replication order, provider)` key.
5. Add the renewable location using the sorted provider identities and one
   archive/order-bound aggregate digest of those finalized attestations,
   require three healthy replicas, and read back through two distinct providers.
6. Submit the package claim and immutable release through Native AMX.
7. Wait for finality and verify the exact universal resolver row.

The client and publication engine share the fixed 30-second service-clock lead
policy for a newly issued seed receipt. A correctly signed receipt issued ahead
within that bound is journaled, but archive-registration preparation remains
`Pending` without rewriting the journal until the local trusted clock reaches
its issue time. A receipt beyond the bound is rejected before persistence;
only a receipt whose inclusive validity window has elapsed is discarded and
restaged.

The publication proof resolver may retain an existing lock edge only while its
row remains fresh-selectable; yanked, below-quorum, unavailable, or governed
takedown rows require a new proof graph (and make `--locked` fail). The proof's
snapshot must be a canonical finalized ancestor on the current chain, and its
index revision must match the sparse finalized checkpoint for that ancestor.
This permits replication and readback blocks to finalize without invalidating
the operation. Core still revalidates every exact proof row, its nested storage
and yank anchors, and fresh-selection state against current authoritative
registry state when it executes the release claim.

The operation journal contains no secrets and is safe to resume. `publish
--detach` may return the operation id; ordinary `publish` succeeds only after
step seven. Retrying identical commitments is idempotent. Reusing a package
version with different commitments is permanently rejected.
Archive-registration evidence must retain the exact staged receipt from that
operation, including its nonce; another independently valid broker receipt
cannot cross the registration boundary.
Phase three persists a canonical registration intent containing the exact
instruction digest, receipt, fee-quoted signed transaction, and transaction
hash before registry submission. Recovery verifies the returned and status
hashes, requires authoritative applied-height evidence, queries a finalized
archive page covering that height, and persists the authoritative record before
pin coordination begins. The journal retains at most eight contiguous,
append-only attempts. Pending, absent, or transport-unknown transactions are
never replaced. An attempt becomes replaceable only with a finalized
`RetainUnknown` archive decision and either authoritative `Expired` status or a
consensus-committed finalized block time strictly after the exact signed
transaction/receipt validity deadline. A cache-only expiry observation is not
terminal evidence, and an authoritative generic rejection remains permanent.
Pin coordination authenticates the finalized transaction hash, chain/genesis,
snapshot, immutable archive-registration projection, and verification-lock
digest rather than a new live seed receipt, and it rejects fewer than three
finalized parsed-bundle attestations. Each proof is an
immutable universal record keyed by archive, replication order, and provider;
the record retains the complete signed attestation and a domain-separated
digest, while location records and public pages retain only the sorted provider
identities and aggregate attestation-set digest. Core exact-reads and revalidates
every provider record, then recomputes the archive/order-bound set digest against
the complete finalized SoraFS completion set before accepting a location Add.
The projection contains the archive id,
commitment, original staging receipt, registrant, and registration height but
deliberately excludes the renewable location revision and location identities.
The coordinator independently retrieves the exact transaction, proves that its
sole instruction is the matching archive registration finalized by the named
snapshot, then may reproduce the projection from any later finalized exact
archive read; its response separately carries the current mutable archive
record used for location CAS.

Archive locations use a separate append-only journal of at most eight one-based
generations. Before submitting a location Add, the publisher persists the
complete finalized preparation page, current location-set CAS revision,
never-before-used stable location ID, compact provider-attestation set digest,
exact instruction digest, fee-quoted signed transaction, and transaction hash.
Before that compact Add is prepared, a compact immutable set descriptor and one
exact signed registration transaction per provider are installed as
operation/generation-bound, no-replace sidecars. Recovery exact-queries the
finalized audit record first and otherwise replays only the stored transaction;
coordinator-set, instruction, signature, or sidecar substitution fails closed.
The main operation journal retains only a domain-separated hash of the complete
set sidecar and an append-only identity/hash anchor for each expected transaction
sidecar. Installing a sidecar and durably appending its main-journal anchor are
separate advances; no provider registration may be submitted in the same
advance that first exposes its anchor. Once anchored, a missing, reordered,
substituted, or partially reproduced sidecar is a permanent integrity failure
and is never recreated from coordinator input. A finalized rejection may append
another bounded signed attempt only when its non-zero rejection height is
covered by a complete finalized archive page whose location-set revision
strictly exceeds the rejected compare-and-set revision.
The private
storage-coordination replay journal uses the generation as part of its durable
idempotency key, and every request carries the sorted IDs of prior generations;
the coordinator must not return one of them. A later generation is accepted
only after the prior generation has immutable terminal evidence, and its
preparation page must equal or advance that terminal finalized state without
resurrecting any prior identity.

Provider readback replay keys are independently domain-separated hashes of the
exact `(location_id, location_revision, provider_id)` tuple. Replacing a
location or renewing the same stable location therefore gets a new bounded
durable result, while changing any other request field under an existing tuple
is an idempotency conflict. The journal retains a fixed protocol-sized history
large enough for two distinct readbacks in every one of the eight publication
location generations; restart preserves both cached results and conflicts.

An identical active-location Add is a validated no-op before the consumed
location-set CAS revision, so a lost response recovers the exact journaled
transaction rather than building another mutation. For an absent target, the
publisher obtains the current CAS revision from a complete finalized location
page rather than trusting the coordinator's cached revision. A finalized CAS
rejection may rotate only when the covering complete page also proves a later
revision and absence. An applied transaction may rotate only when its applied
height is covered and a still-later revision proves that the identity was
retired. Expiry likewise requires authoritative exact-transaction status and
finalized absence. Unknown, pending, lagging, or coordinator-only claims never
rotate, and a same-ID content conflict remains permanent.

After a generation is finalized active, every replication poll, provider
readback, and release submission rechecks the complete finalized directory. A
later height and revision that omit the stable ID append exact retirement
evidence, clear replication and readback checkpoints, and return to location
coordination for the next bounded generation. Release rejection gets one typed
post-rejection location check; only exact retirement evidence permits rotation.
The journal retains the complete healthy directory page as the replication
floor, not only its target location record. A finalized page may not regress
height, resolver index, archive revision, or the active location revision.
Equal finalized heights require the exact same snapshot; an equal snapshot or
equal archive revision requires the exact same archive directory. Retirement
must be a later page state with an archive revision beyond the complete
journaled floor. The first applied target revision (`expected + 1`) must exactly
reproduce the signed intent fields and application height. A lower healthy or
retirement page observed after a later one was journaled is a retryable stale
poll and never overwrites the journal; the exact journaled record or a higher
revision at a non-regressing page and location height may resume.
A healthy same-ID renewal resumes from its current finalized pin, order, epochs,
provider identities, and attestation-set digest. Core resolves their immutable records and
requires the full proofs to still bind the exact chain, archive, bundle, source,
semantic manifest, verification lock, and replication order. Production
qualification still must exercise the real fee-quote and
submission transport at every location-generation crash boundary.

Before a release transaction may be sent, the publisher must durably append a
compact exact-signed intent. The envelope retains every non-derived payload and
authorization field (creation time, non-zero lifetime, nonce, fee intent,
primary signature, and the optional canonical multisig proof) and reconstructs
the chain, publisher, sole `PublishMusubiReleaseV1` instruction, empty metadata,
and absent attachments from the immutable operation request. The intent binds
both the payload-only transaction hash and a domain-separated digest of the
complete fixed-V1 signed wire. The registry's authoritative status identifies
the payload transaction hash; the authorization-inclusive wire digest is a
local durable replay binding, not consensus evidence that one particular
signature bundle was committed. Status recovery must reconstruct this exact
local transaction, query authoritative payload status before any send, and
resubmit the same bytes while it is live; it must never sign a replacement for
an absent, pending, or transport-unknown attempt.

Release attempts are one-based, append-only, and bounded to eight. A replacement
intent requires a durable terminal outcome plus synchronized finalized evidence
that the exact package version is absent from an empty universal resolver page
and that the archive retention response uses the same snapshot and consensus
time. Expiry needs authoritative expired status or consensus time strictly past
the signature-bound deadline. Rejection additionally requires a covering
higher healthy revision of the same location, or finalized retirement followed
by a later location generation. An identical already-published release is not
proof that this payload applied. Without `Applied` status for the journaled
payload hash, the attempt never synthesizes application evidence. When status
is `Absent`, a finalized same-publisher release whose complete commitments
match may still take Core's idempotent path by replaying the journaled bytes;
a different publisher or any commitment conflict is permanent. Before that
absent transaction is sent or replayed, a fresh finalized replication query
must reproduce the selected location record byte-for-byte from the signed
readback floor. A stale page, retirement, or same-ID renewal therefore leaves
the exact intent durable and unsent. A stale `Pending` status cannot outlive the
signature-bound deadline once synchronized finalized absence and consensus
time prove terminal expiry.

The current 16 MiB journal enforces derived, disjoint budgets for non-release
state, eight release attempts, and final submission plus a compact completion
checkpoint. The full paired home/universal response is bounded and validated at
finality but is not copied into the durable frame. Completion instead retains
the request-derived operation id, chain and genesis identity, the covering
snapshot, structural release and archive identity, the complete immutable
release-manifest digest,
domain-separated canonical digests of both verified projections, and a
self-digest over the complete checkpoint. The operation id binds the public
anti-replay nonce and all other immutable request fields. The checkpoint is
append-only once persisted. A later paired yank,
unyank, governed takedown, or below-quorum storage projection may therefore
complete verification of the already-applied immutable release claim; it is not
misrepresented as proof that the release remains fresh-selectable.

The compact checkpoint is a local commitment to the full response verified at
creation time; its public self-digest is not an authenticated finalized-query
receipt. A completed resume therefore trusts the private journal's protected
storage boundary. Deployments must place that journal below a trusted,
non-replaceable ancestor with rollback-resistant storage or a sealed monotonic
head. A deployment that admits external journal replacement must instead retain
signed finalized-query evidence or requery and rebuild the checkpoint before
returning success.
Complete operational-shape location pages are retained directly. A fixed-bound
audit found that the former 64-provider, 64-approval location shape is not a
consensus-legal V1 maximum: with an 8 KiB canonical account bound and the
3,309-byte maximum supported approval payload, one `AddMusubiArchiveLocationV1`
can approach 28.8 MiB and a four-location response can approach 115.6 MiB,
already exceeding the 10 MiB transaction and 16 MiB block-body corridors.
The V1 wire schema therefore registers each at-most-1-MiB provider proof in a
separate transaction and stores only the provider list plus aggregate set digest
in the location. Core requires every proof registration to finalize in an
earlier block and exposes one exact audit query. This makes the location
instruction and four-location page compact independently of approval geometry;
the production release prepare/status/submit path now uses the compact durable
intent and reconstructs the exact Torii request body for status-first replay.
Descriptor-relative filesystem qualification of the publisher sidecars, real
fee-quote/submission transport exercises, crash/fault injection, and
rollback-resistant checkpoint storage remain release gates.

Final verification retains the resolver page's chain and genesis identities in
the operation evidence and requires both to match the immutable publication
request. It also journals the Native AMX transaction's authoritative applied
height and requires the final snapshot to cover it. An exact-looking release
row returned by another chain incarnation, or from before that application, is
rejected even when its package, version, and content digests happen to match. A
row revision may precede the page's current global index revision when an
unrelated later mutation did not rewrite that exact row.

Core plans and validates the successor package-governance revision, pending
invitation rebases or expirations, resolver-index revision, exact archive
reverse references, resolver row, and public-directory entry before the first
publication write. The final apply phase is infallible, so any stale, malformed,
bounded-capacity, or revision-overflow failure leaves the package and its
pending invitations at the same compare-and-set revision.

The private publication control plane is selected only by the platform Iroha
`client.toml`; project manifests, lockfiles, journals, and argv never contain
its endpoints or credentials. The optional strict `[musubi.publication]`
table contains `seed_ingress_url`, `storage_coordinator_url`,
`ingress_broker`, `seed_provider`, `expected_policy_revision`, an optional
bounded `request_timeout_ms`, at least two distinct `provider_gateways`, and
an optional `namespace_delegation_file`. Unknown fields fail configuration
loading. Service and provider URLs are credential-free HTTPS bases with
redirects and ambient proxies disabled. The delegation file is a bounded,
canonical Norito `MusubiNamespaceDelegationV1` whose delegate must be the
configured publisher; it is public authorization material, not a signing key.

Every private request uses a fixed route and a short-lived, domain-separated,
bounded controller-approval set over the chain, publisher, operation id,
operation kind, typed request digest, and validity window. Approvals are
strictly key-ordered and distinct; a single controller requires its one exact
key, while a multisig controller requires valid member signatures meeting its
weighted threshold. A decoded multisig policy is reconstructed through its V1
validator before quorum calculation, so malformed versions, members, weights,
ordering, or thresholds cannot become publication authority. The verifier
rejects a zero trusted time or any requested future-clock allowance above the
fixed 30-second V1 maximum; a caller cannot widen that protocol bound. The
client constructs every payload field and an injected HSM/KMS or threshold
provider returns only approvals, which the client verifies before building a
request. It owns the authorization timestamp, maintains a non-regressing clock
floor shared by its clones, resamples after the external signer returns, and
rejects an authorization whose lifetime was consumed during signing. The
platform `KeyPair` adapter remains single-key only. Seed ingress carries small
canonical request metadata separately from a versioned plan-and-CAR envelope.
The approvals bind the complete archive commitment plus the
domain-separated digest and exact length of the canonical Norito plan witness;
the envelope carries fixed magic/version bytes, the plan and CAR lengths, the
exact witness, and the exact CAR.
The retired public Torii SoraFS upload route is not a fallback. After
clean-package compiler validation, fresh `publish --detach` first persists the
secret-free operation journal as its recovery anchor, then verifies and
immutably installs the exact plan sidecar before the CAR. A crash can therefore
leave a small discoverable journal, optionally with its plan, awaiting the
remaining sidecar, but never an unindexed CAR. `publish --recover OPERATION_ID`
is the bounded workspace-reconstruction recovery path: it accepts only a
pristine Validation-phase, revision-one journal, derives the exact package
selector and verification graph from that journal without reading or rewriting
the workspace lock, rebuilds the clean package, and requires exact publication
and archive-commitment equality. Under the operation lock it reloads the
unchanged journal and idempotently reuses or installs only that operation's
immutable sidecars. Recovery does not load mutation credentials or advance
publication; the user subsequently invokes `--resume`. An advanced journal,
substituted workspace, stale revision, or mismatched existing sidecar fails
closed. Normal resume never reconstructs unpublished workspace state.
On qualified Unix targets, journal, staged-CAR, plan, and operation-lock reads
use architecture-specific no-follow plus nonblocking opens before descriptor
metadata is trusted. Immutable readback and directory synchronization use the
same policy, so a post-inspection FIFO or device substitution fails instead of
blocking the publisher.

The server counterpart is a transport-independent, closed three-route core.
It accepts only exact `POST` routes and canonical bounded Norito authorization
and request encodings,
checks chain/genesis/publisher/operation identity and bounded clock skew,
authenticates before parsing the bounded envelope, then canonically
decodes/re-encodes the plan and reproduces the CAR, root, chunk-plan, PoR,
source-tree, descriptor, semantic-manifest, verification-lock, and bundle
commitments before any journal reservation or backend call. It returns only
canonical typed success or redacted error bodies. Its injected
crash-safe journal atomically consumes authorizations, binds each operation ID
to one chain/genesis incarnation, publisher, and immutable archive/CAR
commitment, rejects equivocation, and reuses an exact completed result. When
only a completed seed receipt has expired, a fresh exact
authorization may atomically reopen that same request: ingress idempotently
confirms the same CAR and the broker replaces only the expired receipt, while a
failure restores the prior completed record. This refresh cannot alter the
operation binding or typed request digest. Seed ingress, permanent
pin/replication coordination, and provider readback are separate injected
backend traits. The service, rather than the signer, constructs the exact
bounded receipt payload and expiry after staging. An injected signing-provider
trait returns only controller approvals; the service assembles and verifies the
receipt against the exact binding and trusted time before journal commit. This
boundary admits HSM, KMS, or threshold collection implementations without
giving them authority to substitute receipt fields. The in-process `KeyPair`
implementation is explicitly a software adapter for focused tests and
controlled development deployments. Private request objects carry no
caller-supplied time. The service samples its injected fail-closed,
non-regressing trusted clock at authorization admission, again after CAR
staging to set receipt issuance and expiry, and after approvals return before
commit. An authorization that was live when atomically admitted need not remain
live during backend or HSM work, but a receipt whose lifetime is consumed by
signing is aborted and never cached. The injected production clock must retain
a rollback-resistant time floor across service restarts; the core also rejects
regression within one process. Service construction also requires the
signing provider's broker and seed backend's provider identity to equal the
public deployment configuration, and rejects a multisig broker when its
highest-weight subset within the 64-approval receipt bound cannot meet the
controller threshold.

Stock `irohad` opens no listener for these routes. A deployment-owned runner
must assemble the service core, crash-safe journal, concrete HSM/KMS signing
provider, SoraFS
backends, and qualified private HTTPS/TLS ingress, then hand that runner to the
dedicated supervisor adapter. TLS material and backend credentials never enter
argv, project files, publication journals, Torii, or the daemon-private runtime
provider broker. Hostname binding to deployment-signed provider adverts and
the concrete HSM/storage adapters remain deployment qualification gates.
The stock tree does not yet supply the authoritative storage/finality backend:
an implementation must prove that the exact transaction contains the matching
archive-registration instruction and was included by the named finalized
snapshot, then match the immutable registration projection against a finalized
exact archive read before coordinating SoraFS. The latest-state
public archive query is sufficient for that projection because Core never
mutates its fields after registration; historical mutable location state is no
longer part of the request contract. Trait injection and authenticated
requester bytes alone are not finality evidence.

The production adapter dependency order is therefore explicit and fail-closed.
The plan-bearing seed boundary is implemented in the stock service core. A
maximal 16,384-chunk witness needs at least 720,896 bytes for offset, length,
and digest fields alone, before file records and Norito framing, so it is
carried in the bounded body envelope rather than the 64 KiB authenticated
metadata header. The publisher persists the canonical witness as an immutable
operation sidecar beside the CAR; detach and resume reproduce the same request
without putting either byte stream or a path in the secret-free journal. The
service rejects the retired raw-CAR media type without compatibility, resolves
the exact registered chunker from the commitment, enforces file/chunk/heap and
decode bounds, and invokes the reusable provider-grade Musubi verifier in
`sorafs_car::musubi::MusubiBundleVerifierV1`. That verifier validates the
canonical CAR and plan plus every nested bundle commitment before the service
calls the admitted seed backend with the verified `CarBuildPlan` and CAR.

The remaining deployment dependencies are:

1. have every admitted provider completion adapter invoke the shared
   provider-grade verifier before issuing its completion-authority attestation;
   seed ingress already uses that verifier, but storage completion alone never
   authorizes an attestation;
2. implement a durable idempotent coordinator which independently retrieves
   and verifies the exact finalized archive-registration transaction, submits
   or reconciles canonical pin and replication operations, waits for each
   distinct provider completion, and returns the complete bounded provider
   attestations plus authoritative current archive/location state; the publisher
   durably registers those proofs one transaction at a time before the compact
   location CAS;
3. implement authenticated provider readback with redirect denial, DNS/IP
   pinning, bounded streaming, and invocation of the same shared verifier; and
4. resolve public policy and identity bindings through `iroha_config`, resolve
   credentials and signing keys only from deployment runtime providers, then
   construct a private TLS runner after daemon-owned finalized-state and SoraFS
   handles are available.

`run_with_musubi_publication` is only the supervisor injection boundary; it is
not a production backend or finality adapter. Until the remaining deployment
dependencies are present and qualified, the stock launch must remain
`Unavailable`. An
in-memory adapter, latest-state query without transaction inclusion proof,
publisher-supplied evidence, ordinary SoraFS storage completion, or the retired
public Torii upload route cannot satisfy this boundary.

Active and yanked releases cannot lose their last healthy archive location.
Explicit location retirement or underlying pin/order invalidation must leave
at least three distinct current providers across the archive's remaining
locations. Deliberate provider removal may reduce that count while at least one
valid provider remains, matching the replica-degradation rule below.
Replica degradation removes a row from fresh selection and emits an alert but
does not rewrite release content. Before either updating availability or taking
an unchanged-state fast path, Core validates the archive record and embedded
identity, every active location's structure and exact storage key, the reverse
references, authoritative releases, resolver rows, and referenced packages.
Any divergence fails without advancing the resolver revision or projection.

## Policy, limits, and operations

`MusubiRegistryPolicyV1` has `Closed`, `Allowlisted`, and `Open` admission.
Closing blocks new archives, releases, and aliases only. Reads, repair, owner
recovery, and yank/unyank remain available. Policy is configuration/governance
state with deterministic defaults, never an environment toggle.

First-release defaults are:

- source payload: 64 MiB;
- bundle payload, including the three mandatory metadata entries: 96 MiB;
- CAR: 96 MiB;
- files: 4,096;
- chunks: 16,384;
- dependencies: 256;
- exports: 1,024;
- resolver graph: 1,024 nodes, depth 64;
- archive locations: four;
- fresh-selection replica quorum: three; and
- query page: 50.

Rust, OpenAPI, Kotlin, Java, and Swift validate the same archive bundle
ceiling. Their focused boundaries accept 64 MiB plus one byte as metadata
headroom and reject 96 MiB plus one byte.

Consensus maxima are fixed and may be larger than local CLI defaults. Every
hash, ordering, resolver choice, and state transition is hardware-independent.

Bounded events plus low-cardinality metric, dashboard, alert, and runbook
contracts cover publication phase age, replication shortfall, ingest
deadletters, integrity failures, cache corruption, stale cursors, unauthorized
governance attempts, and storage pressure. Only an authoritative long-lived
producer may materialize a series. Core, Torii, and the injected private
publication service produce their owned transition/rejection signals; journal
phase age, cache/capacity, selected-root storage, consumer fetch, restart-safe
shortfall alert qualification, and the remaining long-lived producer wiring
remain operational release gates. Paged Musubi queries preserve exact cursor
failures inside Core and map them to Torii's bounded metric reasons without
changing the public `Expired` query-error wire. An absent series is unknown,
not healthy.
Rollout proceeds
through a four-peer devnet, a five-to-ten namespace Taira allowlist with a
two-week soak, and a 30-day invite beta. Open admission requires zero
critical/high findings, completed recovery drills, load and chaos success, and
sustained SLO evidence.
