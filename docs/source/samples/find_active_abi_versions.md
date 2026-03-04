# FindActiveAbiVersions — Norito Query Schema (Sample)

Type name: `iroha_data_model::query::runtime::ActiveAbiVersions`

Fields
- `active_versions: Vec<u16>` — sorted list of ABI versions that are currently active on this node.
- `default_compile_target: u16` — highest active ABI version; compilers should target this by default.

Example Norito JSON response (first release; single ABI)
```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```

Query (signed) — `FindActiveAbiVersions`
- Route: `/query` (Norito-encoded `SignedQuery`)
- Singular query box variant: `FindActiveAbiVersions`
- Output variant: `ActiveAbiVersions`

Notes
- ABI version 1 is always active in the first release; governance‑activated versions extend the set permanently. The example above reflects the single‑ABI state.
