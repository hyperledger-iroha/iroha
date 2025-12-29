<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FRR Templates for SoraNet PoPs

This directory hosts operator artefacts referenced by roadmap task
**SNNet-15A2 – BGP policy & monitoring suite**:

- `examples/` – sample Norito JSON descriptors (e.g., `sjc01.json`) demonstrating
  how to describe a PoP.
- `generated/` – optional staging area for rendered FRR configs (CI jobs and the
  xtask helper default to `artifacts/soranet_pop/`, but storing snapshots here is
  convenient for playbooks/tests).

To render a config from an example descriptor:

```bash
cargo xtask soranet-pop-template \
  --input fixtures/soranet_pop/sjc01.json \
  --output deploy/soranet/frr/generated/sjc01.frr.conf
```

See `docs/source/soranet/bgp_policy.md` for the descriptor schema, BFD/RPKI
guidance, and dashboard references. Generated configs intentionally avoid
mutable placeholders so that CI can diff artefacts and operators can attach them
directly to rollout evidence bundles.
