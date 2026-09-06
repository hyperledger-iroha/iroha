# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-e7983b4b17c8d0619faa9592c8d0e72b5bbbd3b105abb48c0eea9f40224daaa4"></a>

<!-- Original context: Roadmap / JavaScript SDK first-release follow-up -->
- Replace the remaining synchronous module-scoped Norito length-flag stack with
  an explicit codec context, and move source-test runtime factories into
  non-exported internal modules. Generate declaration allowlists from the same
  reviewed runtime export inventory so root, browser, and subpath conditions
  cannot drift.


<a id="record-dff832d09341fa392ae65c593a3aaf0a166cb41d8c271453214a19c25dd9f53f"></a>

<!-- Original context: Roadmap / C# SDK release qualification -->
- Rebuild and stage the same-revision ABI-23 `connect_norito_bridge`, then rerun
  the 10 native privacy, SoraFS reference, and Hijiri tests that are blocked on
  the current host. Run formatting with the pinned .NET 8.0.419 SDK and repeat
  the full build, unit, integration, sample, and package corridor from one
  settled candidate.


<a id="record-2bf3d306cf0b260b41696e447ce2897472468605d139d1f34d4b0b3fc7d1742c"></a>

<!-- Original context: Roadmap / Default-on capability follow-up -->
## Default-on capability follow-up



<a id="record-3bad986251b3c63f51cc7b4d0193ab7088f460721f4c70e69fa9f007893b066c"></a>

- Promote compute from the standalone `xtask` harness into a governed Torii/Core
  runtime before enabling `compute.enabled`: add an authoritative manifest and
  contract target, stable catalog route, canonical authentication, bounded
  execution and replay state, real IVM metering, and a live Kiso pricing
  subscriber. The current configuration bit has no daemon consumer and must not
  advertise a capability that nodes cannot execute.


<a id="record-f9fd483996c5c9a52998d4926a23f5c7eb1c87c7d71e1e2315ae034b504fb1ed"></a>

- Upgrade the locked QUIC stack to a qualified `quinn-proto` release, rerun the
  transport security and four-validator loss/recovery matrix, and only then add
  QUIC to the portable daemon aggregate.


<a id="record-8d04b350673f279236744b72cc9ee1dbbf3c7babe1f5dc4faef24e30ebc2bb86"></a>

- Extend Kagami's prepared-Compose projection with an immutable, read-only
  prover-key directory per validator; remove the generated-profile prover
  opt-out after its custody and restart tests pass.


<a id="record-4bdaa3dca6d5e695e51bd7737e165b8f2b5709f05a5e535e60cbb8fc43495cb6"></a>

- During a separately authorized lockfile refresh, remove the retired
  OpenTelemetry exporter dependency tombstone from `iroha_telemetry`.



<a id="record-ddce2ae8d3fe35eee8217d2fb4ba6e5151b6f794b7ba1f31f1483540dc989f74"></a>

<!-- Original context: Roadmap / Telemetry first-release closure -->
- Add a production-source reachability gate for Prometheus metric families,
  structured-event exporter features, and public telemetry routes so
  definition-only observability surfaces cannot return without an explicit
  producer and consumer.


<a id="record-e573a1385f6014f938780ed89871c3d610d090de528cb8e910e21c1f1cc33602"></a>

<!-- Original context: Roadmap / Derive/proc-macro first-release closure -->
## Derive/proc-macro first-release closure



<a id="record-6afd2fbc3767bcc9f94fd01d00df243952d2afc4d2bc7b8c6e4c2e57e5d223fe"></a>

- Remove the remaining unused `darling` development pin from `iroha_derive`
  during the next lockfile-owned dependency refresh; the normal dependency
  graph no longer uses it.


<a id="record-81ff179d6d8ce177daaa037bc0d1689df025d068d07763af7c87e492d4ec7f45"></a>

- Replace the authenticated generated-emitter source copy with one shared
  implementation only in a dependency-edge and lockfile-owned change. Keep its
  source-integrity guard until all five consumer macro crates use that shared
  implementation.


<a id="record-8de43f75e0a5718b2e22c6be3b9a0533a4af58c6c8a6cfbb74f51d6bfd1644bb"></a>

- Add compile-time and generated-token-size budgets for the shipping derive
  feature sets, plus a production-source reachability check for helper methods
  and compatibility attributes, before making further caching or parsing
  changes.



<a id="record-243dce6e2c6cf9950584e13385c6ea2e844660946f127237a4aa8a3766525854"></a>

<!-- Original context: Roadmap / Mochi first-release closure -->
- Add a production-source reachability and dependency guard for Mochi exports,
  commands, routes, environment inputs, and helper binaries so compatibility
  aliases, test-only release branches, and unowned wrappers cannot return.


<a id="record-4e58d35136eafaa6805e00a0a969d9613b4cb9f1679b96925c472dd785e29338"></a>

<!-- Original context: Roadmap / Iroha configuration first-release closure -->
- Benchmark bounded TOML loading (including `extends`), typed TOML-to-Norito
  conversion, complete-config parsing, and public config projection. Add
  allocation and source-graph ceilings before introducing caches or changing
  merge order, and preserve byte-identical results across hardware.


<a id="record-9a64e40891502efba2175ae21cf5e544258d8f72e5db60e1694b5ccc446dd94e"></a>

<!-- Original context: Roadmap / Kagami first-release closure -->
## Kagami first-release closure



<a id="record-8a356da0a42b715aa4fd45bd7468350ad8c46647af381c461dbc73effffe15a3"></a>

- Re-run the complete Kagami unit/integration suite after the concurrently
  edited instruction registry/genesis state, consensus/profile commitments, and
  config fixtures settle. Follow with the multi-hour workspace suite on the
  release candidate; strict Kagami Clippy and checked-in generated-help equality
  already pass.


<a id="record-5806378c9fb7e10f0e8f4228ab37611d2fa059a2c97fdc66dbffc3c0280a2c45"></a>

- Add a production-source reachability check for Kagami commands, binaries,
  features, and direct dependencies so empty switches, unowned helper binaries,
  compatibility aliases, and duplicate serializers cannot accumulate again.
  Apply proven direct-dependency removals in a dedicated lockfile-owned change
  so `Cargo.toml` and `Cargo.lock` remain atomic.


<a id="record-9be036b71ca64185b67b84ab637221a868ea74a967765fd489a710476d311b17"></a>

- Benchmark batched Kura inspection, bounded codec conversions, localnet bundle
  generation, and prepared-genesis policy restaging. Add allocation ceilings
  and identical-output checks before caching equivalent validator-policy work or
  changing batch sizes.


<a id="record-e299bc6cf27c7b9662745e072fda68139996358ebc0b41d698c6380c6cb4c707"></a>

- Qualify owner-only key/config custody on every shipping host platform. Keep
  unsupported platforms fail closed until an equivalent no-follow, ACL/mode,
  single-link, atomic-publication implementation is available.



<a id="record-0af55b3566d65ddcdf36ff7bc4a492c7ad2cc25393832f77e03092ef73bdea06"></a>

<!-- Original context: Roadmap / SDK first-release closure -->
- Select one first-release transaction executable carrier in the data model and every SDK. Migrate
  instruction-only transactions and shared fixtures to it, then remove dual `instructions`/`entries`
  parameters and explicit batch-selection switches rather than preserving a legacy carrier.


<a id="record-3d36b79108068fcc3d88e974d895d6b1c0e7b4d7cefe57d2608254af5c6bb477"></a>

- Publish one reviewed, source-and-lock-authenticated ABI-23 artifact inventory for every release
  target. Local source-fingerprint-bound qualification now covers all five Apple slices, the full
  Swift package, Darwin JavaScript and Python native bindings, and the complete host C# suite; it is
  not artifact publication or cross-platform release evidence. Re-run every SDK against the signed
  candidate on its release OS/architecture matrix, add allocation benchmarks for the exact-size
  byte-field encoders, and retain byte-for-byte parity tests across implementations.



<a id="record-fd57eb39e3436e10a590a487f0dc701aa3e9d47f9a569dbea8680b0b64f59527"></a>

<!-- Original context: Roadmap / IVM first-release closure -->
## IVM first-release closure



<a id="record-956c595960c999212565a1d6617e70bbd12de5451b5369444d8c33857d4703f9"></a>

- Add a production-source closure check that rejects alternate instruction
  decoders or interpreters, empty feature switches, and unowned execution
  facades before they can accumulate in the release surface.


<a id="record-1cf56d01cd1b52497bcf3dccfcf9fc840dac8730b88bef08a0d11ad0558e6e8a"></a>

- Benchmark the canonical fixed-width interpreter, decode/prepared caches,
  register Merkle commitments, and ordered block scheduler. Add allocation and
  retained-byte budgets while pinning identical gas, traps, state, and proofs
  across thread counts and scalar or accelerated hardware paths.


<a id="record-24c27b6fa10e5b2cd2c712a0a2eed83c22e133557b8d5b32752637672b0ab947"></a>

- Split the remaining large VM and host implementations along their existing
  loader, execution, pointer-validation, metering, and proof ownership
  boundaries after tests pin those boundaries. Keep one execution semantics and
  avoid introducing another dispatch layer during the split.



<a id="record-bf8ff550d7e13d78c32e0c3fef33554fe586922aabde7d974a8f31466dbcb087"></a>

<!-- Original context: Roadmap / Norito archive API closure -->
## Norito archive API closure



<a id="record-0d216f595266723baa535412232858ad546844b757a45ea323e18a13f21eda0b"></a>

- Centralize short archived-field padding used by `decode_archived_field` and derive-generated
  `decode_from_slice` implementations in one fallible aligned-storage helper. Charge every
  temporary allocation at its allocation site and remove the duplicated `Vec` padding path.


<a id="record-964ab1f1f26286b52c6c93d8b2c048f398488aaf681f9e62a0d215c96e3e5c49"></a>

- Replace the stateful `from_bytes`/`ArchivedBox` marker workflow with an
  owning or borrowed archive view that carries its payload context explicitly,
  then migrate remaining callers to exact value decode. Remove the global
  payload-context setters once no caller can deserialize a marker after the
  validating function returns.


<a id="record-4936d54b9b6cb3e62500b31a902dca457136de540ceccc593bae46bbb713963b"></a>

- Audit the experimental columnar and broad streaming helper surfaces against
  first-release production call sites. Retain only layouts with an explicit V1
  wire contract and focused consumers; remove benchmark-only public adapters
  instead of preserving dormant APIs.



<a id="record-01cfec196832d8ad8acba4489e0a4b4f5c734c1c8a4c618501b3fb17535033e3"></a>

<!-- Original context: Roadmap / Data-model first-release closure -->
## Data-model first-release closure



<a id="record-65db49998eda6c34ef3ec5ef62fcea9e8d8ef29cbe0dbb2bb745489702573b7c"></a>

- Enact an enabled exact-network base validation-fee policy through SORA
  Parliament, wait the mandatory 120,960-block activation delay, and then
  qualify the signed native-Norito Hijiri quote and fee-bearing transaction
  path in a real four-peer network before calling it release-qualified. Cover
  global and per-account governance updates, missing-record defaults, direct
  live-multisig-signatory quote authorization, the selected nested fee-context
  binding, stale quotes, aggregate rounding, private failure envelopes, and
  behavior under message loss.


<a id="record-a505f48ebef4e9188ddc6cf56aba068106cf1e5308802248767ac19bba79208b"></a>

- From a clean committed candidate, rerun the canonical OpenAPI generation and
  signing workflow so its manifest and version-index provenance names the
  release source and all three Torii schema snapshots come from that one
  settled authority. A dirty-tree development pass is not release provenance.


<a id="record-b31f89be8d52223767fb41cae3c1f2aab30265b91a7781fe52ede222d3d06097"></a>

- Keep observer and evidence ingestion, peer reputation, registry credits,
  Hijiri checkpoints, and dedicated events or telemetry deferred until each has
  authenticated bounded ingress and one deterministic state owner. Define the
  exact evidence-path ingress semantics before exposing observer records to
  untrusted callers.


<a id="record-c79b70af1e97afdc2de2c175214dcc6540788e6c061a377b38dd6b73c5e01c9f"></a>

- Remove the always-rejected `RegisterProviderOwner` and
  `UnregisterProviderOwner` instruction surfaces from the registry, executor,
  fixtures, and SDKs instead of retaining inert pre-release operations.


<a id="record-785666121b3962fd9b9c2a8ed0e840cedef189b1b30c7a64ab40d145c7399f0d"></a>

- Remove the domain-selector state that canonical `AccountAddress` no longer
  serializes, then retire `AccountRekeyTransitionProvenance::LegacyUnspecified`
  and its startup normalization. Regenerate cross-SDK fixtures in the same
  change so there is one account model from the first release onward.


<a id="record-264004574fadf0faf08db7889727cbc0085dee263e58a534a9fbdd4635b1a8ae"></a>

- Replace the boxed repository-initiation compatibility codec with its direct
  enum payload, plan committed-transaction indexes from the typed predicate
  tree instead of the legacy flat filter carrier, and validate DA proof-policy
  hashes through borrowed bundles without temporary vectors.



<a id="record-000d4cd483e1696521c6e0876b9d224b1e4185f51dcc29fa4673fa3bd7142bfe"></a>

<!-- Original context: Roadmap / Native Torii MCP release qualification -->
- Finish the explicit curated capability registry: audit additive-only mutation
  declarations, preserve orthogonal operation/authority/mutation/retry/world/
  sensitivity/signing semantics, and keep public writer profiles limited to
  reviewed `iroha.*` capabilities with operator authority hidden.


<a id="record-b356408e96340f2733649ae40b496f59b31d9d9d12a1b1f02eca5512596b68d1"></a>

<!-- Original context: Roadmap / Repository structure follow-ups -->
- Validate the reduced Norito/data-model frontend on representative lower-memory
  macOS hosts and under a Linux cgroup hard limit. Keep future reductions inside
  bounded modules and derive-family isolation rather than adding crates or
  feature fragmentation.


<a id="record-a04fc5968318b72e3e3f1e5d73556e60aa16bc569c82521b9eaf6b45cc888931"></a>

- Keep Norito RPC fixture generation behind the sole create-only
  `xtask norito-rpc-fixtures --output-root <absent-absolute-external-root>`
  owner and its internal `norito_codegen_exporter` library, followed by the
  sole `xtask norito-rpc-verify` checker. SDKs and CI are consumers of those
  outputs; do not add SDK-local generators, generic exporter binaries,
  compatibility commands, or migration adapters.



<a id="record-15db3f050d790218d9495928153255c652f0f39eca564f86b88fd1be9c19cce6"></a>

<!-- Original context: Roadmap / Memory-containment follow-ups -->
- Repeat the current cold workspace and focused data-model profiles on a
  representative lower-memory Mac and under a Linux cgroup hard limit. Add a
  serialization-throughput benchmark for nested writer-backed values so future
  frontend reductions retain both deterministic bytes and runtime performance.
  Keep native Cargo jobserver parallelism as the default; use the documented
  opt-in constrained runner only on hosts that actually require serialization.

Mixed-executable-batch follow-up is limited to completing the full workspace
suite and the complete platform SDK suites on toolchains with their required
native bridges and runtimes available.



<a id="record-29220a05cdb26c26b15390a5b2f77f6df96907b3ddb5d8207ccff70088916db7"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
## Release and Stabilization

**Status:** active.



<a id="record-3601299d6cfe9b5ccdeb17df00e724117eef0eadbf788dd7222f01d4cf9148d2"></a>

- Keep active Core verifier-profile filters in first-release terminology:
  retired unqualified vote and anonymous-transfer profile roots must stay
  rejected through the native Halo2/Pasta production backend classifier, and
  the policy guard's
  `--negative-control-core-retired-backend-profile-wording` mode proves the
  old `is_legacy_*_backend_profile` helper names cannot return.



<a id="record-02ca4c2a6b1fb3bd83a8557a8e0c43ce67f9907a16929bc7c46604cb7e607505"></a>

- After the next signed Taira rollout, capture a four-validator restart
  benchmark proving that the single bounded-parallel Kura finality audit is
  reused by retained-record validation, replay planning, and active-height
  recovery. The benchmark must record audit/init time, time to Torii readiness,
  and time to consensus alignment; it must also confirm that the startup-only
  identity inventory is cleared before runtime while the fixed-size runtime
  finality cache remains bounded.


<a id="record-1642cd2849541dd64b602aa4ad2937cfd1b12850645d4a61fcd9f666556fff16"></a>

- Move the shared Iroha 2 / Iroha 3 codebase toward a broadly consumable
  release with clear release notes, SDK parity, and operator documentation.


<a id="record-c812aaa480809408216ac81f38eddef4e4d6cc74c70e47511cc740eda725e591"></a>

- CUDA acceleration release readiness now has focused Norito helper, IVM CUDA
  grouped-test, scheduler GPU fallback/adversarial, FASTPQ batched Poseidon
  parity/negative, strict workspace warnings-as-errors clippy, workspace build,
  and workspace test no-run coverage on the local CUDA toolchain. Remaining
  release work is to keep those gates in CI and extend the hardware matrix to
  the production GPU/driver profiles before relying on CUDA in validator fleets.


<a id="record-301a9655342d5f3f536806d2121675f14ef2dd2135cbc72c0b389afb77d6b2de"></a>

- The C# SDK package release corridor now has a reusable isolated
  package-consumer guard that installs the packed NuGet artifact, rejects
  project-reference fallback, and runs managed API smoke checks. The C# PR
  workflow now packs into the guarded artifact directory and runs that consumer
  smoke after packaging. The SCCP production-corridor `dotnet-sdk` workflow
  route now targets `windows-latest`, installs the pinned Rust `1.93.1`
  MSVC toolchain before Rust cache restore, and uses Bash execution so it can
  produce native `connect_norito_bridge.dll` evidence. The Windows `.NET`
  SCCP artifact is now collected and verified on `.NET 8.0.422` / `win-x64`:
  the bridge DLL built with a recorded SHA-256, the native loader path was
  first in `PATH`, `dotnet test` used a Windows-local artifacts directory,
  VSTest reported 43 SCCP tests passed with zero failures or skips, and the
  direct TRX file plus positive byte count were validated by the corridor.


<a id="record-8b43c81cbe1f700212946213412c53206f0ae7a3181d7b2a8f56449584468dea"></a>

- Nexus public-lane validator state now treats `(lane_id, validator)` storage
  keys as authoritative ownership for live rosters and staking economics:
  mismatched persisted rows are ignored or rejected across topology inference,
  stake snapshots, election profiles, due activation, released exits, direct
  activation/rebind/exit/bond/unbond mutations, reward bookkeeping, slash
  handling, penalty lookup, staking admission, peer/account cleanup guards,
  multisig account-rekey rewrites, Soracloud runtime authority, host-finance
  stake accounting, and the Torii public-lane validator app API. Public-lane
  reward-claim, commit election profile, multisig account-rekey, slash handling,
  and Torii stake-share/reward app API paths now apply the same exact-key rule to
  `(lane_id, validator, staker)` shares and `(lane_id, epoch)` reward records
  before consuming or exposing account-facing economic state. Slashing also
  stages validator and share mutations until exact stake-share rows can satisfy
  the whole slash, with self stake debited through exact self-owned share rows
  before delegator rows. Canonical state snapshot export now applies the same
  filter before serializing public-lane validator, stake-share, and reward rows,
  so stale mismatched storage cannot be normalized into live rows on restore.
  Account, asset-definition, and domain unregister guards now apply the exact-key
  rule before treating public-lane stake-share or reward rows as cleanup
  blockers, so malformed ignored rows cannot keep unrelated state undeletable.
  Remaining Nexus scale-out work should keep new validator lifecycle, economic,
  and autoscale paths on the same key/record invariant.


<a id="record-a09e536f3bc9e0297db226a0714a19991a9ef060a055b938647eba491a4540cd"></a>

- Public-lane staking authority is now part of that invariant: validator
  registration/exit require the validator account, registration initial stake
  must be validator-owned self-stake, and bond/schedule-unbond/finalize-unbond
  require the staker account. Keep future staking and autoscale economics on
  this owner-authorized mutation boundary.


<a id="record-99a1eb9eea005fbe23ea18aab31d976f1769bb3b25e485f8462dd9ed40edc3ec"></a>

- Public-lane validator reset cleanup now pins both storage-key and embedded
  record lane ownership even when the storage-key validator account and embedded
  validator account disagree, so stale mismatched validator rows cannot retain
  live scope or survive lane destruction.


<a id="record-5e8fbf74dbab293c6dd8b28869ba34d61ae9dc5332a97cc2a17d8e82563737e4"></a>

- Future-created autoscale repair-retire coverage now proves invalid managed
  lanes block unrelated lifecycle changes without side effects, while the
  explicit repair retire prunes stale future-lane DA pin-intent indexes and
  emergency overrides and preserves active default-lane state.

