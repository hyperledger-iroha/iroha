# Generic query identity fixtures

The two immutable captures record actual compiler names, both directional codec
hashes, adaptive payload bytes and complete canonical frames before the query
identity declarations were added. Each family includes a root, a populated
vector, an option and a populated ordered map.

| Feature selection | Families | Frames | Fixture SHA-256 |
| --- | ---: | ---: | --- |
| Default | 24 | 96 | `bf9729bee6ebcf73579659cd0d088a5f3d630385d69ec7e75188432f8b1b32ab` |
| Default plus `ids_projection` | 29 | 116 | `78ecf99df5eff1eecba94498266d0b925ed01c88a076b40c77700535db9a7115` |

The files are `query_generic_full_identity_frames.json` and
`query_generic_ids_identity_frames.json`. Their existing feature-dependent
selector layouts are captured separately. The capture writer has been removed.

Five generic owners now declare nominal composition: `CompoundPredicate<T>`,
`SelectorTuple<T>`, `QueryWithFilter<T>`, `ErasedIterQuery<T>` and the private
`MembershipValues<T>`. Four concrete owners declare their captured identities:
`CommittedTxPredicate`, `SelectorMode`, `CommittedTransaction` and
`CertifiedMergeTransactionInclusion`. The two typed-hash markers `BlockHeader`
and `TransactionEntrypoint` use their names observed inside the original
membership frames; separate assertions check both existing codec hashes.

The values cover account metadata predicates, nested committed-transaction
predicates, all six membership literal types, nonempty erased query payloads,
full and ids-only selectors, and ordinary and merge-carried committed
transactions. The committed fixtures contain deterministic trigger instructions,
results and Merkle proofs. They establish wire behavior, not finality or proof
authorization.

Permanent tests compare every captured field, decode and re-encode the complete
frames, and reject wrong schema headers and truncation. Identical payloads under
different item markers must remain different typed frames, including their
containers. Identity-only markers have no payload codec or schema exporter.
The private membership decoder still requires its existing allocation budget;
a separate test checks rejection outside that scope and restoration on drop.

The default query selection passes 202 tests and `ids_projection` passes 203,
with zero failures or ignored tests. All 5,109 selected development inputs stay
unchanged during each run. The feature build also
corrected a stale bounded-JSON test to use the actual ids-only `SelectorTuple`
API, preserving the exact-bound and one-byte-too-small assertions.

Pre-declaration source inventories, successful capture transcripts, executable
hashes and candidate test checkpoints remain under ignored
`target/architecture-redesign/owned-storage-identity/query-generic-identity/`.
Each successful capture retains 5,106 unchanged selected inputs. This is scoped
host evidence; the atomic codec cutover, physical model moves, other supported
feature/target selections and release qualification remain open.
