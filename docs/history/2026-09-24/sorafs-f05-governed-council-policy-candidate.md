# SoraFS F05 canonical council-policy candidate

2026-09-24, `optimizations`, in `/Users/takemiyamakoto/devstuff/iroha`.
This is one scoped provider-admission source cut, not production admission or a
grant. The sole V1 DataModel `ProviderAdmissionCouncilPolicyV1` now fixes a
nonzero genesis network and policy identity, immediate predecessor-linked
revision, at most 32 strictly ordered strong Ed25519 keys, a satisfiable
threshold, a pause state, and a domain-separated canonical Norito digest. Its
pure signed-claim validator checks the exact policy id/revision/digest,
network, deterministic execution lifetime, and council signature quorum. The
policy remains a **candidate** until consensus governance enacts it; decoding
or locally constructing it grants no authority. Tests cover Norito roundtrip,
key/order/capacity rejection, successor substitution, and signed-envelope
claim, signature, pause, and expiry checks. No alternate V1 decoder or HSM
requirement was added.

The 32-key ceiling is checked by `validate()` **after** generic Norito
decoding. Norito's sequence decoder allocates for the advertised vector before
this type's validator runs unless its caller installs explicit `DecodeLimits`.
No native pre-allocation decode/resource admission or State/archive retention
budget is implemented by this cut; those remain required before network input
can carry this policy.

The source still has no governed State policy row, admission-event head or
revocation tombstone, native admit/renew/revoke execution, or finalized
State/Kura reader. `ProviderAdmissionCouncilPolicy` in
`crates/sorafs_manifest/src/provider_admission.rs` is still a local key/quorum
set; `crates/irohad/src/main.rs::build_shared_sorafs_provider_cache` loads
`trusted_council_keys` and `envelopes_dir` from node-local configuration.
`crates/iroha_core/src/query/provider_ingest_finalized.rs::capture_projection`
starts with `provider_owners` and has no admission/head union. The current
`https_source_evidence.rs` accepts caller-supplied registry, advert and pins,
and explicitly cannot issue or authenticate a grant. Those paths must not be
treated as a second production trust source.

The next atomic implementation cut is native Parliament enactment and
World/State retention of this policy, followed by a bounded per-provider
admission-event head/tombstone with exact compare-and-set transitions. Only
then can the finalized projection union provider IDs from both owner and
admission maps and the daemon query authenticate an exact current head. The
HTTPS resolver needs the separate governed origin, StreamToken issuer, DER
roots, and per-use revocation check. Until those are connected, F05/G06 and
promotion remain blocked. The design and adversarial requirements are recorded
in `docs/history/2026-09-24/sorafs-provider-ingest-native-council-producer-design.md`.

Validation: focused `scripts/cargo_fast.sh --stable-local-metadata
--incremental -- test -p iroha_data_model --lib
sorafs::provider_admission::tests -- --nocapture` passed **3/3** in this checkout.
Scoped Rust formatting and `git diff --check` passed. This is local policy
codec/claim evidence only, not native enactment or a release claim.
