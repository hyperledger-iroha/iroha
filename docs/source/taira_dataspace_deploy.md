# Native dataspace deployment contract

The supported first-release command is `iroha dataspace plan|apply|status`.
It consumes the same [DPN definition](../../dataspaces/dpn.toml) for every step.
The parent network owns consensus, validator services and fees. The dataspace
owner chooses a name, parent, owner key, optional account alias and total fee cap.
Restricted visibility, a one-year lease and the parent committee are defaults.

```sh
iroha --operator-private-key-file /PRIVATE/operator.key \
  dataspace plan dataspaces/dpn.toml --trust /PRIVATE/network-profile.json
iroha --operator-private-key-file /PRIVATE/operator.key \
  dataspace apply dataspaces/dpn.toml --trust /PRIVATE/network-profile.json
iroha --operator-private-key-file /PRIVATE/operator.key \
  dataspace status dataspaces/dpn.toml --trust /PRIVATE/network-profile.json
```

Set `dataspace.max_fee` to a deliberate positive cap before planning. Its asset
comes from authenticated namespace policy. The one cap covers the exact rents
and all three transactions, including recovery; it is not a cap per transaction.
The owner key comes only from `dataspace.owner_key`. Both key files must meet the
native custody checks (direct owner-only files, mode 0600). The operator key is a
separate credential allowlisted by the selected validators for authenticated
reads; `--operator-private-key-fd` can supply it through an inherited descriptor.
No client configuration, manual manifest, lane ID or operation ID is required.

The parent operator must register and activate `Committee` credentials for the
selected validators, using the same keys and trusted proofs of possession.
Global `Validator` credentials alone do not authorize participant-lane signing.
Planning checks this prerequisite; the dataspace owner cannot grant it implicitly.

## Public trust input

The network operator supplies an independently selected public trust profile.
For a retained network, export it locally from its approved public evidence:

```sh
iroha dataspace export-profile \
  --network-id CHECKED_NETWORK_ID --chain CANONICAL_CHAIN_ID \
  --chain-discriminant ACCOUNT_DISCRIMINANT \
  --genesis-signed /PUBLIC/genesis.signed.nrt \
  --genesis-public-key /PUBLIC/genesis.public_key \
  --peers /PUBLIC/peers.json \
  --output /PRIVATE/network-profile.json
```

`peers.json` contains four records with `torii_origin`, `peer_id`,
`node_fingerprint`, `build_fingerprint` and `config_fingerprint`. Export verifies
the signed genesis, NetworkId and exact roster, and writes a new file under an
existing owner-private directory. It reads no credentials and contacts no node.
The chain ID and account discriminant are independent operator pins: signed
genesis does not contain them. Do not obtain expected pins from an untrusted
endpoint. Existing public-reset preparation can produce the same profile through
`iroha taira public-reset export-deployment-profile`.

## Planning, execution and recovery

- `plan` authenticates all four selected validators, checks capabilities, owner
  permissions and funding, allocates an unused lane, derives the committee and
  exact alias prices, then saves a reviewable plan without submitting a ledger
  transaction. Catalog writes require exact `CanSetParameters` authorization.
  Public control-plane reads, the separate allowlisted operator credential and
  owner-scoped account and transaction reads cover deployment observations;
  the owner does not need access to unrelated ledger data.
- `apply` resumes that plan through catalog registration, ownership grant and
  paid namespace registration. Catalog execution atomically creates its native
  fixed-lane policy; the lane activates at the catalog height plus two. Control
  transactions remain global; application work targets the private dataspace.
- `status` submits nothing. It checks the retained transactions and fresh
  authenticated state, and may retain verified proofs and receipts locally.

The default private state root is `~/.iroha-dataspaces`; `--state DIR` selects
another root. Operation identity derives from network, owner and dataspace name,
so moving the definition does not create another operation. Changed semantic
inputs or trust cannot reuse its saved plan. Preserve this directory throughout
the operation. Dispatch is once-only within that durable journal; abandoning or
copying state cannot prove that a transaction was never submitted.

An explicit `plan` can refresh an entirely unsigned plan after quotes or the
catalog change. Once any signed phase or dispatch evidence exists, its original
intent is immutable. `apply` never silently rebases or replaces a transaction.
If initial planning fails before saving a plan, correct only `max_fee` and rerun
`plan` to update that cap. This preserves the verified read-only proof cache;
it cannot change a saved plan, signed operation or other definition fields.
If the still-unsigned alias phase has an expired quote deadline, it can extend
that deadline while keeping the same names, owner, terms, policy and exact rent.
An already signed alias transaction is retained unchanged.
Each signed payload and dispatch claim is retained before submission; a timeout
or missing response resumes observation of that same transaction. Partial or
changed retained evidence fails closed.

Each invocation shares one `--timeout-ms` budget (default 180000) across reads,
dispatch, waits and verification. Preflight retains verified history in the
operation's `preflight-finality/` directory, even before a plan exists. After a
timeout, rerun the same command with the same definition, trust and state. It
reauthenticates the retained proofs locally and fetches their successors; the
cache never replaces genesis-anchored verification.
Success requires independently anchored
certificate history, exact transaction inclusion and execution commitment, and
all four validators' current state. It also requires the expected native lane
instance to be active and signing on each selected validator. Catalog entries
alone cannot establish completion. Pending verification keeps
`deployment_complete` false; `apply` prints its report before failing, while
`status` can report an ordinary pending operation successfully.

## Source ownership and qualification

The CLI definition adapter is `crates/iroha_cli/src/dataspace_definition.rs`;
planning and recovery are in `taira_dataspace_deploy_definition.rs` and
`taira_dataspace_deploy.rs`. The native bridge is
`crates/iroha_core/src/state/runtime_catalog.rs`. See the
[review and acceptance goals](../../specs/configuration_simplification.md),
[lane contract](../../specs/sumeragi_lanes.md) and
[deployment design/status](../../specs/network_deployment.md).

This runtime supports an HTTPS parent with four authenticated NPoS validators.
Owner-provided committees, SSH/edge provisioning and monitoring sections are
rejected. Source tests do not qualify a live deployment or replace the production
four-validator release corridor; current validation evidence is recorded with
the acceptance goals.
