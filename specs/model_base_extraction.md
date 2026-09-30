# Foundational model extraction inventory

`iroha_model_base` owns `Name`, `StatePath`, `ParseError`, `Metadata`, `ChainId`,
`DomainId`, topology records and `PeerId`, including their validation, schema,
FFI and storage-key implementations. Consumers import these canonical owners;
retired aggregate and SDK facade paths are removed. `NetworkId` and `IdBox`
remain at ledger composition. Compose a domain identity with
`IdBox::from(domain).encode()`.

Owner tests move with their implementation; entity/composition tests remain in
`iroha_data_model`. Captured wire frames, malformed-input assertions and FFI
behavior remain required across supported feature configurations. Ordinary
iterative JSON `Value` destruction must remain bounded on ordinary stacks;
cleanup callbacks and codec compatibility paths are not accepted substitutes.

The [architecture redesign](first_release_architecture_redesign.md) and
[canonical identity contract](norito_schema_identity.md) govern remaining
extraction and acceptance. Physical extraction, the required measured compiler
memory reduction, aggregate feature coverage and full release qualification
remain open. Test totals from previous source revisions do not qualify the
current candidate.
