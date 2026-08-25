# Startup Trust Roots

Iroha resolves a local trust root before it accepts a genesis block body or
replays persisted chain state. Peers do not distribute genesis, and the P2P
protocol has no genesis request or response messages.

## Normal startup

Normal startup is bound to both:

- the configured `genesis.public_key`; and
- one exact signed-genesis consensus-header hash.

Exactly one of inline `genesis.expected_hash` or canonical one-line
`genesis.expected_hash_file` is mandatory and supplies the exact hash independently
of any block body. Production templates use `/run/iroha/genesis.expected_hash`,
the same public file selected by client `network_id_file`. A locally provisioned signed `genesis.file`, when present,
must agree with it. Configuration normalization rejects a missing or malformed
hash before startup reads a genesis body from the artifact or Kura. This
ordering means an on-disk or operator-provisioned block can satisfy a trust root
that the operator already selected, but cannot select its own trust root.

The signed genesis block must contain exactly one block signature, at index
zero. Iroha verifies that signature with `genesis.public_key`, verifies that
the first transaction authority embeds the same key, and requires the block's
consensus-header hash to equal the resolved exact hash. The header commits the
ordered transaction intents through their Merkle root. Each embedded genesis
transaction must also carry a valid authorization proof for that same genesis
account; the block signature is not a substitute for the transaction proof.
Resultless consensus proposals authenticate the genesis header, signature,
chain, authority, executable shape, and sidecar commitments before any
bootstrap instruction executes. Deterministic execution results and their
Merkle root are checked after execution.

Genesis-only executor authority exists only while the candidate header has
height one **and** committed block history is empty. The Initial executor still
uses a closed, explicit instruction surface in that context; a genesis-shaped
header replayed over existing state receives no bootstrap exception. A signed
and exactly pinned genesis may seed governance permissions and install the
chain's initial executor because it is the chain's reviewed state root, not an
ordinary transaction path.

A fresh node therefore needs the complete signed `genesis.file`. A restarting
node may omit that file when the matching signed genesis body is already
present in Kura. `genesis.expected_hash` remains required in both cases. Empty
storage with only an expected hash fails because Iroha never fetches the
missing body from another peer.

The daemon and production genesis tooling treat every local trust-root body as
untrusted input before parsing. The signed genesis artifact is limited to 64
MiB and decoded under explicit sequence, aggregate-allocation, and nesting
budgets; the optional JSON genesis
manifest is limited to 16 MiB and preflighted before tree construction at
262,144 lexical values/keys/containers, 1 MiB per string, and depth 64. A
`--config-blake3` configuration is a single
flattened TOML source and therefore has the same 1 MiB source ceiling as the
ordinary configuration loader. All three paths use a bounded max-plus-one read
from a stable direct regular file and reject final-component symlinks/reparse
points, size changes, and inode substitution. Producers must split oversized
application setup into post-genesis governance or transaction flows instead of
raising these first-release startup memory ceilings. Each compiled IVM program
referenced by the source manifest is likewise read through the stable bounded
reader and may not exceed the V1 transaction bytecode ceiling (4 MiB). The
executor and all IVM trigger programs in one expanded manifest are additionally
limited to 64 MiB in aggregate, so a manifest cannot multiply the per-program
allowance into unbounded retained bytecode.

Kagami applies the same limits when signing, validating, embedding PoPs,
preparing Docker bundles, and round-tripping generated localnets. Its normalized
JSON view is emitted incrementally per instruction so expanded diagnostics do
not require a second complete JSON tree or simultaneous compact and pretty
renderings in memory.

Generated Docker swarms are validator-only. Signing happens before deployment;
the genesis private key, source manifest, executor inputs, and client
credentials are never mounted into a runtime service. Normal `kagami docker`
generation consumes one authoritative prepared bundle: it decodes and verifies
the signed body, then requires every `peerN.toml` chain, verifier key, exact
hash, validator identity, trusted roster, and PoP map to agree exactly with the
signed `RegisterPeerWithPop` roster. Compose reuses those validator identities
and embeds relative read-only paths for all three public genesis artifacts, so
generation cannot silently substitute a new roster. The public verifier key and
exact hash are separate Compose secrets; the signed body is a read-only bind.
The launcher exports `GENESIS`, `GENESIS_PUBLIC_KEY`, and
`GENESIS_EXPECTED_HASH` before configuration normalization. It validates
non-empty inputs and the canonical one-line marked-hash shape; Iroha repeats the
body, signature, verifier-key, and exact-hash checks.

An explicit non-empty `kagami docker --seed` selects deterministic development
mode for relocatable sample manifests. Only that mode generates validator
identities and uses `IROHA_GENESIS_*_FILE` placeholders. It never generates
random identities implicitly, and the operator must prepare its artifacts for
the exact seeded roster.

## Audited snapshot startup

An audited provisional snapshot is a separate, explicit trust root. Set
`snapshot.bootstrap.enabled = true` together with the exact lowercase
`snapshot.bootstrap.audited_sha256` and non-zero
`snapshot.bootstrap.audited_height`. This mode intentionally does not read or
derive authority from `genesis.file` while the imported prefix is provisional.

Deferring the genesis trust root is permitted only when Kura reports that
specific provisional imported-prefix state. Snapshot authentication must bind
the configured chain, exact payload digest, terminal height, block lineage,
and restored state boundary. Iroha finalizes deferred Kura recovery only after
authentication, recomputes the durable replay plan, and reauthenticates the
snapshot boundary before replay. Any missing, mismatched, or still-provisional
state aborts startup; it does not fall back to normal genesis or peer data.

The audited snapshot path is for a deliberately reviewed hard-fork boundary,
not routine node enrollment. See `specs/sumeragi_v2.md` for the snapshot
artifact and replay invariants.

## Operator checklist

1. Distribute the same signed genesis artifact and configured public key to
   every node before first start.
2. Record its exact consensus-header hash through exactly one of
   `genesis.expected_hash` or `genesis.expected_hash_file` on every node. Inline
   typed `genesis.expected_hash` uses tagged `hash:UPPER#CRC` Norito text; the
   public identity file uses raw lowercase marked hash text. Kagami prints the
   raw value after signing. `--expected-hash-out <name>.expected_hash` writes the
   canonical one-line value and atomically publishes `<name>.identity.toml`,
   which binds the same exact hash as raw client `network_id` and tagged
   validator `genesis.expected_hash` in one artifact. Production validators and
   clients consume that same one-line file through `expected_hash_file` and
   `network_id_file`; deployment renderers consume the paired artifact instead
   of assembling the two security domains independently. Generated localnets
   also decode the signed body back and check the same hash bytes in every peer
   config. Use normal seedless
   `kagami docker` generation so the same prepared validator bundle and all
   three operator-approved files are validated before Compose is written.
3. Verify the external artifact and configured hash are identical before first
   start and after every reprovisioning operation.
4. Use audited snapshot bootstrap only with a separately reviewed digest and
   height, and preserve the matching imported-prefix storage as one unit.

The retired `genesis.bootstrap_*` settings are rejected during configuration
parsing. There is no compatibility mode for peer genesis retrieval.
