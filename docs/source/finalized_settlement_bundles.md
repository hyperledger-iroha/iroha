# Offline finalized settlement carriers

`iroha --machine app execution verify-settlement --bundle PATH` accepts the original one-file version 1 bundle and the additive version 2 index described here. Both require the same externally selected `--network-id`, `--trusted-context-id`, `--expected-entry-hash`, `--session-id`, `--profile-id` and `--outcome-hash`. No downloaded file supplies or replaces those expectations.

Version 2 makes the 256-height limit a **per-batch bound**, not the maximum distance from the original context pin. One `BridgeFinalityVerifier` persists through every prefix and the final settlement bundle. Every next proof must be the immediate authenticated successor, including epoch changes at a file boundary. A filename, digest, height or roster supplied by an endpoint never creates a new trust anchor.

## Directory format

The index has exactly three fields:

```json
{
  "version": 2,
  "finality_batches": [
    {
      "file": "finality-000001.json",
      "sha256": "<64 lowercase hex characters>",
      "bytes": 12345
    }
  ],
  "settlement_bundle": {
    "file": "settlement-finality.json",
    "sha256": "<64 lowercase hex characters>",
    "bytes": 23456
  }
}
```

The example lengths and digests are placeholders. `bytes` must equal the actual positive UTF-8 JSON file length. SHA-256 covers those exact bytes, including whitespace. These digests detect file substitution or incomplete export; they are not signatures and do not establish trust. The ordered prefix array may be empty. All referenced names must be unique, flat ASCII basenames: a nonempty stem containing only letters, digits, `-` or `_`, followed by `.json`, at most 128 bytes total. Leading zeroes in names are allowed. Files are adjacent to the index; paths, URLs, directories, symlinks and special files are rejected. On Unix, opens use `NOFOLLOW | NONBLOCK`; on Windows, they use `OPEN_REPARSE_POINT` with regular-file checks.

Each prefix file contains exactly:

```json
{"version":1,"finality_chain_base64":["<canonical Norito bridge-finality carrier in canonical base64>"]}
```

The final file is the unchanged version 1 settlement bundle:

```json
{
  "version": 1,
  "finality_chain_base64": ["<one or more consecutive finality carriers>"],
  "executed_block_wire_base64": "<canonical executed SignedBlockWire>",
  "block_proofs_base64": "<canonical BlockProofs>"
}
```

Neither format permits extra context or trust fields. The last carrier must authenticate the supplied executed block. Native verification then checks the exact wallet-signed entry hash and successful result, exactly one explicit `SettleGameSessionV1` for the selected session, all independently selected proof/outcome bindings, and the compiled native execution proof. An authenticated but unrelated transaction still fails. Finality and mathematical verification do not change the profile's separate qualification flag.

## Memory and work admission

Each prefix and final bundle retains the existing limits: at most 96 MiB JSON, 64 MiB aggregate decoded carriers, 256 heights, 9 MiB per finality carrier, 32 MiB executed block and 16 MiB entry/result proofs. A collector must flush a prefix when either its height or byte bound is reached. Keeping the final bundle to one height reserves enough room for its block and proofs under the aggregate limit.

The index is at most 1 MiB with at most 4,096 prefix references. Only one prefix's JSON and decoded carriers are processed at a time; the verifier retains the latest authenticated proof, not the full history. Exact file length and hash are checked before decoding its carriers. The standard bounded canonical Norito decoder and JSON preflight remain in use.

Total work is separately admitted by explicit local CLI options:

| Option | Default | Hard maximum |
| --- | ---: | ---: |
| `--max-finality-heights` | 65,536 heights | 1,048,576 heights |
| `--max-finality-archive-bytes` | 4 GiB | 64 GiB |

Both must be positive. The byte budget includes the final block and entry/result proofs. The sum of indexed JSON lengths must also fit twice the selected decoded-byte budget plus 1 MiB of framing allowance. Verification checks cumulative heights and decoded bytes as it progresses. Increasing a work budget does not alter the original trust pin. Exceeding a hard limit fails explicitly; there is no automatic endpoint checkpoint or skipped-history fallback.

Successful JSON verdicts additionally report `finality_heights_verified` and `archive_bytes_verified`. These counters describe the actual verification work. A successful download or index export remains unverified transport data.

## Qualification status

The native test source includes a real locally signed 257-height BLS/PoP chain split at 256, the same indexed-file path through final executed-block and Merkle authentication, an epoch transition exactly at a file boundary, missing/duplicate/reordered heights, foreign network/context pins, work-budget failures, extra trust fields, file substitution, missing files and symlink rejection. The long-chain fixture uses unrelated transactions to isolate finality and must fail at settlement identification after authenticating the complete chain; it does not claim a funded game occurred. Existing native tests separately reject invalid execution proofs, and the ignored end-to-end test generates a real execution proof.

Run `cargo iroha-fast -- test -p iroha_cli --bin iroha execution_finality::tests -- --nocapture` in the assigned warm Cargo lane. Source and syntax checks are not runtime qualification. Normal CLI execution tests, actual four-validator settlement/inclusion, wallet deployment and cryptographic release review remain required before activation.
