# SoraFS Repair Fixture

This directory contains deterministic repair workflow fixture payloads for the
SF-11 reference validator.

- `task_v1.to` is the canonical Norito `RepairTaskRecordV1` payload.
- `task_v1.json` is a readable summary of the same fixture.
- `negative/task_manifest_mismatch_v1.*` remains structurally valid but names a
  different manifest, producing the release-wide bundle's `SFS-BND-002`
  vector.
- `negative/task_provider_unassigned_v1.*` remains structurally valid but names
  a provider outside the replication order, producing `SFS-BND-003`.

Regenerate the complete fixture set with:

```sh
cargo run --locked --offline -p sorafs_manifest --features dev-tools --bin generate_por_fixtures -- --write
```
