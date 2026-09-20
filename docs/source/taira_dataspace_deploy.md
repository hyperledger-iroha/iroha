# Native Taira dataspace deployment

`iroha taira dataspace-deploy` plans and advances one physical dataspace, its
immutable bootstrap grant, and one paid account alias. It uses the configured
native account client, including the normal `--config-fd` and
`--config-source-path` custody boundary. It requires the canonical Taira chain
and account profile 369.

The command is a first-release additive workflow. It does not overwrite an
existing lane, move a dataspace, repair an existing namespace, submit empty
transactions, or create blocks to advance time.

## Validator epoch maintenance

The network operator must provision and commit the next mint-finality roster
before each NPoS boundary. The native `iroha taira epoch-maintenance` workflow
uses independently selected public genesis/peer trust and a bounded public
schedule produced by `kagami kagemusha derive-mint-finality-epoch-schedule-v1`.
Run its `maintain` command alongside application traffic with a separate ledger
owner holding `CanSetParameters`; an HTTP operator credential alone cannot
submit the maintenance transaction. The DPN deployment receives no validator
seed material and does not manage validator epochs.

For boundary height B, the next roster must have executed by B−1. Queued
maintenance needs three canonical carrier heights, so preparation rejects a
parent later than B−4. These carriers contain actual admission, availability and
parameter execution work. A public API listener or one successful deployment
phase does not establish that the next epoch has been prepared.

The maintainer retains one exact signed transaction per network and target epoch,
observes uncertain submissions, and verifies its successful authenticated
execution on all four validators. It stages the following roster only after an
actual epoch transition. Its finite schedule and invocation budget require
explicit renewal and supervision; it does not create empty blocks to reach a
future epoch. Provisioning public keys does not prove future election membership.
The workflow supports the selected fixed four-validator configuration and
rejects observed membership changes. The consensus boundary remains authoritative.

## Commands

For an existing network, export the target profile from the retained public
signed genesis, its independently selected public key, checked NetworkId and
four public peer records:

```sh
iroha taira dataspace-deploy export-profile \
  --network-id CHECKED_NETWORK_ID \
  --genesis-signed /ABSOLUTE/PUBLIC/genesis.signed.nrt \
  --genesis-public-key /ABSOLUTE/PUBLIC/genesis.public_key \
  --peers /ABSOLUTE/PUBLIC/peers.json \
  --output /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/deployment-profile.json
```

`peers.json` is an array of the existing profile's four peer records, each with
`torii_origin`, `peer_id`, `node_fingerprint`, `build_fingerprint` and
`config_fingerprint`. Select these public pins independently from approved
validator deployment evidence. A historical build pin must not be reused after
an upgrade, and untrusted HTTP discovery must not become the expected authority.
The command verifies the native signed genesis, checked network, exact four-peer
roster and node hashes, then publishes the native trust format without replacing
an existing file. Its receipt hashes the exact public inputs and output. It loads
no client configuration or credentials and makes no network request. Export does
not establish authorization or deployment success; plan and apply still compare
fresh signed attestations against every selected pin.

When preparing a reset, the existing assembled inventory and complete
`prepare-public-inputs` bundle can also export the target profile:

```sh
iroha taira public-reset export-deployment-profile \
  --inventory /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/assembled-inventory.json \
  --public-inputs /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/public-inputs \
  --output /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/deployment-profile.json
```

This local command verifies native genesis identity, the inventory's exact
public genesis artifacts, and all four selected peer identities and
fingerprints. It reads only the supplied public inventory and public bundle;
it does not open any configuration, key, source, or host path referenced by
the inventory. Its output is a locally selected target expectation. It does
not prove authorization, release qualification, or live deployment. The output
must be a new file under an existing owner-private directory.

The four-validator preflight and completion reads need the runtime operator credential allowlisted
by all four selected validators for this exact NetworkId. Pass the global
`--operator-private-key-file` option before `taira`, or use
`--operator-private-key-fd` for an inherited read-only descriptor. This key is
separate from the ledger account key and reset authorization signer; the CLI
does not infer it from `CLIENT.toml` or the environment. Its file must have mode
0600. Keep the existing parent directory private to its owner (mode 0700).

Generate a native manifest from that profile and current namespace policies:

```sh
iroha --config CLIENT.toml \
  --operator-private-key-file /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/operator.key \
  taira dataspace-deploy init \
  --dataspace dpn --lane-id 6 --lane-profile restricted-full-replica \
  --account-alias admin \
  --trust /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/deployment-profile.json \
  --payment-asset 6TEAJqbb8oEPmLncoNiMRbLEK6tw \
  --alias-create-maximum 0.5 --transaction-fee-maximum TX_FEE_CAP \
  --lease-years 1 --output-dir /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/manifest
```

Owner and checked NetworkId come from the configured native client. The two
policy versions and asset bindings come from native SNS policy reads. The
explicit lane profile selects public or restricted visibility with FullReplica
storage, four validators and fault tolerance1. The default quote lifetime is
3600 seconds; `--quote-lifetime-secs` accepts1..86400. The generated account
alias is additional, preserving any existing primary alias. `init` checks the
native paid plan and atomically writes `deployment.json` inside the selected
output directory; it submits nothing. In the example, that file is
`/ABSOLUTE/OWNER_PRIVATE_DIRECTORY/manifest/deployment.json`; the `manifest`
directory must be new. The inline lane manifest is generated from the signed
genesis's registered and activated universal-lane validator accounts and peer
bindings. The selected profile supplies each peer's Torii endpoint. Generation
requires the exact four selected peers, four distinct accounts and quorum three;
bindings absent, inactive or ambiguous in signed genesis fail before a deployment file is written.
The trust file contains the independently selected public genesis key, exact
signed genesis wire and four public peer/fingerprint records. Both generated and
loaded deployment manifests pass the same native schema, alias and committee
validation used by Core before any deployment phase can be planned. A loaded
manifest must retain the exact generated committee and selected peer endpoints;
edited committee inputs are rejected. The signed genesis bindings describe the
proposed deployment intent. Core separately verifies current account, peer, role
and key eligibility when activating the catalog; generation is not live authority.

Public-reset inventories select each validator's canonical HTTPS root URL,
including an explicit nondefault port when needed. Four distinct URLs may use
one existing DNS hostname and its existing TLS certificate; the four account,
peer, genesis and fingerprint bindings remain independent. Inventory roots end
with `/`, such as `https://validator.example.org:8443/`. Path prefixes are not
accepted because proxy rewriting would change the path covered by native
request signatures. Select listener ports and verify certificate coverage and
routing before applying the reviewed inventory; examples do not provision them.

The maintained `scripts/render_taira_edge_nginx_conf.py` reads the roster's
canonical authority-only `torii_public_address` values, such as
`https://validator.example.org:8443` (without `/`). It uses each effective HTTPS
port, rejects duplicate or conflicting listeners, and preserves request paths
and authentication headers. Select the convenience host's upstream with
`--public-upstream-validator taira-validator-1`; the selector is the exact
validator slug, even when all four validators share a hostname. With no
selector, the first ordered validator is used. Keep `--tls-lineage` pointed at
the explicitly selected certificate lineage covering that hostname.

Create the journal parent with mode 0700, then use the manifest path
printed above. Replace `OPERATION_ID` with the exact ID returned by `plan`:

```sh
iroha --config CLIENT.toml \
  --operator-private-key-file /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/operator.key \
  taira dataspace-deploy plan \
  --manifest /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/manifest/deployment.json \
  --journal-dir /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/journals

iroha --config CLIENT.toml \
  --operator-private-key-file /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/operator.key \
  taira dataspace-deploy apply \
  --journal-dir /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/journals --operation-id OPERATION_ID

iroha --config CLIENT.toml \
  --operator-private-key-file /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/operator.key \
  taira dataspace-deploy status \
  --journal-dir /ABSOLUTE/OWNER_PRIVATE_DIRECTORY/journals --operation-id OPERATION_ID
```

`plan` prints the operation ID and writes an immutable native plan below
the selected journal parent's `OPERATION_ID` directory. Repeating the same plan
validates and reads that saved plan. A different intent cannot reuse the same
operation ID. If `operation_id` is null, the ID derives from the canonical
intent and checked NetworkId;
reordering the two alias intents does not generate another identity.

`apply` resumes from the saved plan and waits for each retained transaction
before advancing to the next phase. It polls the exact signed transaction using
the same client and journal; observation never signs or submits a replacement.
A pending operation can be resumed with the same operation ID.
Phase and finality transitions appear on stderr; machine stdout remains one
typed result. `apply` exits successfully only after authenticated completion
with a named receipt. An incomplete result is printed before the command exits
with an error, so automation can retain the report without mistaking it for a
successful deployment. `status` reports an inspected operation's pending state
without treating that state as a command failure.
For a failed phase, the terminal error includes its transaction hash and the
retained global/local `Rejected` or `Expired` observations, with source and any
reported block height. For `Rejected`, one authenticated query requests the exact
retained transaction's committed details. A matching rejected result supplies its
native error chain; exact typed absence is reported as unavailable details. Other
query or identity errors remain failures. `Expired` causes no details query, and
no failed transaction is resubmitted. The formatter uses the retained report and
never prints a signed payload or claims independently anchored completion.

`apply` and `status` accept `--timeout-ms` (default `180000`). Each invocation
creates one absolute deadline shared by its preflight, HTTP reads and proof
verification. Pending-phase sleeps consume that same budget. The CLI checks
the deadline before first dispatch and before admitting completion, so an
expired budget cannot authorize a new submission or a late success. A caller
with a broader workflow deadline must pass only its remaining milliseconds.

`status` submits no ledger transactions. It can retain verified proofs, canonical
carrier bytes and completion receipts in the local journal. It can inspect a
saved operation after write permissions have been revoked. Capability, owner,
network, native codec and retained transaction checks still apply.

## Manifest

The manifest has one closed versioned schema:

| Field | Native type and meaning |
| --- | --- |
| `schema_version` | `1` |
| `operation_id` | Explicit null or 1–96 ASCII letters/digits/`-`/`_` |
| `network_id` | Checked native `NetworkId` |
| `owner` | Native `AccountId`; must match the configured signer |
| `dataspace` | `RuntimeDataSpaceAdditionV1` |
| `lane` | `LaneConfig` |
| `lane_manifest` | `RuntimeLaneManifestV1`, containing the native inline manifest |
| `alias_request` | `AliasSetupPlanRequestV1` with exactly one dataspace intent and one account-alias intent |
| `spending` | Object described below |
| `finality` | Native trust profile: `genesis_public_key`, `genesis_signed_wire_hex`, and four `peers` with `torii_origin`, `peer_id`, `node_fingerprint`, `build_fingerprint`, `config_fingerprint` |

`spending` contains `asset_definition_id`, `alias_create_maximum`, and
`transaction_fee_maximum`. Both maxima use native `Quantity` JSON. The two
alias quote guards must use that asset and must not authorize more than the
per-resource acquisition maximum. For a deployment authorizing at most
0.5 XOR for each resource, set both guards and `alias_create_maximum` to the
native quantity `0.5`. Set an explicit transaction fee maximum separately.

The account alias must target the existing owner directly in the new
dataspace, with no domain segment. Existing primary aliases can be preserved
by choosing the native `Additional` role. Native intent fields, policy
versions, paid terms and deadlines retain their usual consensus semantics.

The native bootstrap-grant constructor checks the canonical dataspace name,
full SNS selector hash and derived numeric ID. The physical descriptor,
lane, inline manifest and both namespace intents must agree on that identity.
No string slicing or external identity codec is used.

Planning captures the current native lane catalog, incarnation root and
runtime overlay hash. The transaction uses all three as compare-and-set
preconditions. Applying never silently rebases a changed catalog. Sparse
lane IDs and the existing exclusive lane namespace bound are preserved.

## Spending and phase order

New-plan creation first verifies challenge-bound statements from all four
selected nodes, their exact genesis/peer/build/configuration identities and the
public finality and lane-status routes. Planning checks native capabilities, the exact existing owner, complete
effective `CanSetParameters` and `CanReadAllLedgerData` permissions, and the
owner's global balance of the selected asset. The initial balance must cover
two acquisition maxima plus three transaction fee maxima. Before each
unsubmitted phase it must cover the remaining maxima.

The phases are:

1. Submit one native additive catalog `SetParameter`.
2. Require the exact physical readback, the absence of its bootstrap grant,
   and a typed missing SNS registration; submit one native grant `SetParameter`.
3. Require the exact committed grant, request a fresh native alias plan with
   exactly two paid `Create` resources, and submit its exact native instructions.

A native alias plan is obtained before the physical transition and again
after the grant. Planning between those phases is deliberately not required:
the catalogued namespace requires its bootstrap grant before it can be planned.

Each phase requests the native fee quote for its exact unsigned payload,
validates the authority payer, Nexus component, asset and cap, and signs that
exact quoted payload. The signed transaction, native instruction vector, fee
quote, and any alias plan are persisted before a durable dispatch claim.
A finite transaction lifetime is checked before its first submission.

## Recovery and verification

The owner-private operation directory has a descriptor-bound exclusive lock.
Signed preparation and dispatch records are immutable. A claimed phase is
never submitted again, including after a timeout, crash, missing response,
or an absent pipeline lookup. A transaction that expires before its first
dispatch is not replaced automatically. Incomplete or changed retained files
fail closed. Keep the same journal directory and do not delete claim records.

Once-only dispatch is scoped to this durable journal; it is not a new ledger
idempotency API. Operators integrating existing claim paths must preserve and
bind them before activating this controller. Copying or abandoning a journal
does not establish that its transaction was never submitted.

Progress requires exact global and configured-peer local `StateApplied` at
the same nonzero height, plus successful authenticated details containing the
same native signed transaction bytes. These observations are labeled
`applied_verification_pending`. `deployment_complete` remains false until the
native completion layer validates independently anchored finality, the exact
execution commitment/inclusion, and all four validators' state. An unavailable
or invalid proof keeps the phase observations and returns the actionable
`verification_error` field; subsequent status/apply resumes verification.
The bounded proof synchronization layer can return `verification_sync_pending`
while retaining its verified cache, without submitting another transaction.
An authenticated peer still catching up, or a carrier newer than its captured
tip, yields `verification_peer_pending`. The same pending result covers an
exact HTTP 409 `ErrorEnvelope` with code `bridge_finality_attestation_failure`,
whose sole `finality_attestation_failure` detail has reason `TipChanged` and its
`tip_mismatch` payload: the requested, applied and consensus-decision heights
differ. The SDK requires the payload and envelope to bind the request's height,
challenge, node and network. This unsigned progress response never proves
finality. Generic 404s, conflicting reasons or payloads, and invalid attestations
remain errors. Apply retries only explicit
progress outcomes within its deadline; other verification errors stop the call.
A successful completion names its immutable `completion_receipt`; exact verified
canonical carrier bytes are retained beside the proof cache and per-peer receipt.
Independent peer reads run in a bounded four-worker group. One coordinator
verifies the successor chain and owns journal publication. Completion requires
all four peer checks. Each later completion uses a fresh challenge and fresh
validator state; a saved completion receipt alone cannot establish current
success.

The typed `VerificationRequestV1` carries the original catalog/overlay
baseline, expected additions and grant, paid alias intent, each retained wire
SHA256/instruction vector/alias plan, pipeline observations, and committed
transaction DTO. It appears in the `verification` field of `apply` and `status`
output. The integrated native completion layer verifies independently anchored
finality, the exact native `ExecutionCommitment` and canonical inclusion,
and all four validators' final state against the independently selected trust
profile. Neither a single peer observation nor a caller-supplied commitment is
itself that proof.
