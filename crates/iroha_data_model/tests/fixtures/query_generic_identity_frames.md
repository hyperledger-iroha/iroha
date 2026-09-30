# Generic query identity fixtures

The current canonical query captures are reproduced by the explicit ignored unit
`query::generic_identity_tests::capture_query_identity_frames_for_first_release_migration`
under each selected feature layout. Every family has root, populated vector, Some
and populated ordered map frames.

| Feature selection | Families | Frames | Fixture SHA-256 |
| --- | ---: | ---: | --- |
| Default | 23 | 92 | `66a36a6d0a10c05917ab9782817784926d8dc1d96dfa39ac31a4973b9edb47f9` |
| Default plus `ids_projection` | 28 | 112 | `de644c2f23dd85776b6fc82ff5652fe219f709a827449d6f1d95db17ea8b5a73` |

The files are `query_generic_full_identity_frames.json` and
`query_generic_ids_identity_frames.json`. Tests authenticate each complete file,
compare every recorded codec field, decode and re-encode complete frames, and
reject wrong schema headers and truncation. Equal payloads with different item
markers remain distinct typed frames in every container. Identity-only markers
have no payload codec or schema exporter.

The generic owners include `CompoundPredicate<T>`, `SelectorTuple<T>`,
`QueryWithFilter<T>`, `ErasedIterQuery<T>` and private `MembershipValues<T>`.
Concrete owners include `CommittedTxPredicate`, `SelectorMode`,
`CommittedTransaction` and `CertifiedMergeTransactionInclusion`. Typed-hash
markers preserve their explicit nominal identities. The private membership
codec keeps its allocation budget; tests check refusal outside that scope and
restoration on drop.

Values include metadata predicates, nested committed transaction predicates,
six membership literal types, erased query payloads, full and identifiers-only
selectors, and ordinary and merge-carried committed transactions. Supplied
transaction results and Merkle proofs are codec fixtures, not finality or proof
authorization.

The exact preceding captures and original documentation are preserved under
the September 30 historical record.
Earlier source inventories and component counts attest only their recorded
candidate. Historical frames are never current decoder inputs or compatibility
paths; current tests pin the current canonical wire layout.
