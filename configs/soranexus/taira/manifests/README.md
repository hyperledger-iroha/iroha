Generated Taira lane manifests belong here when running the checked-in config
directly from the repository root.

Do not commit live validator account IDs, private keys, or runtime-only rollout
material. Use `scripts/render_taira_validator_bundle.py` with a populated local
validator roster to generate per-validator `manifests/governance.manifest.json`
files for deployment bundles.
