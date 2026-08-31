---
title: SoraFS L1 Deployment Qualification Contract
summary: Non-secret, fail-closed topology qualification before genuine rollout evidence is collected.
---

# SoraFS L1 Deployment Qualification Contract

The L1 topology checker qualifies a deployment plan before operators collect
rollout evidence:

```bash
python3 scripts/check_sorafs_l1_deployment_qualification.py \
  --manifest /runtime/reviewed/sorafs-l1-topology.json \
  --deployment-id <reviewed-production-deployment-id> \
  --environment production \
  --summary-out artifacts/sorafs/l1-topology-qualification.json
```

The repository also ships a schema-complete, non-secret production topology
shape and matching response file:

```bash
python3 scripts/check_sorafs_l1_deployment_qualification.py \
  @scripts/examples/sorafs_l1_deployment_qualification.args.example
```

The example manifest is
`scripts/examples/sorafs_l1_deployment_qualification_manifest.json.example`.
Its four validators, two independently operated storage providers, two
regional gateways, two independently administered Governance DAG/Kubo
instances, runtime handles, model digests, and ordered 17 lane slots exercise
the complete closed schema. Its identifiers and digests are shape-only public
values, not a claim of a live deployment or genuine qualification evidence.
Operators must bind the checker to their reviewed production manifest and
command-line deployment context before collecting any lane summary.

The input uses schema `sorafs.l1.deployment_qualification.v1`. It is a
schema-closed, payload-free plan containing:

- the exact Taira network name, chain ID, numeric chain discriminator, and
  ordered `taira-validator-1` through `taira-validator-4` voting identities,
  each with DA and RBC enabled; Minamoto or any other network is rejected;
- between 2 and 64 unique SoraFS storage providers operated by at least two
  distinct operator identities;
- exactly two gateways with distinct region and administrator identities;
- exactly two Governance DAG instances with distinct Kubo runtime handles and
  administrator identities;
- distinct production runtime handles for monitoring, an authenticated external
  signer, a key-custody provider, and WebAuthn;
- between one and 64 signed model artifacts, each bound by a production
  identifier, positive revision, artifact digest, detached-signature digest,
  verified Ed25519 or ML-DSA-87 algorithm, and signer public-key fingerprint;
- an explicit policy stating that credentials and private material are absent
  from configuration and must be injected externally at runtime; and
- the canonical ordered 17-lane inventory from
  `check_sorafs_production_readiness.py`, with every slot bound to the same
  deployment ID and production environment.

The schema is closed at every level. `deployment` contains `deployment_id`,
`environment`, `network`, `chain_id`, and `chain_discriminant`; the final
three fields must equal the canonical public Taira constants. Validator rows contain `validator_id`,
`voting`, `da_enabled`, and `rbc_enabled`. Storage-provider rows contain
`provider_id` and `operator_id`. Gateway rows contain `gateway_id`, `region`,
and `administrator_id`. Governance DAG rows contain `instance_id`,
`kubo_handle`, and `administrator_id`. `runtime_handles` has exactly the
`monitoring`, `external_signer`, `key_custody`, and `webauthn` keys.
`runtime_material_policy`
sets `configuration_contains_credentials=false`,
`configuration_contains_private_material=false`, and
`external_injection_required=true`. `signed_model_artifacts` contains no model
bytes, signatures, credentials, or private material: each schema-closed row has
only `artifact_id`, `revision`, `artifact_sha256`, `signature_algorithm`,
`signature_sha256`, `signer_public_key_fingerprint_sha256`, and
`signature_verified=true`. Each lane row contains only `gate`, `deployment_id`,
and `environment`.

The checker accepts opaque non-secret handles only. It rejects unknown fields,
duplicate JSON keys, unsafe paths, secret-looking fields or values, test/mock
handles, aliases, missing topology members, shared gateway/DAG administration,
and lane/context drift. The deployment ID and environment must also match
independent operator-reviewed command-line values.

Success emits `status=configuration-qualified`,
`qualification_scope=pre-deployment-configuration`,
`live_evidence_recognized=false`, and `promotion_eligible=false`. Those values
are intentional: this artifact proves only that the proposed topology is
well-shaped. It is not a lane summary, is not accepted by the aggregate
promotion gate, and cannot replace the signed nine-prerequisite envelope or any
of the 17 genuine payload-free evidence summaries.

## Independently sign the exact qualification

Construct the signed companion with the public no-private-key workflow in
`scripts/build_sorafs_topology_qualification_envelope.py`. `prepare` revalidates
the exact summary, the Taira chain binding, all four ordered validator
identities, the review clock, and the external software-signer trust tuple. It
then emits a schema-closed prepared object and the domain-separated bytes for
the independently administered Ed25519 signer. `finalize` replays every signed
topology, trust, and review field and re-evaluates freshness under its supplied
clock and maximum-age policy before it accepts the signer's 64 raw detached
signature bytes. `verify` replays the summary and trust tuple again and emits
only the authenticated public binding.

The complete command sequence is in
[`scripts/examples/sorafs_l1_topology_qualification_envelope.md`](../../scripts/examples/sorafs_l1_topology_qualification_envelope.md).
The tool has no private-key, seed, or secret option and rejects such arguments,
including after response-file expansion. Use it only in an owner-controlled,
mode-0700 runtime directory. Outputs are owner-only, exclusively created, and
may not be symlinks or hardlinks. Run `verify` twice with the same explicit
clock and compare the results byte-for-byte before supplying the envelope to a
lane or aggregate runner.

For `prepare`, the prepared JSON is published first and the signing payload is
published last as the completion marker. A handled error rolls both back. An
abrupt host or process failure can leave the prepared JSON without its payload;
that incomplete state is not signable and must be removed before a fresh run.
The standalone `verify` command authenticates only the topology envelope and
does not claim promotion readiness: the foundational and aggregate readiness
flows additionally enforce signer-key and administrator separation from the
resilience, lane, and promotion domains.

Repository-root `artifacts/*` is ignored, so Git status cannot reveal an old
topology artifact. Never treat an existing ignored summary or envelope as
current. Regenerate and revalidate both from the exact reviewed manifest in a
protected runtime evidence directory for every release evidence collection.

The summary binds two manifest digests. `manifest_sha256` hashes the exact
reviewed manifest bytes, including their JSON formatting.
`canonical_manifest_sha256` hashes the schema-closed manifest after
deterministic JSON rendering. The former prevents a substituted byte stream;
the latter makes independently rendered but semantically identical input
visible during review. Every lane checker and collection runner requires the
exact summary through `--topology-qualification-summary`. A ready lane records
the SHA-256 of those exact summary bytes plus both manifest digests and the
reviewed deployment context. Missing, substituted, or mismatched qualification
input blocks the lane.

The independently signed topology companion is also schema-closed. Its
`signer_authentication_kind` is exactly `external-ed25519`, `signer_backend` is
exactly `software`, and `signature_algorithm` is exactly `ed25519`. The
signature covers the distinct service and administrator identities, positive
key and policy revisions, non-zero policy digest, public-key fingerprint, and
the exact topology binding. Local or non-software backends, same-identity
administration, and key reuse with a resilience, lane, or promotion signer fail
qualification even when a detached signature is otherwise valid. Iroha has no
HSM-specific backend or migration mode; deployment-owned custody behind the
external signer must not relabel this release.

The binding also carries the exact Taira network, chain ID, discriminator, and
SHA-256 of the canonical ordered four-validator identity array.
The same binding is signed inside the foundational prerequisite envelope,
beside the ordered 17 lane-summary digests. The aggregate checker and runner
also require the exact qualification summary and demand equality across the
qualification input, every present lane, the signed envelope, the aggregate
deployment context, and deterministic replay. The qualification summary
remains configuration-only evidence throughout this chain; passing it never
increments `summary_file_count` or `recognized_summary_count`.

The next local qualification attachment is the
[L1 resilience and disaster-recovery qualification](l1_resilience_qualification.md).
It binds partition, restart, rotation, failover, recovery, restore, rollback,
and package-yank rehearsal artifacts to this exact topology. Its trusted
summary digest and signer fingerprint are covered by the existing
nine-prerequisite external-software-signer envelope and independently
reverified by the aggregate and replay runner. It remains outside both the
17-lane summary count and the fixed nine prerequisite IDs.

## Remaining L1 work

Configuration qualification does not provision or test infrastructure.
Operators still must deploy the reviewed four-validator topology, exercise
DA/RBC and recovery, bring up independently administered gateways and
Governance DAG/Kubo instances, inject isolated authenticated signing,
key-custody, and WebAuthn providers from runtime-only secret stores, operate
multiple storage providers, complete the 1,000-stream and 24-hour soak
exercises, and collect one valid fresh summary for every lane. L2 remains
blocked until the trusted external software Ed25519 signer signs the ordered
nine-prerequisite envelope and both aggregate replays return the exact ready
counts. Until genuine deployment evidence exists, the honest readiness state
is `recognized_summary_count=0` of 17, regardless of a successful
configuration qualification.
