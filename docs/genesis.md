# Genesis configuration

A `genesis.json` file defines the first transactions that run when an Iroha network starts. The file is a JSON object with these fields:

- `chain` – the deployment-unique chain identifier. It is exact,
  case-sensitive ASCII, is at most 128 bytes, begins and ends with an
  alphanumeric character, and otherwise permits only alphanumerics plus
  `.`, `_`, `:`, and `-`.
- `executor` (optional) – path to the executor bytecode (`.to`). If present,
  genesis includes an Upgrade instruction as the first transaction. If omitted,
  no upgrade is performed and the built‑in executor is used.
- `ivm_dir` – directory containing IVM bytecode libraries. Defaults to `"."` if omitted.
- `consensus_mode` – consensus mode advertised in the manifest. Required; use `"Npos"` for the public Sora Nexus dataspace, or `"Permissioned"`/`"Npos"` for other Iroha3 dataspaces. Iroha2 defaults to `"Permissioned"`.
- `transactions` – list of genesis transactions executed sequentially. Every entry may contain:
  - `parameters` – initial network parameters.
  - `instructions` – structured Norito instructions (e.g., `{ "Register": { "Domain": { "id": "wonderland" }}}`). Raw byte arrays are not accepted, and `SetParameter` instructions are rejected here—seed parameters via the `parameters` block and let normalization/signing inject the instructions.
  - `ivm_triggers` – triggers with IVM bytecode executables.
  - `topology` – initial peer topology. Each entry keeps the peer id and PoP together: `{ "peer": "<public_key>", "pop_hex": "<hex>" }`. `pop_hex` may be omitted while composing, but must be present before signing.
- `crypto` – cryptography snapshot mirrored from `iroha_config.crypto` (`default_hash`, `allowed_signing`, `allowed_curve_ids`, `sm2_distid_default`, `sm_openssl_preview`). `allowed_curve_ids` mirrors `crypto.curves.allowed_curve_ids` so manifests can advertise which controller curves the cluster accepts. Tooling enforces SM combinations: manifests that list `sm2` must also switch the hash to `sm3-256`, while builds compiled without the `sm` feature reject `sm2` entirely. Normalization injects a `crypto_manifest_meta` custom parameter into the signed genesis; nodes refuse to start if the injected payload disagrees with the advertised snapshot.

The signed genesis block is the deployment trust anchor, not the chain label
by itself. Genesis has one authentication boundary: exactly one block signature
from the configured, single-key genesis authority. That signature covers the
consensus header, including height, previous-block hash, and the Merkle root of
the ordered transaction intents. Admission recomputes that root from the
external entrypoints and checks every intent's exact `chain`, authority, and
instruction-only executable. It also requires height 1 with no previous block,
and the state-dependent pipeline accepts that height only against an empty
chain.

The authorization-proof bytes carried by individual genesis transaction
containers are not separate trust anchors and are deliberately not reverified.
The block signature authenticates the execution-affecting transaction intent,
so requiring an additional valid signature over every transaction would add a
redundant second proof by the same authority without protecting more state.
Normal, non-genesis transactions continue to require their own authorization
signatures.

Peer bootstrap additionally checks the requested chain and public key, and
requires `genesis.expected_hash` to pin the exact consensus-header hash on every
remote fetch. A local signed genesis file is itself the configured artifact and
does not require a separate hash pin. Operators must therefore assign a
different chain identifier to each distinct deployment; intentionally reusing
both a chain identifier and its signing authority declares the same transaction
replay domain.

Genesis is also the chain's authorization root. It may create accounts, assign
roles and genesis-only administrative permissions, register active verifier
keys and confidential-asset policy, seed governance state, and select the
executor that will govern later blocks. Those effects intentionally survive
genesis: preventing a correctly signed genesis from choosing them would only
move the same trust decision into node-local code. Review the normalized
manifest and its executor bytecode before signing, distribute the signed block
or its exact hash out of band, and treat a maliciously signed genesis as a
different chain—not as an untrusted transaction that runtime permissions can
repair.

The built-in initial executor still applies structural validation during
genesis. References must resolve, verifier records must be active where
required, and every instruction must execute successfully. After genesis,
`CanUpgradeExecutor` and the other genesis-only administrative permissions
cannot be newly delegated by resource ownership: the genesis-only-name check
runs before capability-root resolution, while executor upgrade requires an
exact permission already established by genesis or the active executor's own
governance policy.

Example (`kagami genesis generate default --consensus-mode npos` output, instructions trimmed):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [
        { "Register": { "Domain": { "id": "wonderland" } } }
      ],
      "ivm_triggers": [],
      "topology": [
        {
          "peer": "ed25519:...",
          "pop_hex": "ab12cd..."
        }
      ]
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519", "secp256k1"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### Seed the `crypto` block for SM2/SM3

Use the xtask helper to produce the key inventory and ready-to-paste configuration snippet in one step:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` now contains:

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "secp256k1", "sm2"]  # remove "sm2" to stay in verify-only mode
allowed_curve_ids = [1]               # add new curve ids (e.g., 15 for SM2) when controllers are allowed
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```

Copy the `public_key`/`private_key` values into the account/client configuration and update the `crypto` block of `genesis.json` so it matches the snippet (for example, set `default_hash` to `sm3-256`, add `"sm2"` to `allowed_signing`, and include the right `allowed_curve_ids`). Kagami will refuse manifests where the hash/curve settings and signing list are inconsistent.

> **Tip:** Stream the snippet to stdout with `--snippet-out -` when you just want to inspect the output. Use `--json-out -` to emit the key inventory on stdout as well.

If you prefer to drive the lower-level CLI commands manually, the equivalent flow is:

```bash
# 1. Produce deterministic key material (writes JSON to disk)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hydrate the snippet that can be pasted into client/config files
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **Tip:** `jq` is used above to save a manual copy/paste step. If it is not available, open `sm2-key.json`, copy the `private_key_hex` field, and pass it to `crypto sm2 export` directly.

> **Migration guide:** When converting an existing network to SM2/SM3/SM4, follow
> [`specs/crypto/sm_config_migration.md`](../specs/crypto/sm_config_migration.md)
> for the layered `iroha_config` overrides, manifest regeneration, and rollback
> planning.

## Generate and validate

1. Generate a template:
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
   ```
`--consensus-mode` controls which consensus parameters Kagami seeds into the `parameters` block. The public Sora Nexus dataspace requires `npos` and does not support staged cutovers; other Iroha3 dataspaces may use permissioned or NPoS. Iroha2 defaults to `permissioned` and may stage `npos` via `--next-consensus-mode`/`--mode-activation-height`. When `npos` is selected, Kagami seeds the `sumeragi_npos_parameters` payload that drives NPoS collector fan-out, election policy, and reconfiguration windows; normalization/signing turns these into `SetParameter` instructions in the signed block.
2. Optionally edit `genesis.json`, then validate and sign it:
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   To emit SM2/SM3/SM4-ready manifests, pass `--default-hash sm3-256` and include `--allowed-signing sm2` (repeat `--allowed-signing` for additional algorithms). Use `--sm2-distid-default <ID>` if you need to override the default distinguishing identifier.

   When you start `irohad` with only `--genesis-manifest-json` (no signed genesis block), the node now seeds its runtime crypto configuration from the manifest automatically; if you also supply a genesis block, the manifest and config still must match exactly.

- Validation notes:
  - Kagami injects `consensus_handshake_meta`, `confidential_registry_root`, and `crypto_manifest_meta` as `SetParameter` instructions in the normalized/signed block. `irohad` will recompute the consensus fingerprint from those payloads and fail startup if the handshake metadata or crypto snapshot disagree with the encoded parameters. Keep these out of `instructions` in the manifest; they are generated automatically.
- Inspect the normalized block:
  - Run `kagami genesis normalize genesis.json --format text` to see the final ordered transactions (including injected metadata) without providing a keypair.
  - Use `--format json` to dump a structured view suitable for diffing or reviews.

`kagami genesis sign` checks that the JSON is valid and produces a Norito‑encoded block ready to use via `genesis.file` in the node configuration. The resulting `genesis.signed.nrt` is already in canonical wire form: a version byte followed by a Norito header describing the payload layout. Always distribute this framed output. Prefer the `.nrt` suffix for signed payloads; if you don't need to upgrade the executor at genesis, you can omit the `executor` field and skip providing a `.to` file.

When signing NPoS manifests (`--consensus-mode npos` or Iroha2-only staged cutovers), `kagami genesis sign` requires the `sumeragi_npos_parameters` payload; generate it with `kagami genesis generate --consensus-mode npos` or add the parameter manually.
By default, `kagami genesis sign` uses the manifest's `consensus_mode`; pass `--consensus-mode` to override it.

## What Genesis Can Do

Genesis supports the following operations. Kagami assembles them into transactions in a well‑defined order so peers deterministically execute the same sequence.

- Parameters: Set initial values for Sumeragi (block/commit times, drift), Block (max txs), Transaction (max instructions, bytecode size), Executor and Smart Contract limits (fuel, memory, depth), and custom parameters. Kagami seeds `Sumeragi::NextMode` and the `sumeragi_npos_parameters` payload (NPoS election, reconfig) via the `parameters` block so startup can apply consensus knobs from on-chain state; the signed block carries the generated `SetParameter` instructions.
- Native Instructions: Register/Unregister Domain, Account, Asset Definition; Mint/Burn/Transfer assets; Transfer domain and asset definition ownership; Modify metadata; Grant permissions and roles.
- IVM Triggers: Register triggers that execute IVM bytecode (see `ivm_triggers`). Triggers’ executables resolve relative to `ivm_dir`.
- Topology: Provide the initial set of peers via the `topology` array inside any transaction (commonly the first or last one). Each entry is `{ "peer": "<public_key>", "pop_hex": "<hex>" }`; `pop_hex` may be omitted while composing but must be present before signing.
- Executor Upgrade (optional): If `executor` is present, genesis inserts a single Upgrade instruction as the first transaction; otherwise, genesis starts directly with parameters/instructions.

### Transaction Ordering

Conceptually, genesis transactions are processed in this order:

1) (Optional) Executor Upgrade
2) For each transaction in `transactions`:
   - Parameter updates
   - Native instructions
   - IVM trigger registrations
   - Topology entries

Kagami and the node code ensure this ordering so that, for example, parameters apply before subsequent instructions in the same transaction.

## Recommended Workflow

- Start from a template with Kagami:
  - Built‑in ISI only: `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Sora Nexus public dataspace; use `--consensus-mode permissioned` for Iroha2 or private Iroha3).
  - With custom executor upgrade (optional): add `--executor <path/to/executor.to>`
  - Iroha2-only: to stage a future cutover to NPoS, pass `--next-consensus-mode npos --mode-activation-height <HEIGHT>` (keep `--consensus-mode permissioned` for the current mode).
- `<PK>` is any multihash recognised by `iroha_crypto::Algorithm`, including the TC26 GOST variants when Kagami is built with `--features gost` (for example `gost3410-2012-256-paramset-a:...`).
- Validate while editing: `kagami genesis validate genesis.json`
- Sign for deployment: `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- Configure peers: set `genesis.file` to the signed Norito file (e.g., `genesis.signed.nrt`) and `genesis.public_key` to the same `<PK>` used for signing.

Notes:
- Kagami’s “default” template registers a sample domain and accounts, mints a few assets, and grants minimal permissions using only built‑in ISIs – no `.to` required.
- If you do include an executor upgrade, it must be the first transaction. Kagami enforces this when generating/signing.
- The executor must implement the migration entrypoint and return a canonical
  Norito migration result. Production nodes always run that guest entrypoint;
  they do not infer migration behavior from bytecode size or IVM header fields.
- Use `kagami genesis validate` to catch invalid `Name` values (e.g., whitespace) and malformed instructions before signing.

## Running with Docker/Swarm

The provided Docker Compose and Swarm tooling handle both cases:

- Without executor: the compose command strips a missing/empty `executor` field and signs the file.
- With executor: it resolves the relative executor path to an absolute path inside the container and signs the file.

This keeps development simple on machines without prebuilt IVM samples while still allowing executor upgrades when needed.
