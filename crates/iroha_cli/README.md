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

Use `iroha taira doctor` for read-only public-testnet diagnostics. Authorized
public reset writes belong to the durable `iroha taira public-reset apply`
coordinator. Its low-level `write-canary` child accepts exactly one ordered
operation and one prepare, retained-envelope submit, or read-only recovery
action; it is not a one-shot operator command. Keep onboarding tokens and all
signing inputs in owner-only runtime files outside the repository.

Public node onboarding is deliberately a single future surface:
`iroha taira join --data-dir <owner-only-directory>`. It will consume the
published signed bootstrap bundle, generate local keys, and join as a
permissionless observer with no operator-issued admission token. Validator
activation is a separate on-chain transition through the existing staking and
peer lifecycle after the node has synchronized; it does not use a parallel
off-chain token format. The command and bundle are not shipped yet. The
disposable four-validator devnet is qualification tooling, not a way to join
the public testnet.

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

Refer to [Iroha Special Instructions](https://docs.iroha.tech/blockchain/instructions.html) for more information about Iroha instructions such as register, mint, grant, and so on.

### Sumeragi consensus helpers

Operator reads require an explicit runtime key file whose public key is allowlisted by the node.
Pass the absolute file path on every invocation; the CLI does not read this credential from the
environment or client TOML and never substitutes the account key. On Unix the file must be an
owner-owned, singly linked regular file with exact mode `0600`. Requests are signed for the exact
`network_id` in `client.toml`.

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
to `offline_kagemusha` for Kagemusha verifier records. Explicit namespace values
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

The first-release CLI intentionally has no generic `zk shield` command. Public-to-confidential
movement is admitted only by the proof-bound Kagemusha V4 top-up flow. In asset policy
configuration, the presence of `vk_shield` enables only that authenticated top-up circuit;
it does not enable an opaque caller-supplied commitment.

Encrypted memo envelopes remain available as a local wallet utility:

```bash
iroha app zk envelope --ephemeral-pubkey 0101... --nonce-hex 0202... \
  --ciphertext-b64 AQIDBA== --print-json --output memo.bin
```

### Register a ZK-capable asset

```bash
iroha app zk register-asset --asset <base58-asset-definition-id> \
  --vk-unshield halo2/ipa:vk_unshield \
  --vk-shield <canonical-kagemusha-top-up-vk>
```

Register and inspect the referenced verifying keys with `iroha app zk vk register`,
`iroha app zk vk update`, and `iroha app zk vk get`. The node rejects a `vk_shield`
record that is not the canonical Kagemusha top-up circuit and schema. In the first-release
surface, `vk_unshield` is the Kagemusha redemption verifier; no asset-bound private-transfer
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

Use `iroha contract dev` when a repository has an `iroha.contracts.toml`
manifest. The manifest is the source of truth for contract sources, aliases,
profiles, Kotodama tests, and smoke declarations.

A single-file seiyaku uses `source`. A typed-module seiyaku uses
`kotodama_project`; these fields are mutually exclusive. The referenced
version-1 Norito JSON manifest declares the exact root imports, locked package
identities, module paths, exports, and transitive imports used by `dev check`,
`dev build`, app bundle construction, and deployment:

```toml
[[contracts]]
name = "demo.app"
alias = "app::universal"
kotodama_project = "kotodama.project.json"
artifact = "artifacts/app.to"
```

```json
{
  "version": 1,
  "root": "contracts/app.ko",
  "imports": [{"alias": "Math", "package": "example/math@1.0.0"}],
  "packages": [{
    "identity": "example/math@1.0.0",
    "modules": ["modules/math.ko"],
    "exports": ["value"],
    "imports": []
  }]
}
```

No wildcard, sibling-file, private-export, or source-order inference is used.
Unknown manifest fields and paths escaping the project directory are rejected.
Diagnostics report both the locked package identity and the unchanged logical
source path.

```bash
iroha contract dev doctor --manifest iroha.contracts.toml --profile local
iroha contract dev check --manifest iroha.contracts.toml --profile local
iroha contract dev build --manifest iroha.contracts.toml --profile local
iroha contract dev test --manifest iroha.contracts.toml --coverage
iroha contract dev test --manifest iroha.contracts.toml --path-filter payments --filter rejects
iroha contract dev test --manifest iroha.contracts.toml --filter rejects_invalid_payment --exact
iroha contract dev schema --manifest iroha.contracts.toml --out docs/interface.md
```

Test source and function selection are deliberately separate: `--path-filter`
selects `.ko` test paths, while `--filter` selects test function names and
`--exact` requires a complete function-name match. A supplied filter that
matches no source or function fails instead of silently running a broader
suite.

`build` emits compiled `.to` artifacts plus adjacent `.manifest.json` and
`.interface.json` files. Source maps and budget reports are content addressed
under `.sidecars/<artifact-hash>/source-map.json` and `budget.json`; mutating
any deployable artifact field therefore selects a different diagnostic
sidecar. An authenticated commit record under `.fingerprints/` binds every
generated output and lets unchanged builds perform no compilation or output
rewrite. `check --locked` rejects missing or stale generated files, which lets
CI fail when checked-in interfaces or payload examples drift from the Kotodama
source.

`deploy`, `resume`, `call`, and `view` resolve contracts by manifest name and
reuse the existing contract app/deploy/call machinery with typed payload
validation from the compiled artifact when it is available.

`dev call` also requires `--draft-intent-file`. A smoke entry whose operation is
`call` must declare `draft_intent = "path/to/trusted-intent.json"`; the path is
resolved relative to `iroha.contracts.toml`. This deliberate hard cut prevents
an app route from substituting executable or metadata bytes that a wallet would
otherwise sign.

`doctor`, `call`, `view`, and `smoke` honor the selected profile's client
config, signer, default gas, and fee asset settings. `doctor` probes the live
Torii endpoint, block-height host surface, signature syscall availability, and
manifest admission path; `smoke` executes the manifest's declared view/call
cases against the resolved profile instead of acting as a parse-only check.

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
