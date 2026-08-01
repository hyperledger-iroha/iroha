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
`=`. Equivalent accepted input produces identical Norito bytes. A prerelease
candidate is eligible only when a comparator in the requirement names a
prerelease with the same major, minor, and patch tuple.

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

Sora Parliament recovery consumes an enacted decision bound to the exact
action digest. Core verifies the existing enactment delay and records the
decision as consumed so it cannot be replayed. Recovery actions cover package
ownership, alias retargeting, and artifact takedown only; ordinary owners
cannot invoke those exceptional paths.

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

V1 exposes typed exact package, release, resolver-index, version, member,
archive-location, alias/history, and ordered-prefix queries. Resolver pages
default to 50 entries. A continuation cursor binds finalized height and hash,
the canonical query hash, the last returned key, index revision, and caller
when authorization affects output. A changed anchor, query, revision, caller,
or boundary is an explicit stale-cursor error.

Description and keyword search is a rebuildable projection of finalized
events. Fuzzy or partial search never affects resolution.

## `Musubi.toml` and workspaces

Every manifest declares `manifest-version = 1`. Unknown or duplicate fields
are errors at every nesting level. A package manifest supports package
metadata, a configurable library directory, explicit exports, optional local
contract targets, tests, readme, license, repository, keywords, and positive
include additions.

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
target constraint. Fresh candidates exclude yanked, takedown, and below-quorum
rows.

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

The canonical bundle contains the semantic release manifest, typed artifact
descriptor, normalized source tree, and verification lock. Provider validation
attests successful parsing and verification of that bundle, not storage of an
opaque byte string.

The user cache path is derived only from the trusted root and archive id:

```text
registry-v1/<archive-id>/src
```

Extraction streams into a private sibling with no-follow/create-new file
creation, verifies every commitment, fsyncs files and directories, then
renames into an absent immutable destination. Repair quarantines only validated
descendants. Lock-controlled deletion, arbitrary replacement, and cache import
do not exist.

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

The operation journal contains no secrets and is safe to resume. `publish
--detach` may return the operation id; ordinary `publish` succeeds only after
step seven. Retrying identical commitments is idempotent. Reusing a package
version with different commitments is permanently rejected.

Active and yanked releases cannot lose their last healthy archive location.
Replica degradation removes a row from fresh selection and emits an alert but
does not rewrite release content.

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

Bounded events, low-cardinality metrics, and operator alerts cover publication
phase age, replication shortfall, ingest deadletters, integrity failures,
cache corruption, stale cursors, unauthorized governance attempts, and storage
pressure. Rollout proceeds through a four-peer devnet, a five-to-ten namespace
Taira allowlist with a two-week soak, and a 30-day invite beta. Open admission
requires zero critical/high findings, completed recovery drills, load and chaos
success, and sustained SLO evidence.
