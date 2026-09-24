# SoraFS native provider-admission producer design

2026-09-24, `optimizations`. **Design only; not implemented.** This records the
next F05/G06 source-authority cut after the retained admission-cursor ancestry
check. It does not authorize HTTPS grants or qualify provider ingest for release.

## Authority and V1 wire decision

The current provider-admission envelope, renewal, and revocation types in
`crates/sorafs_manifest/src/provider_admission.rs` verify council signatures and
predecessor envelope digests. `ProviderAdmissionCouncilPolicy` is a runtime
trust set, however, and the signed envelope/revocation bodies omit the exact
network identity. A daemon-configured policy or a local Torii admission
directory cannot become a deterministic consensus trust root.

For the single first-release V1 layout, add the genesis-derived network ID to
the canonical council-signed admission envelope and revocation body, and make
renewal explicitly require the same network as its predecessor and new
envelope. The provider advert signature payload also needs the network ID
before it can be a grant input. Regenerate their Norito schema identities,
fixtures, SDK decoders, and all affected digest/signature vectors together;
reject the retired layouts without a fallback decoder. The manifest crate can
carry the 32-byte identity and Core must compare it with its exact `NetworkId`.
This decision changes V1 bytes because the first release has no backward
compatibility contract.

The council trust policy must be canonical consensus state: a bounded sorted
set of strong Ed25519 public keys, nonzero threshold, policy identity,
monotonic revision, predecessor digest, and governed activation/revocation.
Parliament or an equivalently certified native governance transition installs
and rotates it. Every validator reconstructs the same
`ProviderAdmissionCouncilPolicy` from that finalized state before verifying an
admission, renewal, or revocation; no operator-only policy can decide ledger
execution. Authenticated software custody is sufficient. No HSM is required.

## Vertical implementation cut

1. Add a bounded native admission head/tombstone per provider to the data model
   and `World` storage. It carries the exact network, current signed envelope
   digest and approved advert-body digest, policy revision, monotonic admission
   revision, predecessor digest, effective height/time, and revocation state.
   Bound canonical envelope bytes, signature and endpoint counts, provider count,
   and aggregate State/archive allocations before mutation. Preserve a
   revocation tombstone so removing an owner or replaying an old envelope cannot
   silently restore source authority.
2. Define closed native admit, renew, and revoke instructions (or one closed
   action) in `iroha_data_model`, with exact wire IDs and Norito roundtrips.
   Execute through `StateTransaction.world` in
   `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`, using the current
   consensus policy, existing council cryptographic validators, and atomic
   compare-and-set predecessor checks. Reject missing policy, foreign network,
   stale policy, skipped revision, invalid council quorum, expiry, replay,
   owner removal, and revocation conflicts. Direct local registry mutation is
   not an execution path.
3. Extend `ProviderIngestFinalizedProviderProjectionV1` and
   `capture_projection` in
   `crates/iroha_core/src/query/provider_ingest_finalized.rs` with the current
   admission/tombstone state. The projection builder must union provider IDs
   from the admission map with registered owner IDs; today it starts only from
   `provider_owners`, which would omit a revoked or removed provider. Existing
   Apply capture in `crates/iroha_core/src/sumeragi/v2_apply.rs` then binds each
   changed projection to the same immutable State view and exact Kura V2
   finality receipt before WSV publication. Startup reconciles the State/Kura
   tip and fails closed on an archive gap.
4. Expose an exact-head, generation-fenced daemon query joining the current
   assignment, retained job-cursor ancestry, council admission lineage, and
   native StreamToken control. Keep `ProviderIngestGovernedHttpsGrantResolverV1`
   uninstalled until a finalized signed advert, separately governed HTTPS
   origin, StreamToken use statement/verification key, exact DER roots, and
   revocation-fresh use-boundary checks are also implemented.

The archive currently returns `BelowRetentionFloor` for pruned historical job
cursors. This is the correct fail-closed behavior. Production liveness needs an
approved policy that retains active-job anchors or a separately authenticated
compact ancestry witness; neither may be inferred from a local outbox cache.

## Required proof and tests

Test canonical network-bound Norito bytes and signatures, foreign-network
replay, duplicate or skipped renewal, stale policy, malicious quorum, expired
envelope, revocation and re-admission, owner removal, and atomic transaction
rollback. Exercise real Apply-to-Kura-receipt archive capture and exact-head
query across admission, renewal, revocation, restart/pending-tip recovery,
missing QC, a stale fork, archive-generation change, and retention below an
active job cursor. A checked council signature without the matching finalized
State/Kura head is insufficient. Four-validator source/recovery qualification
and the HTTPS grant producers remain separate open release gates.

## 2026-09-24 wire and fixture impact audit

**Read-only inventory; no network-binding source change or qualification has
landed.** The smallest single-V1 wire cut adds a required raw 32-byte genesis
network ID, without `#[norito(default)]`, to `ProviderAdmissionEnvelopeV1` and
`ProviderAdmissionRevocationV1` in
`crates/sorafs_manifest/src/provider_admission.rs` and `ProviderAdvertV1` in
`crates/sorafs_manifest/src/provider_advert.rs`. The envelope's unsigned
authorization digest already hashes the complete envelope minus signatures,
so its new field enters both the council signing preimage and final envelope
digest. Revocation also has an independently encoded `RevocationBody` inside
`digest()`; add the field there or a changed outer record would still accept
the old signed body. The advert has both an owned
`ProviderAdvertSignaturePayloadV1` and a borrowed
`ProviderAdvertSignaturePayloadViewWireV1`; update both in the same field order
and retain their byte-identity test. `ProviderAdvertBuilder` must require the
network rather than supply a zero or local default.

Compare the retained envelope's network with the new envelope in
`AdmissionRecord::apply_renewal_inner`, with the revocation in
`verify_revocation_inner`, and with the advert in
`verify_advert_against_record`. Core's eventual native instruction must also
compare all three with its exact genesis-derived `NetworkId`; a valid foreign
council or provider signature is insufficient. The current runtime council
policy and Torii admission directory are not consensus authorization.

Direct construction and signing sites to migrate together are:

- Production and fixture tools:
  `crates/sorafs_car/src/bin/sorafs_provider_advert.rs` (advert builder),
  `crates/sorafs_car/src/bin/sorafs_manifest_builder/provider_admission.rs`
  (envelope sign, renewal, revocation),
  `crates/sorafs_car/src/bin/provider_admission_fixtures.rs` (advert, envelope,
  revocation), and `xtask/src/sorafs.rs` (advert/envelope fixture writer).
  Offline software signing must take an explicit network input for advert and
  admission creation; revocation inherits and verifies the retained envelope
  network. The two fixture writers target the admission fixture surface and
  must not publish divergent vectors.
- Rust test constructors and decoded fixture consumers:
  `crates/sorafs_manifest/src/{provider_advert.rs,provider_admission.rs,pdp.rs,reference.rs}`,
  `crates/sorafs_manifest/src/provider_admission/tests/canonical_preimages.rs`,
  `crates/sorafs_manifest/tests/{provider_admission_fixtures.rs,discovery_propagation.rs}`,
  `crates/sorafs_car/src/bin/sorafs_fetch/tests.rs`,
  `crates/sorafs_car/tests/fetch_cli.rs`,
  `crates/sorafs_node/src/{pdp_provider.rs,potr.rs}`,
  `crates/iroha_torii/src/sorafs/{admission.rs,api.rs,delegated_routing.rs}`,
  `crates/iroha_torii/src/tests/routing_app_api_integration.rs`,
  `crates/iroha_torii/tests/sorafs_discovery.rs`,
  `crates/irohad/src/soracloud_runtime.rs`, and
  `crates/irohad/src/sorafs_provider_ingest_runtime/https_source_evidence/tests.rs`.
  Compile failures after making the three fields required should expose
  remaining struct literals; do not add a compatibility constructor.

Regenerate all admission `advert`, `advert_renewed`, `envelope`,
`envelope_renewed`, `renewal`, and `revocation` V1 `.to` and `.json` files,
plus `metadata.json`, under `fixtures/sorafs_manifest/provider_admission/`
with `cargo run --locked -p sorafs_car --features manifest,dev-tools --bin
provider_admission_fixtures`. Proposal bytes need not change unless their
source changes. Refresh the compiler-observed schema hashes in
`provider_admission/captured_owner_identity_tests.rs` and
`provider_advert/captured_owner_identity_tests.rs`, the independent revocation
preimage capture in
`crates/sorafs_manifest/tests/fixtures/provider_admission_revocation_identity.jsonl`,
and the advert signing capture in
`crates/sorafs_manifest/tests/fixtures/sorafs_signing_identity_frames.json`.
These captures require review against independently encoded bytes, not a
blind hash replacement.

The changed advert/envelope bytes also change the fixed hashes and lengths in
`fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json` and
the routing admission outcome in
`fixtures/sorafs_manifest/reference_sdk/bundle_routing_admission_positive_validation_outcome_v1.json`.
Regenerate these with `cargo run --locked -p sorafs_manifest --features
dev-tools --bin generate_por_fixtures -- --write`, then run its `--check`
mode and `scripts/check_sorafs_reference_sdk_fixtures.py` against the same
source. `crates/sorafs_manifest/src/reference_ffi.rs` and `reference.rs`
also consume renewal/revocation fixtures. The native reference bridge is used
by Swift, Kotlin and Java-source consumers, JavaScript, Python, and C#;
their fixture-bundle suites must all be run after rebuilding matching native
libraries. `ci/check_sorafs_fixtures.sh` independently checks deterministic
provider fixture generation. Router OpenAPI regeneration is required if the
Torii request or response schema changes; no separate typed SDK advert
decoder was found in this audit.

Focused adversarial matrix: accept same-network signed
admission/advert/renewal/revocation; mutate each network field after signing
and reject the signature; independently re-sign a foreign-network object and
reject it against Core's genesis network; reject cross-network renewal and
advert/record mismatch even when both objects have valid signatures; reject
foreign-network revocation, replay, and missing-field retired V1 bytes without
state mutation; prove owned/borrowed advert signing frames remain identical;
and check regenerated fixture bytes twice plus native-reference SDK parity.
This wire cut leaves finalized council custody, native tombstones, HTTPS grant
production, and F05 release qualification open.

## 2026-09-24 signed-transition and constructor audit

**Read-only finding; no native council producer or grant authority exists.**
`ProviderAdmissionCouncilPolicy` in `sorafs_manifest` currently contains only a
strong Ed25519 key set and threshold. Core has no finalized
provider-admission council policy, identity, digest, revision, or admission
instruction. `crates/irohad/src/main.rs::build_shared_sorafs_provider_cache`
instead constructs the policy from the node-local
`sorafs.discovery.admission.trusted_council_keys` and `signature_threshold`,
then loads `envelopes_dir` into Torii's `AdmissionRegistry`. Torii
`GatewayPolicy::evaluate` consults that local registry. These inputs cannot
authorize consensus admission or a production HTTPS grant. They must be
replaced as production trust sources when the finalized producer is connected;
retaining them as an alternate first-release authority would violate the
single-V1 contract.

The renewal predecessor is **checked but not council-signed** today:
`ProviderAdmissionRenewalV1.previous_envelope_digest` is outside
`ProviderAdmissionEnvelopeV1`, whose unsigned canonical encoding is the
council signing preimage. `AdmissionRecord::apply_renewal_inner` compares the
outer predecessor with the current record, but that does not prove council
approval of this particular transition. A valid signed envelope can therefore
be paired with a submitter-selected predecessor if its other renewal
invariants match. Similarly, `ProviderAdmissionRevocationV1::digest` signs a
separate `RevocationBody`, so every new authority-context field must be added
to that inner body. Signing only network identity is insufficient to prove
policy epoch or exact admission-head lineage after rotation or revocation.

The implementation order is deliberately fail-closed:

1. **Wire identity only.** Add the required network field to the envelope,
   revocation signed body, and both owned and borrowed advert signing payloads
   as inventoried above. Require an explicit network in producers; reject old
   V1 bytes and cross-network signatures. Regenerate and independently check
   fixtures and SDK frames. This step supplies domain separation but does not
   activate native admission, the gateway's local registry as consensus
   authority, or the HTTPS grant resolver.
2. **Finalized policy and signed transition.** Define one canonical bounded
   council policy record in `iroha_data_model` with network, ordered strong
   signer keys, threshold, identity, revision, predecessor policy digest, and
   a specified domain-separated canonical digest. Add certified governance
   proposal/enactment in `governance/types.rs`, `isi/governance.rs`, Core
   `smartcontracts/isi/world.rs`, and `state.rs`, with exact wire IDs, Norito
   tests, and State overlay/restart coverage. Only after Core can read this
   finalized policy from the same `StateTransaction` should the final V1
   council-signed envelope and revocation preimages require its exact
   identity/digest/revision, the next admission revision, and the expected
   current admission-event digest. Initial admission expects no predecessor;
   renewal expects the live envelope event; re-admission after revocation
   expects the tombstone event. The outer renewal predecessor must equal the
   signed predecessor, and Core must compare all signed fields with its
   current finalized policy/head and genesis-derived `NetworkId` before any
   mutation. No node-local or fixture policy digest may satisfy that check.
3. **Head, archive, and consumers.** Implement the bounded per-provider
   admission head/tombstone and atomic admit/renew/revoke execution in
   `isi/sorafs.rs` and State. Governed provider-owner rebind/removal currently
   clears completion authority; it must also invalidate admission atomically
   and retain a tombstone. Extend the finalized projection in
   `query/provider_ingest_finalized.rs` by unioning owner and admission IDs,
   then join it to the exact State/Kura head in the daemon. The current
   `https_source_evidence.rs` accepts an `AdmissionRegistry` and advert cache
   only as externally supplied evidence; it is not a production resolver.
   Replace the node-local admission authority in daemon/Torii constructors
   with a finalized query before enabling the gateway or HTTPS grants, and
   retain the independent signed advert, transport pins, StreamToken, and
   revocation-fresh checks.

Tests must include an overlapping-key policy rotation, valid council signature
paired with a changed outer predecessor, duplicate/skipped transition revision,
revoked-head re-admission, owner rebind/removal, stale fork, archive restart,
and transaction rollback. Use deterministic block time for consensus expiry
checks. Bound native decode and retained State/archive allocations before
cloning envelopes or verifying attacker-supplied signature vectors; Torii's
directory caps do not bound consensus input. A changed projection/State wire
must reject or deliberately rebuild incompatible pre-release archives, with
no fallback decoder. The candidate remains blocked until these same-source
positive and negative paths pass; Phase 1 fixtures alone do not qualify it.
