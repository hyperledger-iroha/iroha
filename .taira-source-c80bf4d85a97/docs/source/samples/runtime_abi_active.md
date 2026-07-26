# Runtime ABI — Active Version (Torii)

Endpoint
- `GET /v1/runtime/abi/active`

Response (first release; fixed ABI)
```json
{
  "abi_version": 1
}
```

Notes
- The first release exposes a single fixed ABI version, so this endpoint always returns `1`.
