# Generic query identity fixtures

The current canonical query captures are reproduced by the explicit ignored unit
`query::generic_identity_tests::capture_query_identity_frames_for_first_release_migration`.
`SelectorTuple<T>` has a single data-free wire layout, so one capture covers every
build. Every family has root, populated vector, Some and populated ordered map
frames.

| Families | Frames | Fixture SHA-256 |
| ---: | ---: | --- |
| 23 | 92 | `fd10dd777565d2e3cfc05c8a1e6b8da7852139432255e290378709d66d7861d6` |

The file is `query_generic_full_identity_frames.json`. Tests authenticate the complete file,
compare every recorded codec field, decode and re-encode complete frames, and
reject wrong schema headers and truncation. Equal payloads with different item
markers remain distinct typed frames in every container. Identity-only markers
have no payload codec or schema exporter.

The generic owners include `CompoundPredicate<T>`, `SelectorTuple<T>`,
`QueryWithFilter<T>`, `ErasedIterQuery<T>` and private `MembershipValues<T>`.
Concrete owners include `CommittedTxPredicate`, `CommittedTransaction` and
`CertifiedMergeTransactionInclusion`. Typed-hash
markers preserve their explicit nominal identities. The private membership
codec keeps its allocation budget; tests check refusal outside that scope and
restoration on drop.

Values include metadata predicates, nested committed transaction predicates,
six membership literal types, erased query payloads, the empty selector, and
ordinary and merge-carried committed transactions. Supplied
transaction results and Merkle proofs are codec fixtures, not finality or proof
authorization.

The exact preceding captures and original documentation are preserved under
the September 30 historical record.
Earlier source inventories and component counts attest only their recorded
candidate. Historical frames are never current decoder inputs or compatibility
paths; current tests pin the current canonical wire layout.
