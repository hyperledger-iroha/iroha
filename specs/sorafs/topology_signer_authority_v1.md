# Topology signer authority V1 prerequisite

Status: open. The topology receipt contract is a pure consistency prerequisite. It does not
integrate topology approval into production promotion, authenticate native execution, qualify a
software provider, or prove that a topology was deployed. Authenticated software custody is
supported; hardware is optional and provides no implicit signing authority.

## Current contract and boundary

`crates/sorafs_manifest/src/signer/topology.rs` owns role 16 `TopologyApproval`, isolated from
foundational promotion (5), release manifests (13), final promotion provenance (14), and final
promotion account transactions (15). Its exact label is `topology_approval`; its role domain is
`sorafs.production-readiness.topology-approval.v1`. The deployment-bound purpose admits only
Ed25519. Repository source and maintained SDK inventories had no prior role 16 or topology signer
purpose. Ordinary native roles remain the closed four-role set.

The sole canonical Norito subject binds deployment, genesis-derived network identity, chain label
and address discriminator, the exact release-manifest SHA-256 (candidate), configuration summary,
raw and canonical topology manifests, ordered-validator inventory, review time and exclusive
expiry. Review validity is at most fourteen days. The signed message is the topology-only domain
prefix plus the canonical header-bearing subject. It approves configuration, never live evidence.
An independently reviewed subject must come from validating the actual exact source documents;
this pure crate does not load them, recompute their SHA-256 values, or discover trust from them.

Preparation retains the whole binding digest and reviewed subject. The request binds that digest,
the independently assigned operation id, original custody record/control-state identity and exact
subject commitment. The receipt uses the existing shared four-signature, audit, provenance,
reservation and timely immutable-completion machinery, with topology-specific request, payload,
audit and response domains. The detached role signature must be exactly the receipt's first
signature. Current signer/attester revocation, active enrollment and trusted time are checked.
Recovery may occur after the original reservation expires if its original completion was timely;
the independently reviewed subject must still be current.

`check_topology_receipt_consistency_v1` returns only `Result<()>`. Supplied current/completed rows
can be internally consistent without being authoritative. The result cannot prove that a native
Check or signing operation executed, that the transaction authority held the deployment-scoped
permission, or that result/output inclusion corresponds to the exact input. No decodable or private
"verified topology authority" token is produced. No CLI verifier or production approval consumer
accepts this result. The generic software signer rejects role 16 before provisioning or endpoint
I/O, and its native adapter does not map role 16 to another role.

## Missing native authority and production assembly

The next implementation owner is a dedicated topology authority, following the *interfaces* of
final-promotion authority without reusing its role, permissions, schema identities or namespace.
These work packages are dependencies, not implemented types or completed release evidence:

1. **Data model and execution state.** Add a topology-owned module under
   `crates/iroha_data_model/src/sorafs/` and a typed ISI in `src/isi/sorafs.rs`. It must own bounded
   custody-control revisions, exact request authorization, exclusive reserve/fence/expiry, immutable
   completion and challenge-bearing current Checks. Record the actual executing account, block
   height, ordered instruction index and execution time. Deployment/candidate/request subjects
   cannot change after reservation. Complete-before-release and ambiguous recovery must compare
   the original id, intent, custody, reservation, signatures and audit/response commitment.
   Never accept a caller-supplied execution height or completed row as executed state.
2. **Permissions and Core.** Add deployment-scoped manage, operate and check permissions to
   `iroha_executor_data_model::permission` and actual executor dispatch. Add a purpose-owned Core
   ISI implementation and bounded durable history/restart validation. Extend the closed native
   proof purpose owner in `iroha_core/src/query/signer_check.rs` only after this real ISI exists.
   The same-State history reader and current observation must authenticate exact ordered Network
   input, success result and execution-output inclusion under genuine revision-4 finality, retaining
   the original challenge owner, monotonic deadline and independent height floor. A QC or signed
   observer row alone is insufficient. Required controls include wrong authority/permission,
   revoked/rotated custody, replayed challenge, candidate/intent/fence substitution, failed ISI,
   unrelated successful input, wrong result/output index, divergent state, crash and restart.
3. **Configured service assembly.** Add explicit user-to-actual configuration for topology signer,
   independent custody authority, native account/check provider and governed trust/currentness.
   Thread them through the existing daemon provider registry/broker and operation coordinator.
   Before each provider call, completion and release, consume a fresh topology-owned native Check.
   Keep reserve, key operation, journal, completion CAS and read-only ambiguous recovery ordering.
   An injected provider or local signer journal is not finalized state. Enable role16 generic
   admission only when this assembly exists; do not enable it merely because the receipt parses.
4. **Schemas and executable consumer.** Register only actual topology ISI/state/public request
   roots in `iroha_schema_gen`, regenerate canonical SDK/CLI fixtures, and test missing/unknown/
   retired layouts. Add a topology-owned `iroha app sorafs toolkit` command only when it can consume
   the genuine native authority result and pin the exact reviewed subject, policy, trust, receipt,
   binary and evaluation time. The existing final-promotion command and foundational
   `verify-receipt` are different purposes; neither may be retagged.
5. **Atomic promotion cutover.** Replace the old envelope and every producer/consumer below in one
   source/schema/fixture cutover. Keep `validate_inner_approval_chain()` unconditionally blocking
   until foundational, topology, resilience and lane-inventory approvals each have their genuine
   native custody and completed-operation verifiers. Then connect all four; an outer signature
   over their hashes does not fill a missing inner authority.

## Old envelope to remove in the cutover

The existing `sorafs.l1.deployment_qualification.signed_envelope.v1` is a detached Ed25519 envelope
under `sorafs-l1-topology-qualification-envelope-v1\0`, with exactly these twenty fields:

- `schema`, `reviewed_at_unix`, `signature_algorithm`, `signature_hex`;
- `qualification_summary_sha256`, `manifest_sha256`, `canonical_manifest_sha256`, `deployment_id`,
  `environment`, `network`, `chain_id`, `chain_discriminant`, `validator_ids_sha256`;
- `signer_authentication_kind`, `signer_service_id`, `signer_administrator_id`, `signer_key_revision`,
  `signer_policy_revision`, `signer_policy_digest_sha256`, `signer_public_key_fingerprint_sha256`.

It has no canonical custody record, operation reservation or native completion proof. Its detached
signature checks cannot establish those properties. The new contract has no decoder for it.
Until the atomic cutover it remains an input to the explicitly blocked qualification tooling;
removing it early would strand the existing configuration-only workflows without adding authority.

Cutover owners are `scripts/sorafs_topology_qualification.py`,
`build_sorafs_topology_qualification_envelope.py`, `run_sorafs_production_readiness.py`,
`check_sorafs_reference_sdk_release_evidence.py`, their canary/runner/qualification fixtures and
`check_sorafs_production_promotion_bundle.py`. Remove the old envelope schema, detached-signature
builder/loader and trust-tuple projection together; accept only the actual replacement. Preserve
bounded no-follow reads, independently pinned executable/trust inputs and exact digest binding.
Do not retain an alternate decoder or a migration acceptance flag.

## Scoped tests and qualification limits

The proposed Rust controls cover canonical subject/request/receipt roundtrips; distinct role and
purpose; all independently reviewed subject fields including candidate; whole signer binding;
authority key and policy; active head and revocation; trusted time; reservation/fence; completion;
ordered signatures and message binding; missing/oversized/retired frames; and generic provisioning
and native-adapter refusal before endpoint access. Synthetic signed state is used only to test
consistency. The missing native executed-result checks cannot be qualified by these fixtures.
Current source review and formatting do not replace compilation or execution of these new tests.
