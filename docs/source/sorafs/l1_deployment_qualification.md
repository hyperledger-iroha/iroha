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

The input uses schema `sorafs.l1.deployment_qualification.v1`. It is a
schema-closed, payload-free plan containing:

- exactly four unique voting validators, each with DA and RBC enabled;
- between 2 and 64 unique SoraFS storage providers operated by at least two
  distinct operator identities;
- exactly two gateways with distinct region and administrator identities;
- exactly two Governance DAG instances with distinct Kubo runtime handles and
  administrator identities;
- distinct production runtime handles for monitoring, HSM, KMS, and WebAuthn;
- an explicit policy stating that credentials and private material are absent
  from configuration and must be injected externally at runtime; and
- the canonical ordered 17-lane inventory from
  `check_sorafs_production_readiness.py`, with every slot bound to the same
  deployment ID and production environment.

The schema is closed at every level. Validator rows contain `validator_id`,
`voting`, `da_enabled`, and `rbc_enabled`. Storage-provider rows contain
`provider_id` and `operator_id`. Gateway rows contain `gateway_id`, `region`,
and `administrator_id`. Governance DAG rows contain `instance_id`,
`kubo_handle`, and `administrator_id`. `runtime_handles` has exactly the
`monitoring`, `hsm`, `kms`, and `webauthn` keys. `runtime_material_policy`
sets `configuration_contains_credentials=false`,
`configuration_contains_private_material=false`, and
`external_injection_required=true`. Each lane row contains only `gate`,
`deployment_id`, and `environment`.

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

## Remaining L1 work

Configuration qualification does not provision or test infrastructure.
Operators still must deploy the reviewed four-validator topology, exercise
DA/RBC and recovery, bring up independently administered gateways and
Governance DAG/Kubo instances, inject real HSM/KMS/WebAuthn dependencies from
runtime-only secret stores, operate multiple storage providers, complete the
1,000-stream and 24-hour soak exercises, and collect one valid fresh summary
for every lane. L2 remains blocked until the external HSM signs the ordered
nine-prerequisite envelope and both aggregate replays return the exact ready
counts.
