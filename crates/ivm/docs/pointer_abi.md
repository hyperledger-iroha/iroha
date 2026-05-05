//! Pointer‑ABI Types (IDs and Policies)

This document lists IVM pointer‑ABI types, their stable numeric IDs, and which
ABI policies allow them. IDs are wire‑stable and never renumbered. New types are
added with new IDs; existing IDs must not change.

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
| 0x000C | AssetHandle | OK |
| 0x000D | ProofBlob | OK |
| 0x000E | SoracloudRequest | OK |
| 0x000F | SoracloudResponse | OK |
<!-- END GENERATED POINTER TYPES -->


Notes
- Column denotes whether the type is accepted under ABI v1 (the only supported policy in this release).
- ABI v1 now includes the Soracloud and AXT pointer types shown above; further additions require a deliberate ABI surface change rather than an in-place runtime upgrade.
- TLV structure is enforced regardless of policy; type IDs gate which categories are accepted for host syscalls.
- `DataSpaceId`, `AxtDescriptor`, `AssetHandle`, and `ProofBlob` underpin the AXT (atomic cross-transaction) flow. Default and WSV hosts fully validate these pointers when servicing AXT syscalls, ensuring descriptor membership, capability binding equality, and proof material are honoured.
- `SoracloudRequest` and `SoracloudResponse` carry Norito envelopes for the Soracloud runtime host ABI. They are only meaningful on the dedicated Soracloud syscall block and remain part of ABI v1.
