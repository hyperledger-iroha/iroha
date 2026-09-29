# First-release configuration simplification

Scope: dataspace deployment, `iroha_config` and the node templates on `optimizations`. This is a
first-release interface: renamed or removed fields fail with an actionable
error; they do not gain compatibility aliases.

## Review

The first distinction is ownership. A private DPN dataspace on existing Taira
validators is not a validator process or a separate network. Its owner should
not configure public Parliament, faucet, SoraFS discovery/repair, Inrou, NTS,
global consensus, validator identities, other customers' lanes or network fees.
Removing defaults from a whole-validator file does not fix that scope error.

There is no tracked `dpn.config.toml` in this checkout. Its configuration
surface is owned here by `iroha_config`; the corresponding Iroha Taira template
started at **805 lines and 431 scalar values**. It mixes
node identity, deployment policy, network topology, protocol constants and
optional subsystem tuning. An operator cannot tell which values need a decision.

The downstream DPN repository also carried an actual
`ops/taira/dpn.config.toml`: a 472-line whole-network validator template. That
consumer is now a seven-line, five-input dataspace definition. The unchanged
network settings live in `ops/taira/validator.config.toml`; its native checker
and topology tests use that explicitly named parent-network file. Keeping a
short example in Iroha alone did not address this downstream ownership error.

1. **Defaults masquerade as decisions.** Disabled SoraFS repair/GC workers,
   DA geometry, empty metadata and several ingress limits repeat Rust defaults.
   Copying these values creates another policy owner and prevents improvements
   to defaults from taking effect.
2. **One fact has several inputs.** A profile already determines the network
   discriminant, but the node file repeats it. The profile role and
   `sumeragi.role` could disagree, combining one role's services with another
   role's consensus behavior.
3. **The recommended path is hard to find.** Compiled profiles and `data_dir`
   already reduce operator inputs, but the prominent Taira example is still a
   large flat configuration. Another configuration language would compound this.
4. **Repetition hides the topology.** A long series of nested routing matcher
   tables and default collection fields obscures actual lane/dataspace choices.
5. **A short file is not sufficient.** The newer profile has four baseline lanes;
   the flat Taira example has seven. Swapping them would change the network.
   Consensus settings, authority bindings, fees and custody are not optional
   just because their configuration is inconvenient.

Runtime review also found recovery gaps beyond file length. The source now
keeps the operation lock and validated plan throughout execution, permits only
an unsigned alias phase to extend its submission deadline under unchanged
identity, acquisition terms, policy and exact rent, and checks a native lane's
immutable creation parameters instead of comparing them with later policy
defaults. Preflight retains authenticated proof progress in a definition/trust
bound child journal and continues in bounded batches within the existing
timeout. A retry verifies retained proof bytes locally before fetching the next
successor. These changes have focused regressions; the validation section below
records their current qualification state.

Before any plan or deployment evidence exists, an explicit `plan` may correct
only `max_fee` while preserving the read-only proof cache. That cache excludes
the spending cap from its identity because chain proofs authorize no spending.
All other definition/trust fields remain bound; `apply`, `status` and every
operation with a retained plan or signed evidence still reject a changed cap.

## Goals and acceptance criteria

| Goal | Acceptance criterion | Scope |
| --- | --- | --- |
| C0: Dataspace input contains only dataspace decisions | `dataspaces/dpn.toml` names DPN, its parent, owner, optional account alias and fee cap. Access defaults to restricted and committee to the parent network. Global service/node sections are rejected. `configs/validator.example.toml` is explicitly for a validator process. | Complete; definition contracts and D4 native rehearsal passed |
| C1: One input per fact | `profile`, required `validators`, optional `role` (default `validator`); profile supplies discriminant; role supplies Sumeragi role. Retired selectors and derived overrides fail, including equal-value overrides. | Implemented |
| C2: Defaults have one owner | Remove only values proven equal to production parser defaults; preserve all non-default policy and topology. Tests compare effective sections against the former explicit settings. | Implemented |
| C3: A small, tested starting point | Profile node example under 40 lines; only deployment bindings, no tuning checklist or inline secrets. Load the actual example in a parser regression. | Implemented |
| C4: One generated deployment path | `iroha dataspace plan/apply/status` consumes the definition and derives native manifests, bindings and durable operation identity. Catalog publication also creates the native fixed lane. Focused CLI/native checks and the controlled native rehearsal pass (D0–D4). The P2/P8 network renderer/profile cutover remains open. | Complete for the supported existing-parent path; broader P2/P8 renderer work open |
| C5: Keep complexity from returning | Runtime owners must justify each new operator input. Derive dependent values, keep disabled-subsystem defaults out of examples, and add effective-config regression coverage when pruning policy. | Ongoing: configuration owners |

The supported existing-parent deployment path meets these scoped acceptance goals:

| Goal | Acceptance criterion | Status |
| --- | --- | --- |
| D0: Canonical end-to-end command | `iroha dataspace plan`, `apply` and `status` consume the same definition. Owners need no separate client configuration or hand-written native manifest. Plan saves an immutable local plan without a ledger write; apply advances it and status verifies the actual retained outcome. | Implemented; focused verification passed |
| D1: Derive trust and owner custody | Resolve the parent from the definition and authenticated network identity; derive the owner from its permission-checked key file. Derive committee/account bindings from the independently selected signed genesis and check live Committee-key eligibility before catalog signing. Retain bounded preflight proof progress under the same definition/trust binding without skipping native chain verification. Never invent trust anchors, aliases, permissions or owner nodes; reject unsupported explicit inputs before deployment. | Implemented; focused verification passed |
| D2: One total spending limit | Persist and enforce `dataspace.max_fee` across every namespace acquisition and transaction in the operation, including recovery. Discover the fee asset from authenticated parent policy; reject a quote or retained operation that could exceed the total. | Implemented; focused verification passed |
| D3: Durable recovery without duplicate submission | Retain operation identity, exact intent and each signed transaction before dispatch while continuously holding the operation lock. After an uncertain result, reconcile that same transaction; repeated apply never replaces or resubmits it. Only an unsigned alias phase may refresh its deadline under unchanged terms, policy and exact rent. Changed definitions cannot reuse another intent's journal; a proof-only preflight cache cannot authorize adoption of missing signed-plan evidence. | Implemented; focused verification passed |
| D4: Migrate consumers and verify the complete path | Update examples, command help and consumers to the canonical definition workflow; remove superseded manual-manifest setup. Focused tests cover plan/apply/status, optional aliases, visibility, unsupported inputs, trust/custody rejection, aggregate spending and interrupted recovery. A controlled native rehearsal must demonstrate the supported deployment path before an end-to-end success claim. | Implemented; controlled native rehearsal passed |

Separate release qualification remains open: run the workflow through four daemon
HTTP endpoints and complete the authorized live/release gates. Those checks are
separate from the completed source acceptance above; neither a four-daemon HTTP
run nor a live deployment was performed here.

`validators` stays explicit: trusted peers are bootstrap contacts and can include
observers or only part of a roster. Counting them cannot establish the validator
committee. Public bootstrap keys, PoPs and genesis anchors also stay explicit;
they are trust inputs, not sensible defaults the loader can invent.

## Implementation boundaries

- `DataspaceDefinition` is the existing owner of dataspace inputs. Defaulting
  its committee to the network removes redundant input without introducing
  another format. The canonical runtime supports an HTTPS parent and exactly four
  NPoS validators bound by the independently selected signed genesis, with live
  Committee-key eligibility checked before catalog signing. Owner committees,
  SSH/edge provisioning and any
  explicit monitoring section fail before credential access; parser support for
  future owner topology is not a claim that this command provisions it.
- The DPN example is a dataspace definition, never an input for `iroha3d`.
  D0–D4 verify its executable existing-parent contract with focused CLI checks
  and a controlled native deployment/private-write rehearsal.
- Use the existing `iroha_config::node_config` loader and compiled profiles.
  No new mode, environment override, external include graph or migration shim.
- `data_dir` already owns the fixed state and secret layout. The example relies
  on it instead of repeating paths.
- Custom flat configuration remains available for custom networks. It is not a
  fallback decoder for the retired profile selectors.
- Keep current Taira topology and policy while pruning exact defaults. Profile
  cutover is tracked in the existing deployment plan, not hidden in cleanup.
- Network provisioning, live configuration changes and ledger replacement are
  outside this source change.
- Preflight caching retains HTTP progress and avoids repeated cryptographic
  admission within one invocation. A fresh invocation reauthenticates retained
  proofs from the selected genesis; it does not trust a serialized verifier or
  claim constant-time verification of an arbitrarily long parent history.

## Validation

Focused `iroha_config` node-loader and template contracts cover the operator
surface, retired-key errors, role/prefix derivation, example loading and default
equivalence. Generator inputs and their checked-in examples must agree. These
checks do not establish live rollout or four-validator release qualification.

The four flat templates shrink from 1,254 to 1,014 lines and remove 156
redundant assignments (22.8%). Taira remains 737 lines, including the explicit
dataspace/validator scope label, because its seven-lane topology and deployment
bindings remain; it is not relabelled as the
35-line validator example.

Validation on this change:

- Actual downstream DPN consumer: 27 configuration tests passed. The current
  production CLI parses its five-input definition and rejects an added
  node-only SoraFS section before reading credentials. The renamed validator
  template preserves every parsed setting and all configuration-body bytes.

- Dataspace/network definition suite: 56 passed, including the five-input DPN
  example, restricted/parent defaults, explicit owner-only settings, and
  rejection of irrelevant global configuration. The renamed validator example
  also passes its focused parser test.

- Node-loader tests: 19 passed, including the checked-in example; independent
  profile discriminant protection: 1 passed.
- Compact-template contracts: 4 passed; existing Taira contracts: 5 passed.
- Existing integration tests selected by `profile`: 5 passed, including full
  Minamoto parsing and the Nexus example.
- Focused Python edge/config consumers: 25 passed; provisioning template guard
  passed. All retained parsed TOML leaves are unchanged.
- Initial profile testing exposed four stale tests from the earlier Sumeragi
  queue-geometry removal. The queue/layering cases now assert current protected
  policy fields. The consensus digest pin was updated after auditing commit
  `8a99f3f5ba`'s removal of static block/queue fields and derived queue geometry;
  the policy digest pin is unchanged. The focused profile rerun passes all
  23 tests.
- Native private-dataspace execution: 1 passed in
  `iroha_core/tests/private_dataspace_runtime.rs`. The in-process NPoS chain pays
  for catalog registration, owner bootstrap and the namespace plus optional
  account alias using native fee and SNS policy. It then admits a private write,
  verifies a real three-of-four BLS certificate, rejects a forged certificate,
  stores the lane block and executes the private write through production global
  merge. This is concrete certified execution evidence, not a four-daemon HTTP
  deployment rehearsal or a live deployment.
- CLI dataspace filter: 111 passed, including unsigned alias deadline recovery,
  continuous operation-lock custody, immutable native lane parameter checks,
  preflight cache batching, resume and tamper rejection, and the narrow pre-plan
  fee-cap correction. Operator/owner custody:
  19 passed; current genesis-policy fixture: 1 passed. The bounded authenticated
  consensus-key reader and native default lane-policy regression each pass 1.
- Production CLI build passed. The compiled binary exposes the definition-based
  commands and rejects the retired `taira dataspace-deploy` command; command-help
  smoke checks passed without loading credentials.
- The broader Python devnet run reported 104 failures, 110 passes and 56 passed
  subtests. Failures include the missing ABI-25 native account-codec wheel and
  an unrelated retained-script inventory mismatch. It is not a passing gate.
- All 42 changed Rust files in the scoped formatting gate pass, as do
  `git diff --check` and the retired-codec guard. Whole-workspace
  formatting still reports unrelated `iroha_core` differences in `identifier.rs`
  and `zk.rs`; they are outside this change. Full workspace, four-daemon deployment
  qualification and live deployment have not run. Broader network provisioning,
  owner-node lifecycle and the P2/P8 renderer cutover remain open.
