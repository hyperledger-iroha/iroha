# FindAbiVersion — Norito Query Schema (Sample)

Type name: `iroha_data_model::query::runtime::AbiVersion`

Fields
- `abi_version: u16` — the fixed ABI version currently accepted on this node.

Example Norito JSON response (first release; fixed ABI)
```json
{
  "abi_version": 1
}
```

Query (signed) — `FindAbiVersion`
- Route: `/query` (Norito-encoded `SignedQuery`)
- Singular query box variant: `FindAbiVersion`
- Output variant: `AbiVersion`

Notes
- ABI version 1 is fixed in the first release. The example above reflects the only supported ABI state.
