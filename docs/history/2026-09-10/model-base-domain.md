# Domain identity compilation boundary (2026-09-10)

`iroha_model_base::domain::DomainId` owns the two private DNS labels, validating
constructors, normalization, declared Norito identity, bounded decoding,
canonical slice adapter, JSON object keys and `mv` storage keys. Domain entities,
registration, projections and `IdBox` composition remain in the aggregate.
The superseded aggregate root/prelude/module and IVM mock exports are removed.
The canonical envelope operation is `IdBox::from(domain).encode()`.

The owner preserves all 11 original tests and three private-field compile-fail
examples. An additional base test checks the seven captured DomainId frames,
including container/hash/signature identities, schema, JSON and storage keys.
The aggregate keeps its mixed-frame assertions. The immutable 189-row fixture
remains byte-identical, SHA-256
`9eaba83f63302101a16a324d07bd85bd316dfc1cea8eca7bbafb876e0b57342a`.

The coherent application covers 604 paths. The consumer stage changes 387
imports, 328 qualified paths and 205 scope imports, preserving 11,938 literal
test names and 45,147 assertions. Review covers 303 scope decisions, including
function-local imports, textual includes, conditional features and the retained
IVM pointer-type variant. Core account-query imports are unconditional because
their production parser needs the type. The three unrelated Norito fixture
types remain unchanged. The real Git index is unchanged.

Cargo.lock changes four local dependency lists without any external package or
version change. Base adds the existing workspace getset dependency; samples add
a normal base edge; FASTPQ and executor-data-model derive add test edges.
Integration tests promote their existing base edge to normal dependencies for
the fixture-refresh binary. The lock SHA-256 is
`da4a84fb916bf93e6faa6975175c2c6746e9ebc62c8be69904710423d2c68310`.
The canonical OpenAPI lock-pin generator and check pass. Its initial temporary
path rejection and a malformed check invocation remain recorded separately.

Exact dependency budgets reflect these edges, with no added headroom or layer
exceptions. The manifest fingerprint is
`sha256:13432d8c3686280d4689159ae5414ec046d825f91d8e1865e7e70e683d33a967`.
Workspace declared/required edges change from 1,719/1,587 to 1,723/1,591.
All 20 feature-resolved normal/build boundary checks pass.

Evidence is retained under
`target/architecture-redesign/model-base-extraction-v1/`. All four base feature
configurations (default, transparent, FFI and combined) pass 67 unit and three
allocation tests each, without failures or ignored tests. Default and transparent
use source
`62f799eecb6e6e01200d8529dca22352741098862760f8584b893ab78b290483`.
FFI and combined use source
`3e94d3bed70057cb107e07bc05a31f72186f31c4338d0a979d435758c718e100`;
strict library-and-test Clippy and all three private-field compile-fail examples
pass on that combined source. Workspace formatting, retired-codec guards and
120 guard-tool tests pass. The source-size check retains the same 249 affected
paths and 170 exceptions; migrated imports change some existing findings' line
counts. No exception was expanded. The generated-artifact registry also passes
against a private copy of the Git index containing the new source owners;
the real index remains unchanged.

The final 46-package all-target consumer check passes on source
`c9dfd3d70c42e75af5f2c7f01995994f5fd2b87690a0e7276402538423820938`
with all 9,276 selected inputs unchanged. Developer targets are enabled for
xtask, Mochi and FASTPQ, plus the executor-data-model derive UI harness. This
compiles that harness; its compiler subprocess cases have not run. All 147
earlier warning occurrences remain, plus three dead-code warnings from the
newly selected FASTPQ fixture helpers. The six new migration warning occurrences
are removed. Failed checks retain the missing visitor macro scope and the second
IVM re-export; both now use the canonical owner, with no compatibility path.
The final report is `domain-consumers-check-4` and the warning comparison is
`domain-consumer-warning-comparison-1`.

The composed library build and runtime pass on the same 5,663 unchanged inputs,
source `69cf733d700c512abe5e769cead45fd3105452f3e2aa66623ddfbbf2c545c8c2`:
67 base tests, 3,681 aggregate tests, 802 SDK tests and 386 Musubi tests. The
aggregate retains six ignored fixture-regeneration printers; Musubi retains
one subprocess-only crash worker exercised by its parent. There are no failures.
All eleven moved tests pass at the base owner, including the test previously in
`json_object_key::tests`; every common aggregate test retains its prior outcome.
Both captured aggregate wire-fixture checks and all 35 resolver regressions pass.
Musubi uses four ordinary workers with stack overrides cleared. Its iterative
search source remains unchanged. Runtime reports use the `domain-` prefix;
`domain-aggregate-test-inventory-comparison-1` accounts for the moved tests and
`domain-resolver-root-cause-checkpoint-1` binds the resolver source and artifacts.

Combined FFI and transparent-API execution passes 67 base and 3,681 aggregate
tests on unchanged source
`edcce373d24e53dfda2407e51d83fc72b77a1ae695d6094c27a26041afbe2c11`.
This includes the four metadata export/deallocator checks and both captured
aggregate fixture checks. SDK/Musubi strict library-and-test Clippy and all ten
SDK doctests pass on the composed source above. The separate base/aggregate
documentation run passes seven aggregate examples and three base compile-fail
examples on unchanged source
`877f7e8cc2d4e82dbba62fe3ef5e391e173adbeb9ab7a9ec320b71c1e97eadbe`.

A broader strict Clippy run also selecting the aggregate fails with 236
aggregate diagnostics. These remain unresolved and are recorded in
`domain-aggregate-clippy-diagnostics-1.json`; they are not counted as a passing
workspace lint result. Broader release, native/device and measured build-memory
qualification remain outstanding.
