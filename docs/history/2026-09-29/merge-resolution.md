# First-release merge resolution — September 29

This record concerns the merge of `2478995058` into `d7dfb1eaa8`, initially
containing 209 unmerged paths. It does not supersede the separate
[earlier merge validation](merge-validation.md) or qualify a release.

## Decisions

- Keep the native Sumeragi core and node driver as the sole consensus path.
  Retire old V2 driver, QueuePlan, autonomous-lane and caller-authored finality
  fixtures instead of restoring compatibility owners.
- Execute signed genesis and successor proposals through the actual native
  executor, certificate and publication path in positive chain fixtures.
  Tests for malformed proofs remain explicitly structural negative controls.
- Keep raw Kura reads separate from authenticated execution custody. Strict
  canonical corruption checks preserve the original bytes and reject damage.
- Keep consensus fault injection in the deterministic simulator. The separate
  private-settlement HTTP route controller uses a dedicated non-shipping daemon,
  exact command digests, protected files and serialized command revisions.
- Bind supervised snapshot export and storage-budget maintenance to successful
  native recovery, revoking publication after worker failure. Nonempty snapshot
  World restoration still requires original genesis-backed execution: current
  native results commit witnessed writes rather than the complete World.
- Source-bound release bundles contain the four shipping executables only.
  HTTP route-control, Parliament fixture and disposable-broker binaries are
  excluded from release bundle resolution.
- Preserve canonical Norito framing, domainless account identities and the
  current transaction payload, without retired admission-intent or lane-relay
  compatibility fields.

## Completed scoped evidence

| Check | Result |
| --- | --- |
| Unmerged index entries | 0 |
| Production Core, Torii, CLI, Kagami and daemon check | Passed before subsequent fixture/tooling repairs |
| Configuration tests | 256 passed |
| Sumeragi simulator unit tests | 377 passed, 2 ignored after the initial shared verification refactor; later size refactors pending |
| Deploy tests | 64 passed |
| Signature tests | 245 passed, 1 ignored |
| RAM program tests | 5 passed |
| Hash marker and schema regressions | Passed |
| OpenAPI and static contract scripts | 86 passed, 19 subtests after retired schema and asset cleanup |
| Release bundle custody and shell tests | 55 passed |
| Versioned wire identity tests | 5 passed |
| Izanami matrix scripts | 11 passed |
| Atomic route/session scripts | 129 passed, 8,645 subtests; 13 native-wheel-dependent cases awaiting rebuilt wheel |
| Core test compilation | Passed before subsequent snapshot lifecycle integration |
| Retired-codec guard | Passed |
| Maintained historical archive | Verified 64,736 records and 67,311 occurrences |

These results are scoped to their recorded checks and source point. A complete
workspace test run, current full release qualification, hardware qualification,
Swift native bridge parity and Kotlin/JVM validation are not established here.
The available Swift bridge advertises ABI 21 while the current SDK requires 25;
the JVM runtime is unavailable in this environment.

## Validation in progress

The merged Core/Torii test graph and daemon/network harness are being rebuilt
against native fixtures. The Sumeragi production line budget remains 8,000;
shared ownership and codec duplication is being reduced without raising the
budget or excluding production code. Final unchanged-source checks will be
recorded here after they complete.

The release source inventory still names retired owners and is not current
qualification evidence. Native full-body transport does not supply the required
signed RS16 PayloadManifest/PayloadChunk availability proof; that qualification
must fail closed until the actual integration exists, as recorded in
`specs/sumeragi_goals.md`.
