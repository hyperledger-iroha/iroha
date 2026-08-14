Generated Taira lane manifests belong here when running the checked-in config
directly from the repository root.

Do not commit live validator account IDs, private keys, or runtime-only rollout
material. Use `scripts/render_taira_validator_bundle.py` with a populated local
validator roster to generate per-validator `manifests/governance.manifest.json`
files for the `universal` deployment cohort. That ordinary renderer does not
assemble the distinct `dpn`, `is`, `is2`, or `cbsi` physical cohorts. Their
deployment-owned dataspace manifests and validator/server bindings must be
supplied separately and must pass the public rollout topology/roster gate.
