//! Pointer‑ABI TLV Examples (Norito)

TLV Layout (INPUT region)
- `type_id: u16` (big‑endian)
- `version: u8`
- `len: u32` (big‑endian)
- `payload: [u8; len]` (canonical Norito encoding for the type)
- `hash: [u8; 32]` (hash of `payload`; algorithm per type)

Examples (hex, spaces added for readability)

- AccountId (type_id=0x0001, version=1)
  - Payload (UTF‑8): canonical encoded account literal (IH58 or compressed), for example:
    - `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`
  - TLV (no hash filled):
    - `00 01` | `01` | `<len:be u32>` | `<utf8 bytes of encoded literal>` | `00..00 (32 bytes)`

- AssetDefinitionId (0x0002, v1)
  - Payload: `"rose#wonderland"` → `726f736523776f6e6465726c616e64`
  - TLV: `00 02 01 00 00 00 10 72 6f 73 65 23 77 6f 6e 64 65 72 6c 61 6e 64 00..00`

- Name (0x0003, v1)
  - Payload: `"cursor"` → `637572736f72`
  - TLV: `00 03 01 00 00 00 06 63 75 72 73 6f 72 00..00`

- Json (0x0004, v1)
  - Payload (canonical JSON): `{"query":"sc_dummy","cursor":1}` → `7b227175657279223a2273635f64756d6d79222c22637572736f72223a317d`
  - TLV: `00 04 01 00 00 00 23 7b 22 71 75 65 72 79 22 3a 22 73 63 5f 64 75 6d 6d 79 22 2c 22 63 75 72 73 6f 72 22 3a 31 7d 00..00`

- NftId (0x0005, v1)
  - Payload: `"rose:uuid:0123$wonderland"` (example) → `726f73653a757569643a3031323324776f6e6465726c616e64`
  - TLV: `00 05 01 00 00 00 19 72 6f 73 65 3a 75 75 69 64 3a 30 31 32 33 24 77 6f 6e 64 65 72 6c 61 6e 64 00..00`

- AssetId (0x0007, v1)
  - Payload: Norito-encoded `AssetId { definition, account }` bytes.
  - Textual API representation is canonical encoded form: `norito:<hex>`.
  - TLV: `00 07 01 <len:be u32> <payload bytes> <hash:32>`

- DomainId (0x0008, v1)
  - Payload: `"wonderland"` → Norito-encoded `DomainId`
  - TLV: `00 08 01 <len:be u32> <payload bytes> <hash:32>`

Hash field
- Fill with the 32‑byte hash of `payload` according to the pointer‑ABI type table (e.g., blake2b‑32 for AccountId/AssetDefinitionId/Name/NftId; blake2b‑32 over canonical JSON bytes for Json). The VM validates TLVs on first dereference: version must be 1, `type_id` must be known, envelope must lie entirely in INPUT, and the hash must match. Invalid TLVs trap with `VMError::NoritoInvalid`.
