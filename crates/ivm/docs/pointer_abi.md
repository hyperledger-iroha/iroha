//! Pointer‑ABI Types (IDs and Policies)

This document lists the sole admitted IVM pointer-ABI V1 type table. IDs absent
from this table are invalid, including the retired handle pointer ID 0x000C.

- Validation and policy mapping are centralized in `ivm::pointer_abi`.
- Unknown/forbidden types under a policy are rejected during TLV validation.

<!-- BEGIN GENERATED POINTER TYPES -->
| ID | Name | ABI v1 |
|---|---|---|
| 0x0001 | AccountId | OK |
| 0x0002 | AssetDefinitionId | OK |
| 0x0003 | Name | OK |
| 0x0004 | Json | OK |
| 0x0005 | NftId | OK |
| 0x0006 | Blob | OK |
| 0x0007 | AssetId | OK |
| 0x0008 | DomainId | OK |
| 0x0009 | NoritoBytes | OK |
| 0x000A | DataSpaceId | OK |
| 0x000B | AxtDescriptor | OK |
| 0x000D | ProofBlob | OK |
| 0x000E | SoracloudRequest | OK |
| 0x000F | SoracloudResponse | OK |
| 0x0010 | Quantity | OK |
| 0x0011 | Int | OK |
| 0x0012 | Decimal | OK |
| 0x0013 | AxtAnchoredSpendV1 | OK |
<!-- END GENERATED POINTER TYPES -->

Notes
- Column denotes whether the type is accepted under ABI v1 (the only supported policy in this release).
- ABI v1 now includes the Soracloud and AXT pointer types shown above; further additions require a deliberate ABI surface change rather than an in-place runtime upgrade.
- TLV structure is enforced regardless of policy; type IDs gate which categories are accepted for host syscalls.
- `DataSpaceId`, `AxtDescriptor`, `ProofBlob`, and `AxtAnchoredSpendV1` form the AXT pointer surface. B5 stages one canonical issuer-signed anchored spend. A handle is an internal field of that signed data-model value and has no standalone TLV type or reusable-handle syscall. Production State admission still rejects nonempty spends until finalized source authority and exact transfer occurrence are authenticated.
- `SoracloudRequest` and `SoracloudResponse` carry Norito envelopes for the Soracloud runtime host ABI. They are only meaningful on the dedicated Soracloud syscall block and remain part of ABI v1.
- `Int`, `Decimal`, and `Quantity` are distinct canonical numeric pointer
  domains. `Quantity` is the nonnegative nominal ledger domain used for source
  `quantity` values; hosts reject cross-typed pointers rather than aliasing
  numeric domains.
