# Ironwood activation and Global-pool bootstrap qualification

Source-coupled candidate procedure; no live execution is authorized or recorded here.
Taira's existing ledger must never be reset. See `ironwood_runtime_transition.md` for
immutable definitions and the mandatory decoder rollout gate. Concrete network,
asset, accounts, pool ID, height, notices and signatures are operator-selected
committed inputs; this document does not invent Taira values or permissions.
Taira has no account able to propose, so its one owner-authorized route is the
[certified activation](runtime/taira_time_ironwood_v1.md), not the sequence below.

## Authority and exact transaction sequence

1. Rebuild/qualify every validator and off-chain decoder listed in the transition
   spec. Retain artifact digests, pre/H/post replay, World-proof and historical
   Orchard evidence. Unchanged wire/ABI hashes do not establish readiness.
2. Read committed `runtime_upgrade_policy_v1`, any provenance policy, selection and
   scheduled records from the intended network. Export `/v1/runtime/upgrades` as
   **JSON** (not a Norito frame) to `runtime-upgrades.json` and run:
   `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$PWD/target" cargo run --offline --jobs 6 -p iroha_core --features node --bin ironwood_activation_preflight -- runtime-upgrades.json H`.
   It checks current and scheduled projected predecessors and same-height conflicts.
   A status response alone is not finality; authenticate the complete selection cut
   with `authenticate_runtime_selection_at` and independently authenticated history.
   Recheck before proposal and approval. A mismatch needs a reviewed composition,
   not replacement of the compiled baseline or deletion of scheduled obligations.
3. Retain the exact qualification-evidence artifact as `qualification-evidence.json`.
   Its digest is `Hash::new_from_chunks([b"iroha:ironwood:qualification-evidence:v1\0",
   exact_artifact_bytes])`; whitespace and newlines are part of those bytes. The
   artifact must identify the independently qualified decoder/validator/SDK build,
   source and replay evidence; its digest is a binding, not proof of qualification.
   After the read-only preflight passes, produce the exact proposal JSON:

   ```sh
   CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$PWD/target" cargo run --offline --jobs 6 -p iroha_core --features node \
     --bin ironwood_activation_preflight -- runtime-upgrades.json H \
     --emit-change-set NETWORK_HEX qualification-evidence.json > change_set.json
   ```

   `NETWORK_HEX` is the exact 32-byte marked network identity in lowercase hex.
   The emitter uses `activation_change_set_for_selection` and the authenticated
   current/projected predecessor; it neither signs nor submits. Keep its stderr
   preflight report and emitted JSON. The proposal transaction is exactly `ProposeRuntimeUpgradeProposal { action:
   RuntimeUpgradeActionV1::Schedule(change_set), provenance }`, under an authority
   holding `CanProposeRuntimeChangeSet`. Supply the committed provenance policy's
   endorsements. The CLI `iroha runtime upgrade propose --file <change_set.json>
   [--endorsement <file>...]` constructs this transaction; its HTTP counterpart only
   drafts it. The current Parliament prerequisites below are incomplete; submission
   alone leaves a `Proposed` governance record and does not schedule the change set.
4. After scheduling, every current validator supplies its exact signed
   `AttestRuntimeSupportV1 { attestation }`. The deployed-target CLI signs with
   an explicitly selected runtime-only validator key file, not a node signing endpoint:
   `iroha runtime upgrade attest --id <change-set-id> --validator-key-file
   <private-runtime-path> --release <qualified-release-digest> --tested-checkpoint
   <tested-height>`. Run each validator owner’s selected credentials locally; never
   copy private keys into a runbook or evidence bundle. Check
   network, current committee digest, successor, validity at H and release evidence.
   Wait for certified `Activated(H)` and the exact successor selection. Three of
   four attestations are insufficient. A lapse is not activation.
5. Select a new nonzero pool ID and a dedicated reserve account, already registered
   via `Register::account(Account::new(reserve))` if necessary. The asset must exist
   and its committed balance policy must allow `AssetBalanceScope::Global`. Check
   no prior state/head/history/nullifier or reserve-custody binding exists for this
   new pool/reserve/asset combination. Retain existing Orchard pools unchanged.
6. Use a **dedicated bootstrap multisig**, restricted to this pool-bootstrap
   operation. The **executing authority** must temporarily hold the exact unit permission
   `iroha_executor_data_model::permission::governance::CanEnactGovernance`, directly
   or through a committed role. For a multisig proposal this is the multisig account
   that executes the instruction, not merely an approving signatory. An authorized
   existing holder of the exact `CanEnactGovernance` permission may delegate it with
   `Grant::account_permission(CanEnactGovernance, bootstrap_multisig)` under the
   initial executor's committed permission rules. Authenticate the grantor's exact
   direct or role-derived holding and the successful grant before bootstrap.
   Ordinary account ownership or an administrator label is insufficient. Never
   assume an account called “governance” has it or try to self-grant it. This broad
   permission gates roughly 31 privacy-governance operations, so do not assign it to
   a general-purpose payer, merchant or long-lived application authority.
7. Submit exactly this bootstrap instruction after activation, signed by that authority
   (or as its approved native multisig occurrence):

   ```rust
   BootstrapPrivacyIronwoodPoolV1::new(PrivacyIronwoodPoolBootstrapV1::new(
       selected_new_pool_id,
       selected_asset_definition_id,
       AssetBalanceScope::Global,
       selected_reserve_account,
   )?)
   ```

   Emit the exact JSON instruction vector from the private app copy, using its
   matching native SDK (the exporter is read-only):

   ```sh
   "$PYTHON" tools/ironwood_bootstrap_instructions.py --pool-id-hex "$POOL_HEX" \
     --asset-definition-id "$ASSET_ID" --reserve-account-id "$RESERVE_ID" \
     --chain-discriminant 369 > "$CAMPAIGN/bootstrap.json"
   iroha --config "$SIGNATORY_CONFIG" --fee-payer authority ledger multisig propose \
     --account "$BOOTSTRAP_MULTISIG" --proposal-nonce "$PROPOSAL_NONCE" \
     < "$CAMPAIGN/bootstrap.json"
   ```

   The SDK constructs `Vec<InstructionBox>` with Norito JSON. Collect the multisig's
   actual required approvals and authenticate its successful execution; never hand
   encode this instruction or count proposal inclusion as the bootstrap.

   This binds protocol ordinal 12 and the new namespace. It does not mint funds or
   replace an old pool. Check actual committed success, selected asset/scope/reserve,
   bootstrap digest, initial empty frontier/root at epoch 1 and the single
   reserve-custody owner. Repeating bootstrap must refuse. A proposal's storage or
   approval event alone is not successful bootstrap evidence. Immediately submit
   `Revoke::account_permission(CanEnactGovernance, bootstrap_multisig)` under the
   authorized grantor, and authenticate committed revocation. Verify that the
   bootstrap multisig has no remaining direct **or role-derived** holding of that
   permission and that another privacy-governance operation by it refuses. Retain
   the grant, successful bootstrap and revocation receipts together.

   After governed activation of the keyed State profile, obtain the pool state,
   root head, provenance, payer balance and reserve balance through signed
   `POST /v1/ledger/state-proof/{height}` reads. Verify the five rows at one
   certified Global height as described in `ironwood_boi_integration.md`.
   Give the operator proof reader direct `CanReadStateRow` grants for its keys
   or use its existing `CanReadAllLedgerData` grant. The merchant uses anonymous
   public finality frames and an incoming-viewing credential.
8. Deploy and authenticate the actual BOI registry's contract address, code hash,
   manifest and invocation permissions; initialize its namespace, participant roles
   and scenario counters. Fund the separate payer for both the selected payment asset
   and transaction fees. With an independently controlled merchant, execute the
   BOI two-transaction flow described in `ironwood_boi_integration.md`. Confirm exact
   public debit, reserve increase, merchant receipt, required registry events and
   counters. Retain canonical block proofs and reject counterfeit receipts. No
   production activation is qualified until the four-peer scenario campaign passes.

These transaction types are implemented in `runtime/change_set.rs`,
`smartcontracts/isi/privacy.rs` and
`smartcontracts/isi/privacy/governance_authorization.rs`. The initial
signed-genesis exemption is not available on a running chain. This procedure never
uses a genesis reset, a node-local activation flag or a new IVM ABI.

## Current Parliament prerequisites: activation unavailable

The current design uses anonymous on-chain ballots. The retired timed-OVN/TLE
registration, custody, opening and threshold-release sequence is not an activation
route. Core currently returns local `AnonymousBallotUnavailable` at admission and
defers anonymous inputs before Network execution. A proposal, CLI draft, public
finding or support attestation cannot substitute for a genuine governance result.

The required work is recorded in [Parliament implementation goals](parliament_ballot_implementation_goals.md)
and [B6 qualification evidence](../docs/validation/parliament_ballot_b6.md):

1. Complete E2 authoritative registration and compact source custody, 64-byte
   credential-aware roster identities, acceptance/sealing/restore, authenticated
   publication, bounded signerless admission/execution/gossip, and API/SDK migration.
   Preserve candidate-purpose isolation from production certification and effects.
2. Complete E3 through actual four-validator submissions, finality, restart and
   rollback. Component tests and fixture certificates do not establish this gate.
3. Complete B6 proof-profile selection, cryptographic and resource qualification,
   required hardware evidence, independent review and the owner's qualification
   record. Authorization to implement/test the candidate does not authorize
   binding production use or supply another party's attestation.

TODO: publish the exact qualified anonymous-ballot transaction sequence here after
E2/E3/B6 are complete. Until then, stop before governance submission and activation;
no node-local configuration switch, retired TLE provider, fixture approval or
ledger reset supplies the missing authority.

The eventual runtime proposal must retain its committed notice and proposal bytes,
a genuine `GovernanceCertificateV1` for the exact effect and expected head, and
successful certified enactment into `Scheduled`. Core owns certificate construction
and the enactment delay; no caller-supplied certificate or public `MarkEnacted`
instruction replaces them. At enactment, a changed head is `Superseded` and a
refused effect is `ExecutionFailed` with its writes rolled back. Scheduling remains
separate from certified activation at H, which requires elapsed committed notice,
release support from every current validator, and the exact successor selection.
All external transactions retain their actual signature, fee and permission checks.

## Composition with the Time successor (merged M8 candidate)

The merged catalog uses **one** construction: `ReleaseCatalog::lifecycle_successor`
clones the selected native implementation, preserves its dependencies and storage
owners, appends the exact `trigger_lifecycle` service closure, and binds Time's
`TRIGGER_LIFECYCLE_MODULE` code identity. The content-addressed implementation
descriptor includes the exact services profile and dependency identities. Thus
Time-only and Time+Ironwood have distinct implementation IDs even though they use
the same native module. The former Ironwood-specific composition module is removed.
Only the explicitly compiled baseline, Ironwood, Time and combined profiles are
supported; published descriptors alone never manufacture release support.

Prepare either change from the **current committed selection**, with its actual
published-definition inventory. `prepare_lifecycle_change_set` preserves the
Ironwood owners when Time follows Ironwood. Conversely,
`activation_change_set_for_selection` preserves Time's service and reaches the same
combined descriptor when Ironwood follows Time. The service-only reference helper
refuses additional storage, bindings, active definitions and restrictions instead
of dropping them. Re-run preflight against the complete scheduled inventory before
scheduling; schedule a second change only with the intended predecessor fixed.

The Ironwood convenience constructors preserve restrictions on retained definitions.
They explicitly refuse a predecessor whose restricted definition would be replaced. The frozen
foundation's `Restrict` transition is effective at the change-set height; it cannot
encode an arbitrary existing future effective height on a new definition. Do not
drop or silently re-date that policy. Resolve it through an explicit reviewed
governance policy decision before preparing this constructor. Completion closures
continue to refuse any replacement that strands an obligation. The two-order
qualification below covers these exact supported predecessors, not arbitrary
future native compositions.

The generic governed Time builder retains its existing policy: its explicit
Restrict/Deactivate transitions consume a retired native bundle's restriction,
while retained-definition restrictions remain exact and obligations still prevent
retirement. Review that policy change as part of the governed proposal; no
node-local activation path authorizes removal of a restriction.

`trigger_lifecycle_enabled` resolves the selected compiled services profile through
`ReleaseCatalog::service_enabled(profile, "trigger_lifecycle", closure)`. Baseline
and Ironwood-only return false; Time and combined return true; unknown profiles or
changed closures refuse. The same predicate controls output-capacity capture and
`lifecycle_support::selected_output_support`. Preflight and governed activation
must retain the selected output support checks. Unanimous signed support
attestations and committed notice still govern activation. A local
capacity shortfall stops the unsupported node with `UnsupportedRuntime`; it never
rewrites a governed activation into a different ledger result or changes admission
before activation. Operators must check capacity before signing support.

The Python SDK accepts two exact `(services ID, implementation ID)` pairs generated
from this final catalog in `scheduled_lifecycle_supported_pairs.rs`. It recomputes
both IDs from certified witness descriptors and checks pair membership, exact
service closure, uniform active bindings, selected profile equality and certified
selection/continuation branches. A native module name alone is insufficient.
Catalog derivation and SDK tests cover both activation orders with realistic
predecessor publication lists, and reject altered descriptors and cross-pairs.
Regeneration is explicit maintenance, never an automatic runtime fallback:

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$PWD/target" cargo test -p iroha_core \
  --features node,privacy-release-evidence --lib \
  regenerate_merged_lifecycle_supported_pairs --offline --jobs 6 -- --ignored
```

Merged acceptance covers `triggers.data`, `triggers.pipeline`, `triggers.time`,
`triggers.by_call` and `world.smart_contract_state`, plus real committed activation
and SDK continuation. Exact current-source results and remaining live-network
qualification are in `PROGRESS.md` and `codex/a-impl-merged-m8.md`. Component tests
and generated pair tables do not establish four-peer or deployment readiness.
All off-chain decoders must still satisfy the decoder-readiness gate above before
activation; neither an unchanged wire hash nor governance enforces that rollout.
