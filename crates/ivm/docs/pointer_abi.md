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
<!-- END GENERATED POINTER TYPES -->


Notes
- Column denotes whether the type is accepted under ABI v1 (the only supported policy in this release).
- Runtime upgrades must use the existing type set; no new pointer‑ABI types are introduced in v1.
- TLV structure is enforced regardless of policy; type IDs gate which categories are accepted for host syscalls.
- `DataSpaceId`, `AxtDescriptor`, `AssetHandle`, and `ProofBlob` underpin the AXT (atomic cross-transaction) flow. Default and WSV hosts fully validate these pointers when servicing AXT syscalls, ensuring descriptor membership, capability binding equality, and proof material are honoured.
