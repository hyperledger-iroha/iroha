# Startup Trust Roots

Iroha resolves a local trust root before it accepts a genesis block body or
replays persisted chain state. Peers do not distribute genesis, and the P2P
protocol has no genesis request or response messages.

## Normal startup

Normal startup is bound to both:

- the configured `genesis.public_key`; and
- one exact signed-genesis consensus-header hash.

The exact hash comes from a locally provisioned signed `genesis.file` or from
`genesis.expected_hash`. If both are present, they must agree. If neither is
present, startup fails before a genesis body is read from Kura. This ordering
means an on-disk block can satisfy a trust root that the operator already
selected, but cannot select its own trust root.

The signed genesis block must contain exactly one block signature, at index
zero. Iroha verifies that signature with `genesis.public_key`, verifies that
the first transaction authority embeds the same key, and requires the block's
consensus-header hash to equal the resolved exact hash. The header commits the
ordered transaction intents through their Merkle root; normal genesis
validation then enforces the remaining chain and execution rules.

A fresh node therefore needs the complete signed `genesis.file`. A restarting
node may omit that file only when `genesis.expected_hash` is configured and
the matching signed genesis body is already present in Kura. Empty storage
with only an expected hash fails because Iroha never fetches the missing body
from another peer.

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
2. Record its exact consensus-header hash as `genesis.expected_hash` when
   nodes must restart without retaining the external artifact.
3. Keep the external artifact and the configured hash identical whenever both
   are supplied.
4. Use audited snapshot bootstrap only with a separately reviewed digest and
   height, and preserve the matching imported-prefix storage as one unit.

The retired `genesis.bootstrap_*` settings are rejected during configuration
parsing. There is no compatibility mode for peer genesis retrieval.
