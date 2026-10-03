# iroha_deploy

The shared deployment engine for Kagami and Mochi developer environments and
operator network/dataspace workflows. This crate owns native localnet generation,
persistent supervision, context selection, definition parsing and finality verification. `iroha_cli` wires
`iroha dataspace plan/apply/status` to the existing native deployment engine;
the broader network/host engine remains in the phase plan below.

The design, file formats and phase plan are in
[`specs/network_deployment.md`](../../specs/network_deployment.md).

What exists so far:

- `managed`: workspace-scoped private contexts, four-validator process ownership,
  authenticated native control IPC, durable stop/restart/reset and signed readiness.
  `ManagedStore::up` retains the same generation and signer across starts; readiness
  requires one signed transaction to be applied by every validator. Kagami and
  Mochi call this owner directly. Explicit deployment/recovery targets preserve the
  workspace selection; ordinary startup and default auto-creation select their environment.
  Windows uses owner-restricted native named pipes;
  native Windows lifecycle execution still requires qualification.
  A new generation renders its final paths in private staging, then publishes its
  complete keys, configuration and immutable manifest in one native directory rename.
  Interrupted unpublished staging is discarded under exclusive operation ownership;
  a visible generation with missing metadata fails closed and is never regenerated.
  `up_dataspace` uses an independently installed parent profile and a sixty-second
  foreground budget. The same supervisor owns private validators and bounded outbound
  provisioning/relay turns; parent failure leaves private execution running. Remote
  bindings and release watermarks survive local reset, which cannot replace their child
  identity. Status separates live local peers from historical verified parent receipts.
- `localnet` and `genesis`: canonical configuration generation, independent private
  authority seeds, profile policy and genuine core genesis execution. CLI parsing
  stays in Kagami; generation and process ownership have no CLI or GUI dependency.
- `bootstrap`: canonical release-signed native checkpoint verification against an
  independently installed authority, with private durable release/clock rollback
  protection and explicit network reset identity. `ParentFinalityStore` retains
  the advancing native prefix separately and publishes before returning fresh
  readiness; reopening an older release never rewinds it. Official Taira release-key
  installation, artifact publication and combined parent-provisioning qualification remain outstanding.
  Native installation profiles and bounded unsigned HTTPS retrieval select the release
  independently. Signed account profiles, committee endpoints and optional faucet
  allowances feed exact parent SDK contexts without forwarding child credentials.
  Parent profiles require a committed global root. Signed native World schema and
  optional build-registry roots select cold-download discovery; a fresh committee
  observation precedes authentication of the provider policy and token signer.
  See the [developer acceptance contract](../../specs/kagami_mochi_devex_goals.md).
- `attachment`: exact compact parent registration/anchor operations using the shared
  wallet journal, explicit fee ceilings and fresh independent parent verification.
  Pre-dispatch checkpoints and original signed operations survive timeouts and restart;
  only verified parent ordinary-write receipts advance the anchored child cursor.
  Replay retains at most sixteen verified successors per turn and completes the receipt
  and pending marker in one atomic publication. `relay_once` connects owner-token
  loopback certificate reads to this workflow without forwarding private bodies or credentials.
- `provisioning`: retains the exact child registration, imports its owner into a
  separate public parent wallet, and coordinates the canonical faucet and paid SNS
  operations under signed release allowances. Fresh parent quorum precedes funding.
  Recovery preserves signed journals and the original namespace request; an unsigned
  quote may refresh only before journal creation, within the same spending allowance.
  Its state lives outside local reset, so a replacement child cannot silently inherit
  an existing parent attachment. After namespace completion, a fresh certified parent
  World projection authenticates the exact active owner and lease generation.
  Only `attachment` receipts confirm anchoring. Native supervisor scheduling and CLI/GUI
  wiring share this owner; combined real-network qualification remains outstanding.
  Initial custody directories publish their complete records and locks atomically;
  interrupted staging cannot leave an empty operation journal at its final path.
- `definition`: parsers and validation for network definitions
  (`networks/*.toml`, spec §3.2) and dataspace definitions
  (`dataspaces/*.toml`, spec §3.3). Definitions are read through
  `iroha_config_base`, and every error names its file and key. Unknown keys are
  errors. Dataspaces default to restricted visibility and the parent network's
  committee; owner-run nodes require explicit `committee.source = "owner"`.
  [`dataspaces/dpn.toml`](../../dataspaces/dpn.toml) contains only dataspace
  choices, with no validator or parent-network configuration.
- `iroha dataspace plan/apply/status <definition> --trust <public-profile.json>`:
  the owner key is selected by the definition; the explicit operator credential
  authorizes protected network reads. No client TOML or hand-written native
  manifests are accepted. Plan retains an immutable local artifact; apply plans
  if necessary, then submits each retained transaction at most once; status
  performs no ledger writes. State defaults to `~/.iroha-dataspaces`.
  `max_fee` is one positive aggregate cap across catalog, bootstrap, aliases and
  recovery. Omitting `account_alias` creates no account alias. The supported
  runtime requires an HTTPS parent with exactly four authenticated NPoS
  validators. Owner-node provisioning and explicit `[monitor]` are rejected.
  Focused tests and four-validator runtime qualification remain separate gates.
- `verify::finality`: the light finality verifier behind gate G5 (spec §9,
  §11.2 D-7). It is anchored in an authenticated genesis or a stored
  complete native `checkpoint.norito`, verifies every contiguous successor under
  finite proof and peer budgets, and requires `2f + 1` fresh
  challenge-bound attestations from the authenticated committee. Each epoch's
  committee comes only from the boundary result its predecessor certified. A
  checkpoint that lags by more than one observation budget catches up in
  bounded, individually published pages (`catch_up`). Exact retained
  predecessor decisions permit one block of lag after checkpoint import. Within an
  observation, earlier responses count when their exact decisions were verified in
  its contiguous prefix, including across boundaries. Supplied proofs cannot select
  their own trust roots. `verify::http` supplies native SDK proof retrieval and
  concurrent challenge-bound committee reads, with one operation deadline and
  bounded per-peer tip retries. Release bootstraps constrain every selected endpoint
  to the independently signed HTTPS roots. Other gates remain in the phase plan.

The committed definitions are `networks/dev.toml`, `networks/ci.toml` and
`networks/perf-10k.toml`. The dataspace examples are in `dataspaces/`.

TODO: add `networks/taira.toml` at release time (P9), once the real host keys
are pinned. Until then `tests/fixtures/taira.toml` has the same shape with
generated keys. These operator definitions are separate from managed developer
commands, which use release-authenticated installation profiles and require no
user-supplied TOML.

The shared managed generator also has an internal `StreamTokenAuthorities` profile for
preparing a future native token service. It retains seven distinct private credentials,
registered and funded transaction accounts, and exact initial capabilities in signed genesis.
Its immutable governance configuration seeds the provider owner as the issuer operator before the
first block. The original signed genesis commits that exact owner map through its execution policy;
all four retained configs must reproduce it. This seed does not admit the provider or declare capacity,
and an existing context cannot acquire it by changing configuration after genesis.
The profile also registers distinct non-signing reserve custody and treasury accounts, with no
private credentials, permissions or initial balance. Only the manager receives the canonical reserve
policy and provider-credit management capabilities. Reserve policy, owner-funded collateral,
credit projection and capacity declaration still require their ordinary signed native transitions.
Signed public metadata binds the selected profile, each credential's exact account role and both
reserve account roles;
retained sidecar replacement or removal cannot reinterpret those original identities.
The ordinary CLI and Mochi creation paths still select `Standard`; restart preserves the
selected retained profile. Profile changes require a new context and private roots reject this
service profile. Neither profile metadata nor generated credentials establish provider admission
or current token eligibility. Token services remain disabled until a separate production owner
commits the ordinary policies/enrollment/grants and publishes a retained configuration revision.

`managed::ManagedStreamTokenCustody` implements shared initial Configure/Enroll coordination for
that authenticated profile and its original signed genesis. It retains the original requests,
UTC limits and bounded fees before dispatch, and recovers the exact wallet transaction through
its once-only journal. Independently verified inclusion of that transaction is separate from a
fresh native custody-state proof; retained inclusion remains reportable during a network outage
without granting current eligibility. This source implementation does not activate services,
admit providers, declare capacity or publish configuration revisions. CLI/Mochi integration and
runtime qualification remain separate work.
