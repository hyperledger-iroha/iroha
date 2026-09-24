# SoraFS G06 finalized grant-authority seam

2026-09-23, `optimizations`. This source audit follows the qualified, read-only
[current source-assignment service](sorafs-provider-ingest-g06-current-source-assignment-service.md).
It adds no grant resolver and does not qualify provider ingest for release.

## What the committed head authenticates

The [daemon reader](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query.rs)
selects the visible committed State/Kura head and rechecks the provider-ingest
archive generation around an exact assignment lookup. Its
[resolver projection](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query/current_source_assignment.rs)
exposes the network, head, destination provider-state root, assignment revision,
selected source, sorted source inventory, and canonical payload-free request.
The [archive projection](../../../crates/iroha_core/src/query/provider_ingest_finalized.rs)
contains provider owner, completion signer policy, pin manifest, and replication
order records. These are real finalized inputs, but none is a current council
admission, signed advert, admission revocation, HTTPS origin, token-verification
key, or TLS-root approval for the source provider.

Native [StreamToken custody control](../../../crates/iroha_core/src/query/stream_token_custody.rs)
is a separate provider-scoped governed signer role. Its bounded State lookup
checks the exact role/purpose and history at a requested height, and explicitly
leaves durable Kura/QC finality to the caller. It does not publish the current
source advert, authorize its HTTPS origin or DER roots, or join its StreamToken
signer key to a specific source grant. The active provider proof/admission policy
in `sorafs_proof_outcome.rs` governs proof processing, not council membership
for an HTTPS source.

## Missing producer and unsafe join

No consensus-owned State field, SoraFS instruction, or provider-ingest archive
row publishes `ProviderAdmissionEnvelopeV1`, its renewal/revocation lineage, or
`ProviderAdvertV1` at the current finalized network/head. The
[Torii admission registry](../../../crates/iroha_torii/src/sorafs/admission.rs)
verifies council signatures but loads envelopes from an operator directory and
applies revocations to process-local membership. The
[advert cache](../../../crates/iroha_torii/src/sorafs/discovery.rs)
verifies signatures and retains a replay high-water checkpoint, but its accepted
advert is not a finalized revocation snapshot. An older, still-valid envelope
and advert can therefore coexist with a newer revocation unknown to that daemon.
Pairing either cache with the assignment's committed head would incorrectly
promote local observations into final authority. A synthetic or locally signed
grant cannot repair that gap.

The missing producer must expose an authenticated, bounded, monotonic source
governance snapshot, either from consensus-executed native transitions or an
independently finalized governance service with verifiable ancestry to the
assignment head. It must bind exact network and provider identity; council
policy and envelope/renewal predecessor lineage; active revocation at the
selected head; the complete signed advert digest, issuer and validity window;
and separately governed HTTPS origin, StreamToken verification key, and exact
DER trust roots. Its revision, effective height, predecessor digest, and expiry
must make a stale head, omitted revocation, rollback, or key rotation reject.
The immutable worker admission cursor also needs a proved ancestor relation to
the current head. Only then can an independently administered resolver join the
source assignment and governance snapshot, issue an operator-authenticated
bounded grant, and recheck both authorities before fetch, after fetch, and at
reader EOF.

The [HTTPS leaf](../../../crates/irohad/src/sorafs_provider_ingest_runtime/https_source.rs)
still requires an injected `ProviderIngestGovernedHttpsGrantResolverV1`. The
[evidence adapter](../../../crates/irohad/src/sorafs_provider_ingest_runtime/https_source_evidence.rs)
checks already-issued grants against supplied registry, advert, and pins; it
does not authenticate their finalized origin. No production resolver is
installed, so grant issuance remains fail-closed. Implementing a query-only
join before the producer exists would weaken this boundary.
The separate [native source-token control read](sorafs-provider-ingest-g06-native-token-control-read.md)
now supplies one exact-head governed signer-state input without promoting it to
admission, advert, transport, or grant authority.

## Qualification still required

Once the producer exists, test a live State/Kura read at an exact head, then
advance the head through renewal, revocation, advert replacement, source-owner
rotation, and token/TLS pin rotation. A previously issued grant must fail at
each use boundary after any relevant change, including concurrent revocation
and restart. Test stale-head and foreign-network proofs, skipped predecessor,
substituted source, old admission cursor without ancestry, replayed advert,
missing producer, and unavailable finality. Four-validator ingest and recovery
qualification must use the same candidate. No such grant/lineage tests are
claimed here; the existing focused source-assignment tests cover only the
committed assignment observation. No Cargo command was run for this audit.
G06 and promotion remain open. G06 has no HSM prerequisite and introduces no
compatibility route.
