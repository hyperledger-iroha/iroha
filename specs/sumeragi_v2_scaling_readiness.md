# Scaling genesis readiness contract

The scaling launcher probes each original validator through `iroha --machine
--config <original-client.toml> --output-format json bridge genesis-readiness
--challenge <64-lowercase-hex> --node-public-key <BLS-key> --genesis-hash <canonical-hash>
--context-id <canonical-hash> --request-timeout-ms <1..60000>`. The existing
`--config-fd` and `--config-source-path` inputs also support retained configuration
file descriptors. The command does not submit transactions, retry, bootstrap state,
select a context from returned evidence, or replace an original node process.

The original configuration, BLS role key, genesis hash and height-one context must
remain under the launcher's custody. Each attempt receives a fresh unpredictable
nonzero challenge. One absolute deadline bounds SDK compatibility waiting and all
HTTP requests together. The SDK rejects results completing after that deadline.
The launcher separately bounds child execution and CPU verification by its overall
deadline; a request timeout is not a CPU time limit. Existing deadlines attached to
the SDK context can only become shorter.

`GET /v1/bridge/finality/attestation/1` accepts only the current node-signed statement
for the exact durable genesis tip. The SDK retains canonical bounded Norito decoding,
exact challenge/node/network/genesis binding, identical height-one genesis/tip proofs,
node signature verification and full quorum/PoP verification under the independently
retained original context. An active reducer at height two is allowed while its
committed tip is genesis. Later committed tips fail the fresh-run condition.

Non-success responses use one canonical `ErrorEnvelope` with code
`bridge_finality_attestation_failure` and the sole detail
`finality_attestation_failure`. That `FinalityAttestationFailure` record carries the
request challenge, requested height, closed reason and nullable `tip_mismatch`.
In JSON, `reason` is one case-sensitive scalar string naming the variant in the
table, such as `"GenesisUncommitted"`; tagged objects and alternate spellings are
invalid. The SDK requests canonical Norito, whose reason uses the closed enum.
These are **unsigned failure observations**, not identity or readiness evidence.
They carry no success variant.
The client rejects a wrong challenge, height, reason/status combination, malformed
or noncanonical encoding, duplicate or wrong media type, and a body over 2 KiB.
The outer HTTP response cap remains 16 MiB for the successful proof envelope.
Generic ingress errors, bare 404/503 responses and transport errors fail the probe.

| Reason | HTTP | CLI state |
| --- | --- | --- |
| `ConsensusUninitialized` | 503 | `pending` |
| `GenesisUncommitted` | 503 | `pending` |
| `RestartRequired` | 503 | `restart_required` |
| `TipChanged` | 409 | `conflict` |
| `ConflictingState` | 409 | `conflict` |
| `FinalityUnavailable` | 503 | `unavailable` |
| `InternalFailure` | 500 | `unavailable` |

Restart-required takes precedence even when no initial status exists. Invalid
status is conflict. Empty ledger state is `GenesisUncommitted` only when the
structurally valid runtime status also reports zero committed height; an empty
ledger with an existing committed frontier is conflict. Missing finality evidence
is unavailable. Only the two explicit pending states permit a retry under the
retained deadline. Every endpoint response remains `Cache-Control: no-store` and
varies by challenge and response format.

The command emits one compact JSON object plus a newline, at most 24 MiB total:
`version` (1), `state`, nullable `reason` (lower-snake-case reason), `challenge`,
`node_id`, `network_id`, `genesis_hash`, `context_id`, and nullable
`attestation_norito_base64`. Identity fields echo independent inputs for all states.
Only `ready` includes the canonical standard-base64 attestation and has null reason;
all failure observations omit evidence by emitting null. Invalid input, transport,
unknown status, decoding or cryptographic failure exits unsuccessfully without a
readiness report. The launcher must retain each ready receipt with the original
process/configuration identities and cannot infer peer-local transaction application
or runtime-provider readiness from this genesis statement.

Qualification requires native shared DTO, Torii, SDK and CLI tests followed by actual
four-validator launch/readiness coverage. Source contracts and prepared tests alone
do not qualify this path or close multilane release gates.
