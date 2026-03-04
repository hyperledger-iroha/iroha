# NSC-55 rANS Comparison Artefacts

- CSV export: `benchmarks/nsc/rans_compare_refs/rans_tables.csv`
- Logs: `benchmarks/nsc/rans_compare_refs/logs`
- Report: `benchmarks/nsc/rans_compare_refs/report.json`
- Clip manifest: `benchmarks/nsc/reference_clips.json`
- Norito entropy bench (optional): `norito_entropy.json` when requested.

These files are generated via `benchmarks/nsc/rans_compare.py`. Reference runners can be
supplied with `--runner label="command"`. The script exposes the following placeholders
inside each command:

| Placeholder | Description |
|-------------|-------------|
| `{csv}`   | Absolute path to the generated CSV export. |
| `{tables}`| Path to the SignedRansTablesV1 artefact. |
| `{clips}` | Canonical clip directory (only when `--clips-dir` is provided). |
| `{output}`| Root directory for this comparison run. |

Commands execute inside the repository root with `RANS_TABLES_CSV`, `RANS_TABLES_PATH`,
and (optionally) `RANS_REFERENCE_CLIPS` exported for convenience.
