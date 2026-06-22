# SoraFS Repair Fixture

This directory contains deterministic repair workflow fixture payloads for the
SF-11 reference validator.

- `task_v1.to` is the canonical Norito `RepairTaskRecordV1` payload.
- `task_v1.json` is a readable summary of the same fixture.

Regenerate both files with:

```sh
cargo run -p sorafs_manifest --bin generate_por_fixtures
```

