# Runtime ABI — Active Versions (Torii)

Endpoint
- `GET /v2/runtime/abi/active`

Response (first release; single ABI)
```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```

Notes
- The list is sorted ascending. The default compile target is the highest active version (1 in the first release).

