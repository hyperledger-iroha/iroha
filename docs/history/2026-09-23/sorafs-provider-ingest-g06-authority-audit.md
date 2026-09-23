# SoraFS G06 provider-ingest authority audit

2026-09-23, `optimizations`. This is a read-only source audit of the current
working tree. It does not add a provider backend or qualify a release.

## Finalized assignment boundary already present

The [Core provider-indexed archive](../../../crates/iroha_core/src/query/provider_ingest_finalized.rs#L2721)
reads one provider/order at an exact archive key, returns the provider-state root
and derives the sorted source-provider IDs from the canonical order assignments.
It does not select a current head; its caller must authenticate the key.
The [daemon current-assignment reader](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query.rs#L1289)
does that through the committed State/Kura-visible archive head. It checks archive
generation and the visible key around the direct read, then rejects a missing or
non-pending order, unapproved or changed manifest, source inventory or assignment
revision mismatch, and substituted Musubi binding. The separate
[`validate_with_current_source_request` wrapper](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query.rs#L1362)
brackets an external observation with two current assignment reads and a generation
fence. Neither path issues a grant, authenticates a provider advert, or proves that
an older admission cursor is an ancestor of the current head.

The [worker's finalized-ledger trait](../../../crates/sorafs_node/src/provider_ingest_runtime.rs#L957)
requires `ProviderIngestFinalizedClaimFactoryV1`, whose constructor is private to
the worker. The normal authorization and source request carry no credential or
independent finality capability. Publishing that constructor would let a source
resolver impersonate the worker's claim boundary; it would not establish a
governed read authority.

## Authority still missing

The [HTTPS evidence adapter](../../../crates/irohad/src/sorafs_provider_ingest_runtime/https_source_evidence.rs#L57)
accepts a supplied source request, assignment revision, clock, council-verified
`AdmissionRegistry`, signed `ProviderAdvertCache`, transport pins and already-issued
grant. Its checks bind those inputs to the grant and can reject a revoked registry
entry even while an older advert remains cached. This is validation of supplied
values, not proof that they came from one fresh finalized authority. The current
assignment wrapper is crate-private and is used by a missing-finality test; the
[HTTPS leaf](../../../crates/irohad/src/sorafs_provider_ingest_runtime/https_source.rs#L123)
still requires an injected `ProviderIngestGovernedHttpsGrantResolverV1` to issue and
recheck a grant at each use boundary. The
[catalog-bound source-pool constructor](../../../crates/irohad/src/sorafs_provider_ingest_runtime/https_source_pool.rs#L72)
accepts such resolvers but does not implement one.

The [standard daemon's shared cache builder](../../../crates/irohad/src/main.rs#L364)
loads council envelopes from a configured directory and constructs a persistent
advert replay cache. [Admission revocation](../../../crates/iroha_torii/src/sorafs/admission.rs#L230)
removes an entry from that registry; [advert replay persistence](../../../crates/iroha_torii/src/sorafs/discovery.rs#L957)
protects cache high-water marks. These mechanisms verify signatures and local
replay behavior, but do not tie admission, advert and revocation currentness to
the same finalized network/head as the assignment. The [native State fields](../../../crates/iroha_core/src/state.rs#L6038)
and [provider-ingest archive projection](../../../crates/iroha_core/src/query/provider_ingest_finalized.rs#L285)
expose provider owner, pin, order and completion-authority records; no
admission-envelope, advert or revocation projection appears in that archive. A
cached admitted advert cannot stand in for current revocation evidence.
Advert signing keys, stream-token issuer keys and TLS DER trust roots are separate
roles; none can be inferred from the other. The [existing token route](../../../crates/iroha_torii/src/sorafs/api/storage_token_issuance.rs#L69)
requires an exact-network authenticated operator signature and an enabled issuer,
so a local token or synthetic grant is not evidence.

The next narrow implementation slice is a **qualified read-only source-assignment
service** backed by the existing daemon current-head archive reader. It should
return the canonical request, assignment revision, exact network/head and source
inventory to an independently administered resolver without exposing the worker's
claim factory or a completion-signing capability. The resolver must join that read
to an authenticated, freshness-bounded admission/advert/revocation snapshot whose
lineage is proved against the finalized context, plus separately governed origin,
issuer-key and DER-root pins. It should use the existing evidence validator while
resolving an operator-authenticated bounded grant, and repeat the complete
currentness check before fetch, after fetch and at reader EOF. Unavailable,
changed or stale authority must fail closed. A chain projection or independently
finalized governance service for admission/advert/revocation is prerequisite to
that join; the directory and cache alone do not provide it. This slice does not
claim publisher seed transfer, external completion signing, sealed checkpoint
deployment, or final publication receipts.

## Validation limits

The [dated cross-component checkpoint](election-signer-provider-js-checkpoint.md)
reports three focused Core direct-lookup tests, an extended archive-corruption
test, three daemon current-assignment tests, and one missing-finality evidence
test passing. Those current-assignment results were described as local provisional
evidence because shared DataModel/Core edits overlapped the build. The six
[council/advert evidence tests](../../../crates/irohad/src/sorafs_provider_ingest_runtime/https_source_evidence/tests.rs#L255)
use real signature and cache operations on explicitly constructed fixtures; they
cover binding, revocation with a retained cached advert, replacement, expiry and
substitution. One separate test proves that otherwise-valid fixture evidence is
rejected without a live finalized assignment. These tests do not exercise a
production resolver, finalized admission ancestry, revocation activation across
peers, real token issuance, live TLS, or a four-validator ingest. No Cargo command
was run for this audit. G06 remains open under the
[first-release blocker](../../../specs/sorafs/v1_closure_ledger.md#L1871).
