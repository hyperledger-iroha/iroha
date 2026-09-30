# Native beacon DKG operator contract

`iroha3d_taira beacon-bootstrap` prepares public DKG evidence and private
per-seat credentials. It never submits a transaction or activates a committee.
The first release has one signed, encrypted all-edge DKG layout; the former
central dealer and decimal-height commands are removed.

For a fresh four-voter NPoS genesis, start one `provision-genesis-seat` process
per exact signed-genesis voter. Each process takes the canonical request,
signed prepared genesis manifest and block wire, genesis public key, canonical
height-one `BridgeFinalityProof`, independently pinned `--network-id` and
`--chain-discriminant`, and that voter's native BLS identity on `--key-fd 198`
or `--config-fd 198`. The daemon verifies the exact signed roster, possession
proofs, network, chain, context and fixed h1–h4 DKG schedule. Both session and
attempt IDs derive deterministically from the signed network ID, through the
same Core functions the in-process ceremony uses
(`iroha_core::beacon::ceremony::global_beacon_genesis_{session,attempt}_id_v1`).
Generation is zero. A new reset nonce cannot authorize another DKG for the same genesis.
Seat credentials are encoded and imported by Core's credential codec
(`iroha_core::beacon::credential`). The network deployment cutover deletes this
subcommand; `iroha network apply` drives the Core ceremony
(`iroha_core::beacon::ceremony`) instead.

Each process exclusively claims `attempt-<attempt-id>-seat-N` under an existing
owner-private `--attempt-root` before drawing its one dealer polynomial and
recipient secret. A crash consumes that attempt; missing local state is not
recreated under the same root. The process writes a signed public publication,
then one encrypted private share for every recipient, then one signed
acceptance for every received edge. No share scalar or dealer secret is
published. The supervisor relays only complete canonical public snapshots on
`--public-fd` and a separate contiguous authenticated h2–h4 finality chain on
`--finality-fd`. Both are FIFOs with four-byte big-endian frame lengths. Each
recipient verifies its encrypted edge against that dealer's exact signed
commitment before accepting it. Missing, duplicate, malformed or replayed
edges abort; a frozen roster is not shrunk or rerolled.

After every seat accepts every edge, its owner-private attempt directory
contains `iroha-global-beacon-partial-signer-v1.norito` and
`pending-share.bin`. Public `provider.json` and `public-session.norito` bind
that one seat to the exact finalized transcript. `assemble-genesis-dkg` takes
all four provider manifests, the public session and the h2–h4 proof files,
revalidates the complete chain and writes a public bundle with an unsigned
`FinalizeGlobalBeaconKey` draft. `sign-genesis-install` independently checks
that bundle and signs with one zero-based genesis voting index. Three distinct
current voters must sign. `assemble-genesis-install` verifies the exact quorum
and writes only the lifecycle instruction for ordinary fee-paying admission.
The certificate's effective height must follow finalization and precede the
first signed NPoS mandatory pulse.

For a rotation, `provision-rotation-seat` instead pins signed frozen-selection
evidence, the current context and height anchor, target epoch, transition ID,
exact target seat and authority generation. The same one-owner DKG process
emits pending custody. `assemble-rotation-dkg`, `sign-rotation` and
`assemble-rotation` verify the complete authenticated phase chain and require
the current exact quorum for the draft. `beacon-prepare-custody` retains an
incumbent's current credential while preparing the pending one from the
independently verified provisioning evidence. Only certified boundary finality
can activate the prepared committee; a missing target seat causes certified
retention or a safety halt, never an implicit roster change.

The public-reset Taira controller currently holds four validator configuration
copies on one administrative host while spawning separate per-seat DKG
processes. That tests signed all-edge protocol behavior but is not
operator-isolated custody. Disposable-network 4→7→4 activation and live
phase-time qualification remain open; a staged credential or completed public
transcript alone is not evidence of activation.

Ordinary `iroha3d` nodes with mint-finality duties set
`[sumeragi] mint_finality_seed_fd = 199` and receive one owner-private exact
32-byte seed file on inherited FD 199 for each startup. The daemon consumes
that launch copy before starting consensus. A genesis voter must match its
signed generation-zero Pasta keys and startup rejects a seated validator with
no exact held seed. A future candidate retains its seed without
genesis voting power and can sign only when an authenticated later authority
seats that same peer with matching keys. The supervisor must retain its private
source across restart and stage a fresh consumable launch copy each time. A
`data_dir` node started by the stock launcher instead reads the same raw seed
from the owner-only `<data_dir>/secrets/mint_finality.seed`, with the same
seated and candidate rules, and rejects `mint_finality_seed_fd`.
