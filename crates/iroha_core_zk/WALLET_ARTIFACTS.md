# Wallet artifact producer

`kagemusha_wallet_artifacts` is an offline packaging command around the maintained
`OfflineCompilerV1::wallet_pack`. It reconstructs the entire ordinary-finality
verifier graph, compiles the full current wallet route catalog, and uses the
protocol's existing root-certificate and artifact-manifest signing methods. It
does not install a wallet, register a scheme, submit a transaction or admit a
release. Those owners independently select and authenticate these originals.

Build the production binary without `test-utils`:

```sh
cargo iroha-fast -- build --locked --offline -p iroha_core_zk --bin kagemusha_wallet_artifacts
kagemusha_wallet_artifacts --request /absolute/private/inputs/request.json
```

The closed JSON request has these exact fields:

| Field | Original selected by the operator |
| --- | --- |
| `schema` | `iroha.kagemusha.wallet-artifact-production.v1` |
| `chain_id`, `network_id_hex`, `genesis_public_key` | Actual current signed genesis and Ed25519 genesis authority; the network is derived from its header. |
| `signed_genesis` | `{ "path": "/absolute/original", "sha256": "64 lowercase hex digits" }` |
| `finality_inventory` | Same path/hash shape, containing the complete canonical `Vec<ArtifactRecord>` from the genuine finality producer. |
| `finality_originals_directory` | Private content-addressed directory of descriptor/VK originals. Server proving tables are never opened. |
| `custody_directory` | Existing private recovery directory containing `scheme-root.pkcs8.der` and `artifact.pkcs8.der`. No key is generated or replaced. |
| `scheme_root_public_key_hex`, `enrollment_public_key_hex`, `artifact_public_key_hex` | Independently selected, distinct uncompressed P-256 public points. |
| `output_parent`, `output_name` | Existing private parent and a fresh child name; existing output is refused. |
| `maximum_total_bytes` | Positive wallet-original storage ceiling, at most 128 GiB; this is not a process RSS limit. |

Every input path is absolute and canonical, without symlinks. Input request,
genesis and inventory are retained immutable `0400` single-link files in private
directories. Private key reads use the native `iroha_fs` custody owner and
zeroized buffers. Public input files remain retained and are rechecked before
and after signing. Existing or partial output is preserved on failure.

The finality graph has a 4,096-record cap and 512 MiB aggregate descriptor/VK
ceiling. Each descriptor remains bounded to 1 MiB and each VK to 256 KiB. The
wallet compiler uses the maintained 65,536-row source geometry, on-demand coset
tables and the separate default 64 MiB MSM scratch budget. Full route construction
is expensive; source preparation or a successful Rust build does not prove that
the catalog has been compiled or that physical devices meet performance bounds.

Only after the real compiler returns its private draft does this command issue
Enrollment and Artifact certificates with serial 1 for this first release. The
artifact signer signs the exact compiler-derived manifest body. Protocol helpers
normalize and authenticate every P-256 signature. The completed draft's `finish`
method authenticates the exact certificate and manifest. Only its completed
original lists are copied through bounded verified streams into a fresh `carrier`.
The command reloads the signed verifier pack, authenticates the producer inventory
and genuinely qualifies the entire wallet source graph against the independently
selected native finality verifier before exporting carrier metadata.
There is no caller-selected relation ID, artifact hash, readiness flag or retired
schema path.

The output root retains Scheme, Enrollment certificate, Artifact certificate and
manifest originals, exact public request/genesis/finality inventory, and the
compiler caches. `carrier/` contains the complete verifier pack, producer
inventory, transport and financial-original metadata, and exact closed
`wallet-originals` and `finality-originals` directories. Provisional compiler
originals remain in `compiler-cache`; they are excluded from the carrier. The
carrier is the directory consumed by deployment retention. Authenticated
application runtime, genuine installed startup and financial execution remain
separate validation requirements.

## Ordinary-finality source compilation

`--compile-finality /absolute/private/inputs/request.json` executes the genuine
`iroha_kagemusha_proof::finality::catalog::compile` with `StreamingCatalog`. Its
separate closed request contains `schema` equal to
`iroha.kagemusha.finality-artifact-production.v1`, the same `chain_id`,
`network_id_hex`, `genesis_public_key`, `signed_genesis`, `output_parent` and
`output_name` fields described above, plus:

- `source_revision` and `source_manifest_sha256`: the actual selected source
  capture. This retained provenance is data; release owners still verify its
  signed original capture and actual executable identity independently.
- `maximum_original_bytes`: the logical full descriptor/VK/PK inventory ceiling,
  at least 256 MiB and at most 512 GiB.
- `working_proving_key_bytes`: retained regenerable PK working-set ceiling,
  at least 256 MiB and at most 16 GiB; 512 MiB is the existing source driver's
  bounded working set. Neither value measures or bounds process RSS.

The command derives the current anchor from the selected signed genesis and
compiles the complete fixed source topology. It never selects a retired test
fixture, substitutes an unproved descriptor, or issues a finality proof. The
maintained compiler reconstructs and strictly imports its output graph before
returning. Its bounded regenerable PK cache is `server-compiler-cache`. Each
actual generated descriptor, VK and PK is also sealed in the separate immutable,
content-addressed `server-originals` archive under the same finite complete-graph
inventory limit. The source catalog creates private directories and files with
explicit permissions; it does not change the process umask.

Before completion, a fresh import reads every graph member exclusively from the
server archive through the serving reader, requires that every selected record
was consumed, and compares the genuine terminal source identity to the compiler's
result. The archive is mandatory server material; it is never regenerated while
serving a proof. The completed canonical inventory retains all three roles' hash
and length commitments, while `finality-originals` copies only descriptor/VK
originals. That verifier directory and `finality-inventory.norito` feed `--request`
directly. No server PK enters wallet packaging. Incomplete output is retained but
has no completion record or wallet authority. Compilation must be executed for
the actual fresh genesis; existing engineering graph outputs are not silently
reused as production inputs.

## Terminal Load proof serving boundary

The optional `torii.kagemusha_load_finality` configuration selects exact nonzero
Scheme and signed manifest digests, the signed verifier pack, authenticated
producer inventory, complete `server-originals` archive and an existing private
immutable journal. These identities and paths are mandatory when the service is
configured. Finite configurable bounds with shared defaults cover keys, total
original bytes, artifact count, MSM scratch, journal files/bytes, worker queue, native allocation,
per-height native deadline and maximum receipt height. Absence supplies no
replacement finality provider. The runtime independently authenticates this
installation against Core's actual configured signed genesis and verifies the
complete graph before proving.

`GET /v1/kagemusha/{scheme}/wallets/{wallet}/loads/{request}/finality-proof`
requires canonical payer authentication for the selected network, an empty body
and `Accept: application/x-norito`. The worker independently reacquires the payer's
committed receipt and native event; its original native cursor, installed proof
graph and durable checkpoints produce the terminal LoadFinality frame (at most
16,384 bytes). Responses are private/no-store. Pending, missing or refused proof
material returns HTTP503 `kagemusha_load_finality_unavailable`, with no block-proof
or decoded-receipt substitute. Completed queue entries may be evicted; retries
reacquire and verify the exact retained proof. Pending work is never evicted.

Source and custody/component tests do not establish a successful full signed
installation or an actual financial proof response. Those executions and phone
qualification remain open. Proof work observes cancellation; complete graph
mounting does not currently provide mid-import cancellation.

## Shared packaging ownership

This command composes the maintained CoreZK compiler, signed draft completion,
source qualification and transport APIs. Its reusable operator duties are genuine
ordinary-finality preparation and selected P-256 recovery-custody signing. An
external-signature facade may reuse the same draft and carrier format when its
prepare/sign/finish path is complete; it must not recreate proof semantics or
replace the signed completion and source qualification stages.
