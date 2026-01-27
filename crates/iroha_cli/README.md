# Iroha CLI Client

Iroha Client CLI is a "thin" wrapper around functionality exposed in the `iroha` crate. Specifically, it should be used as a reference for using `iroha`'s features, and not as a production-ready client. As such, the CLI client is not guaranteed to support all features supported by the client library. Check [Iroha 2 documentation](https://docs.iroha.tech/get-started/operate-iroha-2-via-cli.html) for a detailed tutorial on working with Iroha Client CLI.

## Installation

**Requirements:** the stable [Rust toolchain](https://www.rust-lang.org/learn/get-started) (the project pins `stable` via `rust-toolchain.toml`), installed and configured.

Build Iroha and its binaries:

```bash
cargo build
```

The above command will produce the `iroha` ELF executable file for Linux/BSD, the `iroha` executable for MacOS, and the `iroha.exe` executable for Windows, depending on your platform and configuration.

Alternatively, check out the [documentation](https://docs.iroha.tech/get-started/install-iroha-2.html) for system-wide installation instructions.

## Usage
The CLI will attempt to detect your system language for messages. Use `--language <CODE>` to override this selection.

See [Command-Line Help](CommandLineHelp.md).

Refer to [Iroha Special Instructions](https://docs.iroha.tech/blockchain/instructions.html) for more information about Iroha instructions such as register, mint, grant, and so on.

### Sumeragi parameters (K/r) — operator note

Consensus collector settings are controlled both by node config and on‑chain parameters:
- Config keys: `sumeragi.collectors.k` (K) and `sumeragi.collectors.redundant_send_r` (r)
- On‑chain: `SumeragiParameters { collectors_k, collectors_redundant_send_r }`

At startup, nodes adopt the on‑chain values and log a mismatch if config differs. Invalid values (`k == 0` or `r == 0`) are rejected at startup; oversized K/r fall back to the full commit topology when no collector plan can be built. This keeps pacing behavior consistent across validators.

### Sumeragi consensus helpers

Fetch status (leader, HighestQC, LockedQC, membership digest):

```bash
iroha --output-format text ops sumeragi status
```

> `--output-format text` prints leader/HighestQC/LockedQC, membership height/view/epoch/hash, RBC pressure, and VRF counters in one line.

Fetch latest per-phase latencies (ms):

```bash
iroha --output-format text ops sumeragi phases
# Example: propose=11 da=22 prevote=33 precommit=44 exec=55 witness=66 commit=77 ms | ema(propose=15 da=24 prevote=31 precommit=40 commit=70)
```

RBC status and sessions (throughput and active sessions):

```bash
iroha --output-format text ops sumeragi rbc status
# Example: active=2 pruned=10 ready=8 deliver=7 bytes=1234567

iroha --output-format text ops sumeragi rbc sessions
# Example: active=1 first=[h:1234 v:7 chunks=12/12 delivered=true] items=1
```

Fetch commit QC (if present) for a block hash:

```bash
iroha ops sumeragi commit-qc-get --hash BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2
# Prints JSON with fields: subject_block_hash, parent_state_root, post_state_root (hex), height, view, epoch,
# signers_bitmap (hex), bls_aggregate_signature (hex). If missing, commit_qc is null.
```

Tip: You can combine these with `jq` for consistency checks.

### SoraFS gateway helpers

Validate operator-maintained denylist files before deploying them to Torii:

```bash
iroha app sorafs gateway lint-denylist --path docs/source/sorafs_gateway_denylist_sample.json
```

The command checks required fields for each entry type (provider, manifest digest, CID, URL,
account ID, and alias), enforces canonical hex/base64 encoding, validates timestamps, and reports a
per-kind summary so operators can catch mistakes before rolling out new lists.

Generate a TOML snippet with default gateway settings (rate limits, denylist path, ACME hosts):

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

This posts to `/v1/zk/vote/tally` and prints the JSON response, e.g. `{ "finalized": true, "tally": [42, 58] }`.

### Governance helpers (app API convenience)

Build governance transaction skeletons and query governance state via Torii app endpoints. The server does not sign or submit transactions; clients assemble and POST to `/transaction`.

- Propose deployment of IVM bytecode via governance:

```bash
iroha app gov deploy propose \
  --namespace apps --contract-id my.contract.v1 \
  --code-hash 0123...ABCD --abi-hash 0123...ABCD \
  --abi-version v1 --window-lower 12345 --window-upper 12400 \
  --mode Plain
```

Responds with `{ ok, proposal_id, tx_instructions: [{ wire_id, payload_hex }] }`.

- Submit a ballot (auto-detects referendum mode unless overridden):

```bash
iroha app gov vote --referendum-id r1 --proof-b64 BASE64_PROOF \
  [--public public.json]
```

For plain (non-ZK) referenda provide the required fields explicitly:

```bash
iroha app gov vote --referendum-id r1 --mode plain --owner alice@domain \
  --amount 1000 --duration-blocks 6000 --direction Aye
```

- Finalize a referendum (compute tally, emit Approved/Rejected):

```bash
curl -sS -X POST -H 'Content-Type: application/json' \
  "$TORII/v1/gov/finalize" -d '{"referendum_id":"r1","proposal_id":"0123...ABCD"}' | jq .

- Build an enactment transaction (for an approved proposal):

  iroha app gov enact --proposal-id 0123...ABCD [--preimage-hash 00..00] [--window-lower H --window-upper H]

- Apply protected namespaces on the server (admin/testing):

  iroha app gov protected apply --namespaces apps,system

- Build an ActivateContractInstance skeleton (pass `--blocking` to submit via CLI context):

  iroha app gov instance activate --namespace apps --contract-id calc.v1 --code-hash 0xAA..AA [--blocking]

- List active instances for a namespace (admin/testing):

  iroha app gov instance list --namespace apps

- Combined manifest command (prints or saves when --out is provided):

  iroha app contracts manifest get --code-hash 0xAA..AA
  iroha app contracts manifest get --code-hash 0xAA..AA --out manifest.json

- List contract instances in a namespace (filters and pagination):

  iroha app contracts instances --namespace apps \
    [--contains calc] [--hash-prefix aabb] [--offset 0] [--limit 50] [--order cid_desc]

  Add --table to render a clean table instead of raw JSON (columns: Namespace, Contract ID, Code Hash):

  iroha app contracts instances --namespace apps --table [--short-hash]
```

- Read governance state:

```bash
iroha app gov proposal get --id 0123...ABCD
iroha app gov locks get --referendum-id r1
iroha app gov council
iroha app gov referendum get --referendum-id r1
iroha app gov tally get --referendum-id r1

Governance events (subscribe via `iroha ledger events`)
- ProposalSubmitted, ProposalApproved, ProposalRejected, ProposalEnacted
- ReferendumOpened, ReferendumClosed
- BallotAccepted { mode, weight }, BallotRejected { reason }
- LockCreated { owner, amount, expiry }, LockExtended { ... }, LockUnlocked { ... }
```

- Stream governance events:

```bash
iroha ledger events governance [--proposal-id 0123...ABCD] [--referendum-id r1]
```


### ZK Verifying Key registry (register/update)

The CLI wraps Torii VK registry endpoints to submit signed transactions.

Register a verifying key (provide either `vk_bytes` as base64 or `commitment_hex`):

```bash
cat >vk_register.json <<'JSON'
{
  "authority": "alice@wonderland",
  "private_key": {"algorithm":"ed25519","payload":"..."},
  "backend": "halo2/ipa",
  "name": "vk_add",
  "version": 1,
  "circuit_id": "circuit_alpha",
  "public_inputs_schema_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "vk_bytes": "BASE64..."
}
JSON
iroha app zk vk register --json vk_register.json
```

Update an existing verifying key (version must increase). You may supply only the commitment:

```bash
cat >vk_update.json <<'JSON'
{
  "authority": "alice@wonderland",
  "private_key": {"algorithm":"ed25519","payload":"..."},
  "backend": "halo2/ipa",
  "name": "vk_add",
  "version": 2,
  "circuit_id": "circuit_alpha",
  "public_inputs_schema_hex": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "commitment_hex": "0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd0123abcd"
}
JSON
iroha app zk vk update --json vk_update.json
```

Read a VK record as JSON:

```bash
iroha app zk vk get --backend halo2/ipa --name vk_add
```

Deprecate a VK (removes the record in v1):

```bash
cat >vk_deprecate.json <<'JSON'
{
  "authority": "alice@wonderland",
  "private_key": {"algorithm":"ed25519","payload":"..."},
  "backend": "halo2/ipa",
  "name": "vk_add"
}
JSON
iroha app zk vk deprecate --json vk_deprecate.json
```

Compute the schema hash expected in the VK registry:

```bash
# From a Norito-encoded OpenVerifyEnvelope
iroha app zk schema-hash --norito proof_env.norito
# Or from raw public-input bytes (hex)
iroha app zk schema-hash --public-inputs-hex 0x0123abcd...
```

### ZK attachments and prover reports (app API convenience)

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

### Sample shield/unshield flows

Shield public funds (append a shielded note commitment):

```
iroha app zk shield --asset rose#wonderland --from alice@wonderland \
  --amount 1000 --note-commitment 0123ABCD0123ABCD0123ABCD0123ABCD0123ABCD0123ABCD0123ABCD0123ABCD
```

Structured envelopes can be supplied directly instead of pre-generated Norito
bytes by passing the trio `--ephemeral-pubkey`, `--nonce-hex`, and
`--ciphertext-b64` (base64, including the Poly1305 tag). When omitted, the CLI
falls back to loading raw bytes via `--enc-payload`.

To materialize memo bytes or inspect them without broadcasting a shield, use:

```
iroha app zk envelope --ephemeral-pubkey 0101... --nonce-hex 0202... \
  --ciphertext-b64 AQIDBA== --print-json --output memo.bin
```

This writes the Norito-encoded envelope to `memo.bin` and prints both base64
and JSON (when `--print-json` is set) for wallet tooling.

Unshield with a proof attachment JSON:

```
cat > fuzz/attachments/zk/unshield_proof.sample.json <<'JSON'
{
  "backend": "halo2/ipa",
  "proof_b64": "BASE64_PROOF_BYTES",
  "vk_ref": { "backend": "halo2/ipa", "name": "vk_unshield" }
}
JSON

iroha app zk unshield --asset rose#wonderland --to alice@wonderland --amount 1000 \
  --inputs DEADBEEF...CAFE,0123ABCD...ABCD --proof-json fuzz/attachments/zk/unshield_proof.sample.json

### Register a ZK-capable asset (Hybrid)

```
iroha app zk register-asset --asset rose#wonderland \
  --allow-shield true --allow-unshield true \
  --vk-transfer halo2/ipa:vk_transfer --vk-unshield halo2/ipa:vk_unshield
```

### Verifying Key lifecycle (register/update/get) → then bind via register-asset

1) Prepare VK JSONs. Samples live under `fuzz/attachments/zk/`:

   - `vk_register.json` — fields: `authority`, `private_key`, `backend`, `name`, `version`, one of:
     - `vk_bytes` (base64) or `commitment_hex` (64-hex)
   - `vk_unshield.sample.json` — minimal unshield VK sample (backend/name preset to `halo2/ipa:vk_unshield`)
   - `vk_update.json` — same shape; `version` must increase
   - `vk_deprecate.json` — deprecate a VK record (status flips to Deprecated; retained subject to per‑backend cap)

2) Register verifying keys (transfer and unshield):

```
iroha app zk vk register --json fuzz/attachments/zk/vk_register.json
iroha app zk vk register --json fuzz/attachments/zk/vk_register.json \
  # adjust backend/name to create a second VK, e.g., vk_unshield
```

3) Inspect VKs:

```
iroha app zk vk get --backend halo2/ipa --name vk_transfer | jq .
iroha app zk vk get --backend halo2/ipa --name vk_unshield | jq .

### ZK Verify Batch (transparent Halo2 IPA)

Verify multiple OpenVerify envelopes in one request and print per‑item statuses:

```
# Given a Norito-encoded vector of envelopes
iroha app zk verify-batch --norito ./batch.norito

# Or a JSON array of base64-encoded Norito envelopes
# Example: ["BASE64(norito(OpenVerifyEnvelope))", "BASE64(...)"]
iroha app zk verify-batch --json ./batch.json
```
```

4) Update VK (version bump):

```
iroha app zk vk update --json fuzz/attachments/zk/vk_update.json
```

5) Bind VKs to an asset policy (Hybrid) using the new helper:

```
iroha app zk register-asset --asset rose#wonderland \
  --allow-shield true --allow-unshield true \
  --vk-transfer halo2/ipa:vk_transfer --vk-unshield halo2/ipa:vk_unshield
```

Once registered and bound, you can run the shield/unshield examples above. For unshield, ensure your `--proof-json` refers to the matching backend and VK id.

### End-to-end demo script

A tiny end-to-end helper script is available (Bash and PowerShell):

```
fuzz/attachments/zk/demo_shield_unshield.sh
fuzz/attachments/zk/demo_shield_unshield.ps1
```

It runs the following:
1) VK register (transfer + unshield)
2) `zk register-asset` (Hybrid, with VK bindings)
3) `zk shield` with a placeholder commitment
4) `zk unshield` (skipped by default; requires a real proof JSON)

Environment variables you can set before running:
- `CLI_CONFIG`: path to client config TOML
- `AUTHORITY`, `PRIVATE_KEY`: used for VK ops
- `BACKEND` (default `halo2/ipa`), `ASSET_ID` (default `rose#wonderland`)
- `FROM`, `TO`, `AMOUNT`, `NOTE_COMMITMENT_HEX`
- `RUN_UNSHIELD=1`, `PROOF_JSON=/path/to/proof.json` to attempt unshield

### Kotodama samples (non-consensus demos)

Sample Kotodama programs illustrating pointer constructors, logging, and host syscalls. These are not required for the shield/unshield flow above but serve as references.

```
fuzz/attachments/zk/kotodama/zk_shield_example.ko
fuzz/attachments/zk/kotodama/zk_unshield_verify_example.ko
```

Notes:
- Compile to `.to` using the Kotodama compiler (see docs) and deploy/run as a contract if desired.
- ZK verification in Kotodama is backend-gated; for practical flows use the CLI helpers above to submit ISIs and proof attachments.

```
```

List background prover reports and fetch one (non‑consensus):

```bash
iroha app zk prover reports list
iroha app zk prover reports get --id 0123ab...
```

Count prover reports matching filters (server‑side):

```bash
# Total count
iroha app zk prover reports count

# Only Norito reports with a specific ZK1 tag
iroha app zk prover reports count --content-type application/x-norito --has-tag IPAK

# Only successful reports since a timestamp (ms)
iroha app zk prover reports count --ok-only --since-ms 1725500000000
```

Server‑side bulk cleanup (dangerous; use filters + --yes):

```bash
# Dry‑run: show what would be deleted (client lists, server filters applied by CLI)
iroha app zk prover reports cleanup --failed-only --content-type application/x-norito

# Server‑side delete of matching reports (confirm with --yes)
iroha app zk prover reports cleanup --server --yes --failed-only --content-type application/x-norito --since-ms 1725500000000

# Delete within a time window [since_ms, before_ms]
iroha app zk prover reports cleanup --server --yes \
  --content-type application/x-norito \
  --since-ms 1725500000000 \
  --before-ms 1725600000000
```

Run the full sample sequence:

```bash
cd fuzz/attachments/zk
bash ./run.sh
```

## Examples

:grey_exclamation: All examples below are Unix-oriented. If you're working on Windows, we would highly encourage you to consider using WSL, as most documentation assumes a POSIX-like shell running on your system. Please be advised that the differences in the syntax may go beyond executing `iroha.exe` instead of `iroha`.

### Create new Domain

To create a domain, you need to specify the entity type first (`domain` in our case) and then the command (`register`) with a list of required parameters. For the `domain` entity, you only need to provide the `id` argument as a string that doesn't contain the `@`, `#` or `$` symbols.

```bash
iroha ledger domain register --id "Soramitsu"
```

### Create new Account

To create an account, specify the entity type (`account`) and the command (`register`). Then define the value of the `id` argument using an account alias in the `alias@domain` format, where the alias is the account's public key in multihash representation:

```bash
iroha ledger account register --id "ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu"
```

### Mint Asset to Account

To add assets to the account, you must first register an Asset Definition. Specify the `asset` entity and then use the `register` and `mint` commands respectively. Here is an example of adding Assets of the type `Quantity` to the account:

```bash
iroha ledger asset register --id "XOR#Soramitsu" --type Numeric
iroha ledger asset mint --id "XOR##ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu" --quantity 1010
```

With this, you created `XOR#Soramitsu`, an asset of type `Numeric`, and then gave `1010` units of this asset to the account `ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu`.

### Query Account Assets Quantity

You can use Query API to check that your instructions were applied and the _world_ is in the desired state. For example, to know how many units of a particular asset an account has, use `asset get` with the specified account and asset:

```bash
iroha ledger asset get --id "XOR##ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu"
```

This query returns the quantity of `XOR#Soramitsu` asset for the `ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu` account.

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
## Regenerating Markdown Help

Ensure the CLI builds, then run:

```
make docs-cli
# or
cargo run -p iroha_cli -- tools markdown-help > crates/iroha_cli/CommandLineHelp.md
```

This regenerates the full `CommandLineHelp.md` directly from the live CLI, keeping the docs in sync with the actual arguments and subcommands.
