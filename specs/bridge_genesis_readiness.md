# Genesis readiness probe

`iroha bridge genesis-readiness` asks one original validator, once, for a
node-signed statement that its durable tip is the signed genesis, and verifies
that statement against independently retained genesis inputs. It is the
readiness check for a freshly launched network. The command does not submit
transactions, retry, bootstrap state, select a trust root from returned
evidence, or replace a node process.

## Invocation

```text
iroha --machine --config <client.toml> --output-format json bridge genesis-readiness \
  --challenge <64 lowercase hexadecimal characters> \
  --node-public-key <BLS public key of the validator> \
  --genesis-hash <canonical genesis hash> \
  --signed-genesis <signed genesis file> \
  --genesis-manifest <genesis manifest JSON> \
  --genesis-public-key <genesis signing public key> \
  --request-timeout-ms <1..=60000>
```

`--config-fd` with `--config-source-path` may replace `--config` to pass an
inherited configuration descriptor. The command requires
`--output-format json`.

The caller keeps the client configuration, the validator's BLS key, the
genesis hash, the signed genesis, its manifest and the genesis public key under
its own custody; none of them comes from the node. Every attempt uses a fresh,
unpredictable, nonzero challenge.

## Verification

Before sending a request the command validates the signed genesis against its
manifest, the genesis public key and the genesis hash
(`iroha_genesis::validate_prepared_genesis_bundle`), requires the client's
`NetworkId` to be the one derived from the genesis hash, and builds a
`SumeragiFinalityVerifier` from the signed genesis and the validators it
registers ([bridge finality](bridge_finality.md)).

`Client::poll_sumeragi_genesis_readiness` then reads
`GET /v1/bridge/finality/attestation/1` as canonical Norito with the challenge.
It accepts a `SumeragiFinalityAttestation` only if:

- the attestation body is consistent and its BLS signature verifies under the
  expected node key (`SumeragiFinalityAttestation::verify`);
- the body binds the exact challenge, node, network and the verifier's
  consensus instance, and its tip is height 1;
- the verifier admits the embedded genesis proof, and the tip proof carries the
  same decision (`verify_same_decision`).

A consensus driver at height 2 is accepted while its committed tip is still
genesis; a later committed tip is reported as `TipChanged`.

One absolute deadline (`--request-timeout-ms`) bounds SDK compatibility waiting
and every HTTP request together. A shorter deadline already set on the client
stays in force, and a result completing after the deadline is rejected. The
timeout does not bound CPU time; callers bound the process themselves.

## Failure observations

Non-success responses are the unsigned failure observations of
[`torii/api_contract.md`](torii/api_contract.md) (code
`bridge_finality_attestation_failure`). They are not identity or readiness
evidence. The client rejects a wrong challenge or height, a reason that does
not match its HTTP status, malformed or noncanonical encoding, a wrong media
type and a failure body over 2 KiB. Generic ingress errors, bare 404 or 503
responses and transport errors fail the probe. The command reports each reason
as a state:

| Reason | HTTP | `state` |
| --- | --- | --- |
| `ConsensusUninitialized` | 503 | `pending` |
| `GenesisUncommitted` | 503 | `pending` |
| `RestartRequired` | 503 | `restart_required` |
| `TipChanged` | 409 | `conflict` |
| `ConflictingState` | 409 | `conflict` |
| `FinalityUnavailable` | 503 | `unavailable` |
| `InternalFailure` | 500 | `unavailable` |

Only `pending` permits another attempt, within the caller's deadline.

## Report

For a ready node or an accepted failure observation the command prints one
compact JSON object and a newline, less than 24 MiB in total:

| Field | Content |
| --- | --- |
| `version` | `1` |
| `state` | `ready`, `pending`, `restart_required`, `conflict` or `unavailable` |
| `reason` | lower-snake-case reason; `null` when ready |
| `challenge` | the request challenge |
| `node_id` | the expected validator |
| `network_id` | the network derived from the genesis hash |
| `genesis_hash` | the retained genesis hash |
| `consensus_instance` | hexadecimal id of the global consensus instance, derived from the genesis hash and chain id |
| `genesis_execution_hash` | hexadecimal execution result `R` of genesis from the attested proof; `null` unless ready |
| `attestation_norito_base64` | the canonical attestation (at most 16 MiB) in standard base64; `null` unless ready |

Invalid input, transport failure, an unknown status, and decoding or
cryptographic failures exit unsuccessfully without a report. A ready report
authenticates one node's observed genesis execution. Quorum readiness needs
ready reports from independently selected validators, and a genesis statement
never shows that a peer applied later transactions or that its runtime
providers are ready.
