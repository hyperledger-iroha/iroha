# JavaScript qualification source core

`sorafs_javascript_qualification_source.py` owns the pure content relation for
one private, closed suite/fixture core. This is separate from the portable
[package and installed-content relations](javascript_original_archives_v1.md).
It does not install or import an SDK, execute tests, authenticate a release
candidate, or emit an execution/approval report.

The source-owned catalog at
`scripts/fixtures/sorafs_javascript_qualification_sources_v1.json` fixes:

- The six existing shared registration modules, the pure native requirement
  helper, the canonical assertion contract and copied SDK package metadata.
  Their nine raw byte identities are pinned; the 172 assertions, 46 top-level
  cases and nine nested cases are unchanged.
- Exactly 182 fixture names: the complete SoraFS manifest and multi-peer parity
  trees, plus `fuzz/sorafs_chunker/sf1_profile_v1_input.bin`. Fixture contents
  remain original bytes supplied by the independently authenticated candidate
  owner; a projection or caller dictionary cannot establish that authority.

Every catalog name must be present, and no other name is accepted. Source SDK
implementations, the eager source native helper, additional package metadata,
node_modules, caches and aliases are not members. The orchestrator metadata
must select the sole fixed payload path and declare its exact positive integer
byte length. Its JSON is duplicate-free. The catalog itself is a bounded,
byte-pinned source input, not caller-selected policy.

The core admits exactly 191 files, no file over the existing 16 MiB source
ceiling and no aggregate above 64 MiB. Names use the existing bounded shared
archive namespace policy. Cardinality, names and bytes are checked before
set/sort/hash allocations. Frozen projections rederive their content relation
before use; copied files require identical bytes and mode0644.

`OriginalQualificationTree` validates that source relation before invoking the
shared `OriginalTree` physical implementation. `OriginalInstalledTree` separately
validates the original package/dependency relation before using the same kernel.
There is one physical traversal implementation and no fake npm source projection
or compatibility implementation. `TreeMember` is a physical expectation, not
an authentication credential.

The POSIX kernel holds the root and ancestor descriptors, opens descendants
relative to held directories without following links, and compares exact
content, mode, owner, single-link status and original identity. It rejects
unowned leaves and directories. Hashing uses 64 KiB chunks. Its fixed maximum
is 8,192 files, 32 MiB per member, 192 MiB total and 64 path components; each
content projection retains its own smaller admission limits. Temporary leaf
and directory descriptors close after use; root/ancestor handles stay held
through recheck and close. Refusal/reentrancy poisons the owner, and cleanup
attempts each detached descriptor once even if an earlier close reports failure.

Initial/final tree seals establish those observations. They do not detect an
intervening change fully restored between observations, authenticate mapped
native memory, or establish which modules executed. Parent original/candidate
custody and the [fixed child](javascript_child_bootstrap_v1.md) retain those
distinct obligations. The child connects the closed core to installed subjects,
the same prepared session, native cache/files, final hook and stream EOF; its
loader observations are not independent final compiled-byte authority.

TODO: complete the source-owned parent process, runtime/native input and complete
output joins, original-index adapter, signatures and matching-candidate
qualification. The fixed child has only component/static validation; actual
installed SDK/native execution remains required.
