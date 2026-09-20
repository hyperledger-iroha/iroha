# Iroha CLI Client

The `iroha` command-line client exposes the operator and ledger workflows for
the first Hyperledger Iroha 3 release. It builds on the reusable `iroha` client
crate. See [Operate Iroha 3 via CLI](https://docs.iroha.tech/get-started/operate-iroha-via-cli.html)
for the current tutorial.

Within this workspace, `crates/iroha` is the reusable Rust client library and
`crates/iroha_cli` is the crate that builds the `iroha` command-line binary.

## Installation

**Requirements:** install
[Rust 1.93.1](https://www.rust-lang.org/learn/get-started), the toolchain pinned
for this workspace in the repository-root `rust-toolchain.toml`.

Build Iroha and its binaries:

```bash
cargo build -p iroha_cli --bin iroha
```

The binary is written to `target/debug/iroha` (`target/debug/iroha.exe` on
Windows).

See [Install Iroha 3](https://docs.iroha.tech/get-started/install-iroha.html) for
the current installation instructions.

## Usage
The CLI will attempt to detect your system language for messages. Use `--language <CODE>` to override this selection.
For automation, prefer `--output-format json --machine` to suppress startup chatter and fail fast when `client.toml` is missing.

Validator summaries retain complete activation heights and tenure bounds.
Space Directory and ZK JSON inputs use Norito's shared JSON nesting limit
(`MAX_JSON_VALUE_NESTING_DEPTH`, currently 33 including the root value).
Local contract durable-state fixtures require exact NFC path spelling and
reject duplicate decoded JSON keys.

Use `iroha taira doctor` for read-only public-testnet diagnostics. Authorized
public reset writes belong to the durable `iroha taira public-reset apply`
coordinator. Retry the same apply command with the same inventory and authorization;
the durable journal selects recovery inputs for the interrupted phase. Its low-level `write-canary` child accepts exactly one ordered
operation and one prepare, retained-envelope submit, or read-only recovery
action; it is not a one-shot operator command. Keep onboarding tokens and all
signing inputs in owner-only runtime files outside the repository.

A sealed occupied deployment uses the native
[`prepare-dispatcher-transition` / `dispatcher-transition` owner](DISPATCHER_TRANSITION.md)
to advance the fixed dispatcher and its five guards before preparing the next
reset. The completed transfer carries its preparation proof; the owner preserves
current runtime bindings and durable deployment history.

Reset input validation checks the complete action timeout budget before scanning
artifacts or reading signing custody. The install budget counts every required
artifact upload, including `kagami`, plus each validator's stage and install
actions. All four beacon providers activate before the epoch supervisor starts;
restart qualification follows that required barrier. Prepared Inrou stage files
use mode0600; retained runtime snapshots use mode0400. Both remain owner-only, direct,
singly linked files, with unchanged content verification.

Journal admission holds one exclusive lock through classification and execution.
The journal owner explicitly unlocks it on drop, so descriptors inherited by
unrelated forked children cannot delay the next admission.

Automation that already retains a private client file uses `--config-fd <FD>`
with `--config-source-path <absolute-original-path>`. The descriptor is read
directly, without environment overrides; the source path provides provenance
and the base for relative paths and is never reopened. Descriptors must be
read-only, owner-private regular files. Onboarding prepare/submit similarly
accepts `--onboarding-token-fd <FD>` instead of `--onboarding-token-file`.
Do not pass descriptor pseudo-paths through the ordinary file options.

Public node onboarding is deliberately a single future surface:
`iroha taira join --data-dir <owner-only-directory>`. It will consume the
published signed bootstrap bundle, generate local keys, and join as a
permissionless observer with no operator-issued admission token. Validator
activation is a separate on-chain transition through the existing staking and
peer lifecycle after the node has synchronized; it does not use a parallel
off-chain token format. The command and bundle are not shipped yet. The
disposable four-validator devnet is qualification tooling, not a way to join
the public testnet.

### Scaling load terminal handoff

`tx load` requires a fresh nonzero 64-character lowercase hexadecimal
`--invocation-id`. Machine execution emits one bounded JSON line only after
resource collection, workload checks, exact global and peer-local Applied
observations, and durable journal/trace publication complete. The receipt binds
that invocation, pair/variant/seed, resource budget, scheduled request count,
and raw SHA-256 plus byte length of both original output files. Retained native
file and parent handles remain checked through the actual reply flush.

The scaling launcher uses the original global client descriptor, each original
`--account-config` path, and peer3's original `--local-observer-config` path.
It supplies `--fee-payer authority` and every schedule, concurrency and resource
bound explicitly. Account, observer and resource options take original paths;
they do not accept descriptor pseudo-paths. A zero exit and terminal receipt
establish transport custody; joined journal, resource and canonical proof
replay determine whether a trial passes.

### Local SoraFS artifacts

Local SoraFS compilation and packaging run without client configuration:

```sh
iroha app sorafs toolkit compile --source contract.ko \
  --bytecode-out artifacts/contract.to --json-out artifacts/contract.json
iroha app sorafs toolkit pack artifacts/contract.to \
  --car-out artifacts/contract.car --manifest-out artifacts/contract.manifest.to
```

Compilation accepts `--source -` for stdin and publishes the compiler manifest
and authenticated build sidecars with the artifact. Its JSON summary includes
the exact byte length, BLAKE3 digest, ABI version, and source origin. Both toolkit
operations also support machine output without a populated `client.toml`.

### Client configuration

Select a public network with `[account].profile`. The supported `taira` and
`minamoto` profiles derive the correct I105 chain discriminant; the top-level
`chain` value does not select that profile.

```toml
chain = "fc56984b-2be7-431d-840e-21514d1883f0"
torii_url = "https://taira.sora.org/"

[account]
domain = "universal"
profile = "taira"
public_key = "..."
private_key = "..."
```

For a custom network, set `[account].chain_discriminant` explicitly instead.
The corresponding environment overrides are `ACCOUNT_PROFILE` and
`ACCOUNT_CHAIN_DISCRIMINANT`.

The CLI owns two optional filesystem settings that are deliberately absent from
the reusable Rust SDK configuration:

```toml
[connect]
queue_root = "/var/lib/iroha/connect"

[soracloud]
http_witness_file = "/run/iroha/canonical-request-witness.json"
```

`connect.queue_root` defaults to `~/.iroha/connect`. Soracloud mutation commands
load the witness through a bounded, change-detecting reader and validate its
schema, account, exact network request hash, and signer set before sending it.
Configured relative paths resolve from the directory containing the client TOML file.

### Fixed-schedule transaction collection

`iroha tx load` uses persistent SDK clients to prepare and submit a fixed
open-loop workload. Every request must reach state-resolved Applied globally
and on the required local observer at the same block height within the original
drain deadline. The fixed launcher selects the original peer whose stopped Kura
will be inspected. It supplies that peer's client configuration, a funded account
pool, an explicit fee payer and the bounded resource sampler inputs:

```sh
iroha --config client.toml --fee-payer authority tx load \
  --invocation-id "${INVOCATION_ID}" \
  --pair-index 1 --variant one_lane --seed "${PAIR_SEED}" \
  --offered-load-tps "${OFFERED_TPS}" --warmup-seconds "${WARMUP_SECONDS}" \
  --measurement-seconds "${MEASUREMENT_SECONDS}" --drain-seconds "${DRAIN_SECONDS}" \
  --max-submission-lag-ms "${SUBMISSION_LAG_MS}" \
  --account-config "${ACCOUNT_0_CONFIG}" --account-config "${ACCOUNT_1_CONFIG}" \
  --account-config "${ACCOUNT_2_CONFIG}" --account-config "${ACCOUNT_3_CONFIG}" \
  --local-observer-config "${PEER_3_CLIENT_CONFIG}" \
  --resource-program "${ABSOLUTE_PYTHON_PATH}" \
  --resource-worker "${ABSOLUTE_RESOURCE_WORKER_PATH}" \
  --resource-config "${OWNER_ONLY_PROBE_CONFIG}" \
  --resource-budget-sha256 "${PUBLIC_RUN_BUDGET_SHA256}" \
  --resource-capture-dir "${ABSENT_ABSOLUTE_CAPTURE_DIR}" \
  --resource-interval-ms "${SAMPLING_INTERVAL_MS}" \
  --resource-timeout-ms "${SAMPLING_TIMEOUT_MS}" \
  --resource-max-start-lag-ms "${SAMPLING_START_LAG_MS}" \
  --trace-out "${ABSENT_ABSOLUTE_TRACE_PATH}" \
  --diagnostic-out "${ABSENT_ABSOLUTE_JOURNAL_PATH}"
```

Use repeated `--account-config` paths to select an ordered pool of independently
funded signing accounts on the same chain and genesis network as the local
observer. The pool contains 4 through 64 accounts in multiples of four, and each
nonempty cohort contains complete pool rounds. Logical identities select accounts
deterministically and bind self-owned account metadata inserts. The collector
verifies the complete account effects after draining; routing, fee contention and
real workload representativeness still require qualification. Configuration files
and signing credentials remain local.

Preparation, submission, outstanding observations and diagnostic recording have
explicit fixed bounds. A missed schedule, exhausted local capacity, unknown or
failed submission, authoritative terminal failure, or incomplete drain fails
collection; no transaction is automatically replayed. `--help` lists the
lookahead, concurrency and polling controls. Global and local reads share the same
observation capacity and fixed clock. The trace retains global observation latency,
including polling and transport delay; waiting for the local peer does not rewrite
those timestamps or extend the drain deadline.

The new diagnostic JSON-lines file retains the schedule, exact hashes,
observations and final outcomes. The strict V1 trace is published without
replacing an existing file only after every scheduled request is acknowledged
and state-applied globally and locally at the same height within its phase deadline,
and the journal reaches durable storage. Local hash, scope, provenance, height and
observation time are retained separately so the launcher can verify the local
barrier before stopping the peer. Both files have hard byte bounds; oversized experiments fail instead
of truncating records. Endpoint URLs and external error text are omitted; the
journal retains fixed failure stages and bounded status classifications. No
private key or authentication header is written.

This command supplies the transaction-observation component of
[G-SCALE](../../specs/sumeragi_v2_multilane_scaling_gate.md). Trial-adapter wiring,
production collector capacity, deployment/routing and resource qualification,
and the five real paired trials remain separate completion requirements.

### Transaction waits

Use the built-in wait flow instead of shell polling:

```bash
iroha tx status --hash <SIGNED_TX_HASH> --wait
iroha contract call --contract-alias router::dex.universal --entrypoint swap \
  --draft-intent-file trusted-swap-intent.json --wait
iroha contract call --contract-alias router::dex.universal --entrypoint swap --simulate
```

Every non-simulated contract call must provide `--draft-intent-file`. The
secret-free JSON file is the caller-trusted exact contract invocation (resolved
address, code hash, entrypoint, and canonical argument record) plus the exact
final transaction metadata. Build it from the locally verified artifact/schema
and an authenticated deployment binding; never copy either value from the Torii
draft response. The CLI keeps this intent off wire and refuses to sign or return
an unsigned payload unless every signature-bound field matches it.

Run `iroha tools markdown-help` for the complete reference generated from the
installed CLI.

### Atomic private-settlement online auditor

The governed online-auditor flow uses one purpose-specific approval key from
the global `--operator-private-key-file` option and a distinct hybrid capsule
decryption key from an owner-only runtime file. It pins the four ordered Torii
endpoints to a separately governed committee-authority record, verifies each
responder's purpose-separated BLS attestation and proof of possession, requires
one exact three-of-four view, evaluates the decrypted capsule at the middle
ordered node-authoritative height so one outlier cannot choose it, then requires
an exact three-of-four, roster-authenticated approval acknowledgement. Neither
the capsule plaintext nor either secret is
printed. The signing key must be listed in the active local audit policy and
must not be a committee consensus key.

```bash
iroha --operator-private-key-file /run/secrets/aps-auditor-signing.key \
  nexus private-settlement audit-online \
  --committee-endpoint https://validator-1.example/ \
  --committee-endpoint https://validator-2.example/ \
  --committee-endpoint https://validator-3.example/ \
  --committee-endpoint https://validator-4.example/ \
  --committee-authority /etc/iroha/aps-committee-authority.json \
  --payload-digest <LEG_PAYLOAD_DIGEST> \
  --pool-governance /run/secrets/aps-pool-governance.json \
  --auditor-decryption-key-file /run/secrets/aps-auditor-hybrid.json \
  --business-policy /run/secrets/aps-business-policy.json \
  --decision approve
```

The committee-authority file must come from the participant dataspace's
governed configuration; endpoint order must match its ordered validator roster
exactly. Because it is the local trust anchor, it and the three restricted
input files must be absolute, owner-owned, singly linked regular files with
exact mode `0600`; on Linux they must be xattr-free, while macOS permits only
the exact `com.apple.provenance` metadata attribute. Extended ACL entries and
all other xattrs are rejected, and final path components cannot be symlinks. Files are opened
nonblocking and without following the final component, then rechecked through
the retained descriptor before and after the bounded read. The
restricted files must reside on a qualified local filesystem whose
descriptor-bound ACL and xattr APIs expose every effective access grant; NFS
and SMB custody is not qualified by POSIX mode bits alone. The
decryption-key file's strict Norito JSON shape is:

```json
{
  "version": 1,
  "x25519_secret_hex": "<64 lowercase hexadecimal characters>",
  "ml_kem_768_secret_hex": "<canonical lowercase hexadecimal ML-KEM-768 secret>"
}
```

The business-policy file is also strict Norito JSON. It binds one exact
network, route, opaque pool, audit-policy lineage/revision/key epoch, canonical
non-empty allowlists for payer, recipient, sponsor, and asset, inclusive amount
and reimbursement ceilings, a memo-size ceiling, canonical allowed/required
policy-reference lists, and a maximum remaining-height window. Unknown fields,
wildcard identity/asset lists, unordered or duplicate values, a zero window, or
a required reference absent from the allowed list fail closed. There is no
environment-variable fallback. `--decision approve` is necessary but not
sufficient: the decrypted leg must also match every business-policy constraint.
Omitting the decision or using `--decision reject` cannot create or submit an
approval. Decryption-key and pool-governance files are limited to 16 KiB; the
bounded business-policy file is limited to 256 KiB.

Refer to [Iroha Special Instructions](https://docs.iroha.tech/blockchain/instructions.html) for more information about Iroha instructions such as register, mint, grant, and so on.

### Sumeragi consensus helpers

Operator reads require an explicit runtime key whose public key is allowlisted by the node.
Pass either `--operator-private-key-file /absolute/path` or an inherited read-only descriptor with
`--operator-private-key-fd FD` (3–65535). These options are mutually exclusive. The CLI does not
read this credential from the environment or client TOML and never substitutes the account key.
On Unix the key must be in an owner-owned, singly linked regular file with exact mode `0600`.
Descriptor reads use the inherited file directly, preserve the caller's file offset and never
reopen a path. Requests are signed for the exact `network_id` in `client.toml`.

Fetch the exact reducer-owned consensus status:

```bash
iroha --operator-private-key-file /run/secrets/iroha/operator.key \
  --output-format text ops sumeragi status
```

> `--output-format text` prints protocol version, height, view, reducer phase, leader, body state, persistence state, committed height, and restart requirement.

Fetch non-authoritative pipeline, queue, NPoS election, and Nexus lane diagnostics separately:

```bash
iroha --operator-private-key-file /run/secrets/iroha/operator.key \
  --output-format text ops sumeragi diagnostics
```

Consensus VRF epoch and penalty snapshots are retired together with the
`vrf-epoch` and `vrf-penalties` subcommands. Production randomness comes from
finalized global threshold-beacon pulses. Use the current read-only status and
equivocation-evidence commands:

```bash
iroha --operator-private-key-file /run/secrets/iroha/operator.key \
  --output-format text ops sumeragi evidence count
iroha --operator-private-key-file /run/secrets/iroha/operator.key \
  --output-format text ops sumeragi evidence list --limit 100
```

Tip: You can combine these with `jq` for consistency checks.

### SoraFS gateway helpers

Generate a TOML snippet with default gateway settings (rate limits and ACME hosts):

```bash
iroha app sorafs gateway template-config --host gateway-a.example.com --host gateway-b.example.com
```

Pipe the output into your node configuration to bootstrap `torii.sorafs_gateway`.

Derive canonical and vanity hostnames for a provider (useful for direct-mode tooling):

```bash
iroha app sorafs gateway generate-hosts --provider-id 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa --chain-id nexus
```

The command prints JSON with the canonical and vanity hostnames derived from the provider id.

Plan a direct-mode rollout by inspecting manifest metadata and (optionally) admission envelopes:

```bash
iroha app sorafs gateway direct-mode plan \
  --manifest fixtures/sorafs_manifest/example_manifest.to \
  --provider-id 1111111111111111111111111111111111111111111111111111111111111111
```

The command returns a JSON plan capturing host mapping, direct-CAR endpoints, and capability flags
detected from the manifest/admission bundle.

Apply the plan to generate a configuration snippet (the snippet targets `torii.sorafs_gateway` and
the new `torii.sorafs_gateway.direct_mode` table):

```bash
iroha app sorafs gateway direct-mode enable --plan direct-mode-plan.json
```

To restore default gateway settings, emit the rollback snippet:

```bash
iroha app sorafs gateway direct-mode rollback
```

### ZK vote tally (app API convenience)

The CLI provides helpers for app‑facing ZK endpoints. For example, to fetch a vote tally for an election id via Torii:

```bash
iroha app zk vote tally --election-id demo-election-1
```

This posts to `/v1/zk/vote/tally` and prints the snapshot-bound JSON response, e.g. `{ "evaluated_block_height": 42, "evaluated_block_hash": "<64 lowercase hex characters>", "finalized": true, "tally": [42, 58] }`. An unknown election is an HTTP `404`; it is never represented as an empty tally.

### Governance helpers (app API convenience)

Build governance transaction skeletons and query governance state via Torii app endpoints. The server does not sign or submit transactions; clients assemble and POST to `/v1/pipeline/transactions`.

- Propose deployment of IVM bytecode via governance:

```bash
iroha app gov deploy propose \
  --contract-address irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw \
  --code-hash <64-lowercase-hex> --abi-hash <64-lowercase-hex> \
  --abi-version 1
```

Responds with `{ proposal_id, tx_instructions: [{ wire_id, payload_hex }] }`.
The certificate lifecycle and enactment height are Core-derived; this command
accepts no referendum window or voting mode.

- Submit a ballot (auto-detects referendum mode unless overridden):

```bash
iroha app gov vote --referendum-id r1 --backend halo2/ipa \
  --envelope-b64 BASE64_ENVELOPE \
  [--public public.json]
```

For plain (non-ZK) referenda provide the required fields explicitly:

```bash
iroha app gov vote --referendum-id r1 --mode plain --owner <canonical-i105-owner> \
  --amount 1000 --duration-blocks 6000 --direction Aye
```

Proposal-backed Parliament decisions are certificate driven. The node advances
certified attempts at their consensus-scheduled due height; the CLI does not
expose client finalization or proposal-enactment drafts.

- Apply protected namespaces on the server (admin/testing):

  iroha app gov protected apply --namespaces apps,system

- Build governance metadata for protected-namespace admission:

  iroha app gov deploy meta --contract-address irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw

- Audit a governed contract binding by canonical address or alias:

  iroha app gov deploy audit --contract-address irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw

- Combined manifest command (prints or saves when --out is provided):

  iroha contract manifest get --code-hash 0xAA..AA
  iroha contract manifest get --code-hash 0xAA..AA --out manifest.json

```

- Read governance state:

```bash
iroha app gov proposal get --id 0123...ABCD
iroha app gov locks get --referendum-id r1
iroha app gov referendum get --referendum-id r1
iroha app gov tally get --referendum-id r1

Governance events (subscribe via `iroha ledger events`)
- ProposalSubmitted, ProposalRejected, ProposalEnacted
- ParliamentAttemptCreated, ParliamentLifecycleTransitionApplied
- ReferendumOpened, ReferendumClosed, ReferendumDecided
- BallotAccepted { mode, weight }, BallotRejected { reason }
- LockCreated { owner, amount, expiry }, LockExtended { ... }, LockUnlocked { ... },
  LockSlashed { ... }, LockRestituted { ... }
- CitizenRegistered, CitizenRevoked, ThresholdKeyLifecycleApplied
```

- Stream governance events:

```bash
iroha ledger events governance [--proposal-id 0123...ABCD | --referendum-id r1]
```


### ZK Verifying Key registry (register/update)

The CLI builds, quotes, signs, and submits VK registry transactions with the account and key from
the active client configuration. VK JSON files contain public registry data only; signing
authorities and private keys are not accepted in these files.

Register a verifying key (provide either `vk_bytes` as base64 or `commitment_hex`):

The optional `namespace` field defaults to `core` when omitted or `null`. Set it
to `kagemusha_v1` for KAGEMUSHA V1 verifier records. Explicit namespace values
must be non-empty and must not contain leading or trailing whitespace.

```bash
cat >vk_register.json <<'JSON'
{
  "backend": "halo2/ipa",
  "name": "vk_add",
  "version": 1,
  "circuit_id": "circuit_alpha",
  "namespace": "core",
  "public_inputs_schema_hash_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "vk_bytes": "BASE64..."
}
JSON
iroha app zk vk register --json vk_register.json
```

Update an existing verifying key (version must increase). You may supply only the commitment:

```bash
cat >vk_update.json <<'JSON'
{
  "backend": "halo2/ipa",
  "name": "vk_add",
  "version": 2,
  "circuit_id": "circuit_alpha",
  "public_inputs_schema_hash_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "commitment_hex": "0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd"
}
JSON
iroha app zk vk update --json vk_update.json
```

Read a VK record as JSON:

```bash
iroha app zk vk get --backend halo2/ipa --name vk_add
```

Compute the schema hash expected in the VK registry:

```bash
# From a Norito-encoded OpenVerifyEnvelope
iroha app zk schema-hash --norito proof_env.norito
# Or from raw public-input bytes (hex)
iroha app zk schema-hash --public-inputs-hex 0x0123abcd...
```

### ZK attachments (app API convenience)

Upload an attachment (set Content-Type appropriately):

```bash
iroha app zk attachments upload --file ./proof.json --content-type application/json
```

List attachments, download one, and delete it:

```bash
iroha app zk attachments list
iroha app zk attachments get --id 0123ab... --out ./downloaded.bin
iroha app zk attachments delete --id 0123ab...

# Clean up attachments (client-side filtering)
# Preview Norito attachments older than 7 days
iroha app zk attachments cleanup --content-type application/x-norito --older-than-secs 604800 --summary
# Delete all JSON attachments created before a timestamp
iroha app zk attachments cleanup --content-type application/json --before-ms 1725500000000 --yes

```

### Confidential asset ingress

The first-release CLI intentionally has no generic `zk shield` command. KAGEMUSHA V1
top-ups use the payer-signed, proof-bound `/v1/kagemusha/top-up` operation and its pooled
reserve; peer payments never mutate that reserve. The generic confidential-asset verifier
settings do not authorize callers to inject opaque KAGEMUSHA commitments.

Encrypted memo envelopes remain available as a local wallet utility:

```bash
iroha app zk envelope --ephemeral-pubkey 0101... --nonce-hex 0202... \
  --ciphertext-b64 AQIDBA== --print-json --output memo.bin
```

### Register a ZK-capable asset

```bash
iroha app zk register-asset --asset <base58-asset-definition-id> \
  --vk-unshield halo2/ipa:vk_unshield
```

Register and inspect the referenced verifying keys with `iroha app zk vk register`,
`iroha app zk vk update`, and `iroha app zk vk get`. The first-release confidential-asset
model rejects `vk_shield`; KAGEMUSHA V1 top-up and redemption instead use the authenticated
release artifact set and the generic KAGEMUSHA routes. No asset-bound private-transfer
verifier or generic transfer/withdrawal ISI exists.

### ZK verify batch

```bash
iroha app zk verify-batch --norito ./batch.norito
# Or pass a JSON array of base64-encoded Norito envelopes:
iroha app zk verify-batch --json ./batch.json
```

Run the full sample sequence:

```bash
cd fuzz/attachments/zk
bash ./run.sh
```

## Examples

:grey_exclamation: All examples below are Unix-oriented. If you're working on Windows, we would highly encourage you to consider using WSL, as most documentation assumes a POSIX-like shell running on your system. Please be advised that the differences in the syntax may go beyond executing `iroha.exe` instead of `iroha`.

### Create a domain and alias lease

Ordinary transactions create domains through the declarative alias planner so the SNS lease, owner capabilities, and domain state are checked and applied atomically. Put the secret-free setup request in a JSON file, plan it against live state, then verify and submit that exact plan locally:

```bash
iroha app alias setup plan --intent-file alias-setup.json --plan-file alias-plan.json
iroha app alias setup apply --plan-file alias-plan.json
```

Raw `ledger domain register` is reserved for genesis/bootstrap and is not exposed as an ordinary CLI mutation.

### Create new Account

To create an account, specify the entity type (`account`) and the command (`register`). Then pass a canonical I105 `AccountId` via `--id`:

```bash
iroha ledger account register \
  --id "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"
```

### Mint Asset to Account

To add assets to the account, you must first register an Asset Definition. Specify the `asset` entity and then use the `register` and `mint` commands respectively. Here is an example of adding Assets of the type `Quantity` to the account:

```bash
iroha ledger asset register --id "6UoZbEC1BVBbDo99CSvY7qud73yh" --type Quantity
iroha ledger asset mint --id "<ASSET_ID>" --quantity 1010
```

With this, you created an asset of type `Quantity` under the canonical asset-definition identifier `6UoZbEC1BVBbDo99CSvY7qud73yh`, and then gave `1010` units of that asset to a target account.

### Query Account Assets Quantity

You can use Query API to check that your instructions were applied and the _world_ is in the desired state. For example, to know how many units of a particular asset an account has, use `asset get` with the specified account and asset:

```bash
iroha ledger asset get --id "<ASSET_ID>"
```

This query returns the quantity of the selected account-scoped asset.

You can also filter based on either account, asset or domain id by using the filtering API provided by the Iroha client CLI. Generally, filtering follows the `iroha ledger ENTITY list filter PREDICATE` pattern, where ENTITY is asset, account or domain and PREDICATE is condition used for filtering serialized using JSON (check `iroha::data_model::predicate::value::ValuePredicate` type).

Here are some examples of filtering:

```bash
# Filter domains by id
iroha ledger domain list filter '{"Atom": {"Id": {"Atom": {"Equals": "wonderland"}}}}'
# Filter accounts by domain
iroha ledger account list filter '{"Atom": {"Id": {"Domain": {"Atom": {"Equals": "wonderland"}}}}}' 
# Filter asset by domain
iroha ledger asset list filter '{"Or": [{"Atom": {"Id": {"Definition": {"Domain": {"Atom": {"Equals": "wonderland"}}}}}}, {"Atom": {"Id": {"Account": {"Domain": {"Atom": {"Equals": "wonderland"}}}}}}]}'
```

### Contract Developer Workflow

Use [Musubi](../musubi/README.md) for Kotodama package projects. `Musubi.toml`
owns library, contract, test, and dependency declarations. `musubi new`,
`musubi check`, `musubi build`, and `musubi test` use the native compiler and
an explicit locked module graph. Named network bindings select the runtime
client config and exact contract alias for `musubi deploy` and `musubi view`.

The low-level `iroha contract` commands inspect artifacts, derive addresses,
manage aliases, perform calls and views, and run local bytecode diagnostics.
They do not own another project manifest or package build workflow.

### Execute IVM transaction

Use `--file` to specify a path to the IVM bytecode file (typically a `.to` file produced by compiling Kotodama `.ko` source):

```bash
iroha ledger transaction ivm --file /path/to/contract.to
```

Or skip `--file` to read IVM bytecode from standard input:

```bash
cat /path/to/contract.to | iroha ledger transaction ivm
```

These subcommands submit the provided IVM bytecode as an `Executable` to be executed outside a trigger context.

### Execute Multi-instruction Transactions

The reference implementation of the Rust client, `iroha`, is often used for diagnosing problems in other implementations.

To test transactions in the JSON format (used in the genesis block and by other SDKs), pipe the transaction into the client and add the `transaction stdin` subcommand to the arguments:

```bash
cat fuzz/cli_dsl/transaction_log_message.json | iroha ledger transaction stdin
```

### Request arbitrary query

```bash
cat fuzz/cli_dsl/iterable_accounts_query.json | iroha ledger query stdin
```

### Experimental: IDs-only projection (`--select ids`)

When built with the `ids_projection` feature, the CLI can request that iterable queries return only IDs instead of full objects by passing `--select ids`.

Examples (feature-gated):

```bash
# List only domain identifiers (requires --features ids_projection)
cargo run --bin iroha --features ids_projection -- \
  ledger domain list all --select ids

# List only account identifiers with sorting/pagination
cargo run --bin iroha --features ids_projection -- \
  ledger account list all --select ids --sort-by-metadata-key rank --order desc --offset 10 --limit 5
```

Expected output format is the same JSON as for full objects, but the entries are now identifier values, for example:

```json
[
  "w2",
  "w1",
  "w0"
]
```

Note: This feature is experimental and off by default; enable it for testing and iterative development. Behavior and flags may change.
## Rendering Markdown Help

Ensure the CLI builds, then run:

```bash
make docs-cli
# or
cargo run -p iroha_cli --bin iroha -- tools markdown-help
```

The full Iroha CLI reference is rendered from the live command tree and is not
checked into the repository. Redirect it to an operator-chosen path when a
standalone copy is needed. Kagami retains its smaller checked-in
`CommandLineHelp.md` snapshot and validates that snapshot in its unit tests.

The fixed scaling generator accepts its private development seed only through
`kagami localnet --scaling-lanes <1|4> --seed-fd <FD>`. The fixed Python owner
passes an anonymous read-only nonblocking pipe containing exactly 64 lowercase
hexadecimal bytes followed by EOF. The descriptor is consumed before native
output generation; the seed never enters process arguments or public receipts.
Generic localnet development generation has its own independent input policy.

`iroha tx collect-scaling-inputs` requires the independently retained complete
Native context archive via `--native-contexts`, `--native-contexts-sha256`, and
`--native-contexts-max-bytes`. Its canonical `Vec<NativeLaneContextsEvidenceV1>`
contains one post-carrier context witness per height, including empty sets, in
height order from genesis through the exact stopped tip. The collector verifies
that archive against the anchored finality chain and exact Kura carriers before
publishing `Vec<FinalizedNativeContextV1>` and committed Network output queries.
A genesis context alone cannot supply this historical execution evidence.
