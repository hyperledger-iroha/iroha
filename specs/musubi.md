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
- source content length;
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
matches, the graph is acyclic, and the graph remains within 1,024 nodes and
depth 64. This proof does not become a global resolution for consumers.

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
and archive-location management.

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
unrelated workspace MKHE build failure is cleared.

Snapshot loading reserves the complete generic `musubi` state-path namespace:
the bare name and `_`, `/`, `.`, or `:` descendants are rejected as legacy
pre-release state. This prevents an unenumerated retired record shape from
bypassing the reset check.

V1 exposes typed exact package, release, resolver-index, version, member,
archive-location, alias/history, and ordered-prefix queries. Resolver pages
default to 50 entries. A continuation cursor binds finalized height and hash,
the canonical query hash, the last returned key, index revision, and caller
when authorization affects output. A changed anchor, query, revision, caller,
or boundary is an explicit stale-cursor error.

Every resolver-index, version, maintainer, alias-history, ordered-prefix, and
search page echoes the exact bounded request object that produced it, including
its structural selector or terms and page controls. This response context is
required even for an empty first page, so clients compare canonical fields
directly instead of attempting to reproduce a Norito query hash or infer an
identity from result rows. Page validation requires every row to match the
echoed package, requirement, alias, or complete prefix; enforces the requested
effective limit; and binds a continuation cursor to the exact final row of a
full page. Version and resolver cursor advancement uses structured SemVer,
including prereleases, rather than lexical version text. Ordered-prefix text is
canonical `namespace/package-prefix` syntax and remains present when no package
matches. Search echoes the original bounded query text, preserving first-page
identity before a search cursor exists.

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
frame, inline pair, and standalone frame. Its eighteen cases cover namespace
registration; maintainer invitation, acceptance, revocation, role replacement,
and removal; global-alias registration; exact release-digest assertion;
archive registration and location addition or renewal; archive-location
retirement; release publication and reversible yank state; package metadata
replacement; and Parliament-enacted package ownership recovery,
permanent-alias retargeting, artifact takedown, and registry-policy replacement
across Rust, Kotlin, Java, and Swift. An already-framed generic instruction
wrapper is not typed field-to-Norito support.

Swift additionally embeds all eighteen fixture cases in one real signed batch
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
alias, child node, and dependency kind. Multiple versions of one package are
valid.

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
opaque byte string.

Authenticated fetches reject oversized JSON nesting, token inventories,
strings, and unquoted scalar literals before constructing a Norito JSON DOM.
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
4. Wait for finalized approval and distinct provider validations/completions.
5. Require three healthy replicas and read back through two distinct providers.
6. Submit the package claim and immutable release through Native AMX.
7. Wait for finality and verify the exact universal resolver row.

The publication proof resolver may retain an existing lock edge only while its
row remains fresh-selectable; yanked, below-quorum, unavailable, or governed
takedown rows require a new proof graph (and make `--locked` fail). The proof's
snapshot must be a canonical finalized ancestor on the current chain, and its
index revision cannot be from the future. This permits replication and readback
blocks to finalize without invalidating the operation. Core still revalidates
every exact proof row and fresh-selection state against current authoritative
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
parsed-bundle provider attestations. The projection contains the archive id,
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
never-before-used stable location ID, provider attestations, exact instruction
digest, fee-quoted signed transaction, and transaction hash. The private
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
and provider attestations, all of which must still bind the exact chain,
archive, bundle, source, semantic manifest, verification lock, and replication
order. Production qualification still must exercise the real fee-quote and
submission transport at every location-generation crash boundary.
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
request. The platform `KeyPair` adapter remains single-key only. Seed ingress
carries the canonical bounded request metadata separately from the raw CAR body,
and the approvals bind that exact metadata.
The retired public Torii SoraFS upload route is not a fallback. Fresh `publish
--detach` persists the CAR and journal only after the clean package has passed
compiler validation, so a normal resume never needs to reconstruct unpublished
workspace state.

The server counterpart is a transport-independent, closed three-route core.
It accepts only exact `POST` routes and canonical bounded Norito authorization
and request encodings,
checks chain/genesis/publisher/operation identity and bounded clock skew,
authenticates before hashing a CAR, verifies its exact length and digest, and
returns only canonical typed success or redacted error bodies. Its injected
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

The production adapter dependency order is therefore explicit and fail-closed:

1. extend the authenticated seed request with a bounded canonical SoraFS plan
   projection, and durably stage/read back the opaque CAR against that plan and
   the complete archive commitment; a CAR digest and length cannot reconstruct
   the plan required by provider ingest;
2. place a reusable full Musubi bundle verifier below the publisher crate and
   have each admitted provider issue its completion-authority attestation only
   after verifying the CAR, plan, descriptor, source tree, semantic manifest,
   and verification lock;
3. implement a durable idempotent coordinator which independently retrieves
   and verifies the exact finalized archive-registration transaction, submits
   or reconciles canonical pin and replication operations, and returns only
   authoritative current archive/location state and distinct-provider
   attestations;
4. implement authenticated provider readback with redirect denial, DNS/IP
   pinning, bounded streaming, and the same complete verifier; and
5. resolve public policy and identity bindings through `iroha_config`, resolve
   credentials and signing keys only from deployment runtime providers, then
   construct a private TLS runner after daemon-owned finalized-state and SoraFS
   handles are available.

`run_with_musubi_publication` is only the supervisor injection boundary; it is
not a production backend or finality adapter. Until all five dependencies are
present and qualified, the stock launch must remain `Unavailable`. An
in-memory adapter, latest-state query without transaction inclusion proof,
publisher-supplied evidence, ordinary SoraFS storage completion, or the retired
public Torii upload route cannot satisfy this boundary.

Active and yanked releases cannot lose their last healthy archive location.
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
- CAR: 96 MiB;
- files: 4,096;
- chunks: 16,384;
- dependencies: 256;
- exports: 1,024;
- resolver graph: 1,024 nodes, depth 64;
- archive locations: four;
- fresh-selection replica quorum: three; and
- query page: 50.

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
