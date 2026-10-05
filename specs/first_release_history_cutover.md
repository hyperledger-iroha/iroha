# First-release history and cutover

Contract of task T.1 of the [ZK delivery plan](zk_delivery_plan.md). Its factual
part is [`first_release_history_cutover.json`](first_release_history_cutover.json),
which `scripts/check_first_release_history.py` checks against the source
(`pytest pytests/scripts/first_release_history_guard_test.py`).

This is an engineering rule, not a protocol activation. Editing this file, the
inventory or the plan authorizes no reset, deployment or movement of live value.
A live cutover is a separate operator decision carried out with the existing
signed deployment and reset tooling.

## One history per genesis

A history is one signed genesis and the certified blocks built on it, all in the
current canonical layouts. The genesis hash fixes the `NetworkId`, the consensus
instance peers bind in the handshake, and the network a Kura store is bound to.

Strict startup (`iroha_core::sumeragi::node::prepare`) starts from an empty
State: it re-executes the stored genesis, replays every stored block, compares
each result with the block's CommitQC, and finally compares the advanced World
accumulator with a cold capture. Any difference is `NodeError::Replay` and the
node does not start. A store whose durable journal names another network is
refused before a State is anchored to it, and a supplied genesis that is not the
stored one is refused before anything is applied.

Every history-bearing wire type admits exactly one version (`history_wire` in
the inventory). A stored block is the version byte `1`, the Norito header with
the fixed V1 flags and the canonical payload. Storage, block distribution and
genesis loading decode only that framed form (`decode_framed_signed_block`),
which refuses other versions, headerless payloads, other header layouts and
non-canonical encodings. `SignedBlock` still has a headerless `DecodeVersioned`
and a helper that frames a headerless payload; tests, one bench and one
integration test use them, and the guard refuses any production reference
(`test_only_references`, open finding T1-A15).

Retired on-disk artifacts are refused by name and never opened. The inventory
(`retired_store_artifacts`) lists every refusal with its function, its call
from the Strict open path and the names it refuses.

## Ordinary change

A change is ordinary when a node built from it starts in Strict mode on a store
written by an earlier build of the same history and reaches the same heights,
block hashes, certified results and World root.

Two kinds of test hold this. Within one build, `replay_tests` cover cold restart
and full replay: four validators write on-disk stores, every store is closed,
reopened and replayed with nothing else supplied, and real peers restart. Across
builds, `fixtures/core/first_release_history` pins the frames an earlier build
persisted together with their block hashes, World root and execution tip;
`pinned_history_of_an_earlier_build_replays_to_its_pinned_state` stores those
frames, reads them back from disk and replays them. A change that fails it is
not ordinary.

The pinned history is small: the node-test genesis and two blocks of `Log`
transactions. It detects a change to the block or transaction wire, to genesis
execution, to the certified result or to the World root. It does not exercise
every instruction, proof or contract; a change to one of those is classified by
the table below.

`iroha3d --check-storage` decodes every retained block of a stopped node with
the candidate build, and `iroha3d --check-config --json` prints the values peers
and the signed genesis bind. They are read-only diagnostics an operator may run
before installing a binary. They are not a deployment prerequisite.

## Deliberately incompatible change

A change is incompatible when it alters the bytes, validity or certified result
of anything a stored history contains. In the same change it must:

1. replace the layout, relation or semantics directly: no old-layout decoder,
   old-verifier branch, version dispatch, migration shim or compatibility alias;
2. regenerate every artifact the inventory lists for its surface
   (`incompatible_change_surfaces`, each with its generator or checking test);
3. state its cutover in the pull request;
4. make no claim that the previous history replays in the new binary.

| Id | Surface | Cutover | Also regenerate |
| --- | --- | --- | --- |
| `wire` | Block, transaction, entrypoint or query wire | `fresh_genesis` | Signed genesis, SDK fixtures and bindings, `norito.md` |
| `state_layout` | Instruction, event or State layout | `fresh_genesis` | Schema identities, OpenAPI, the owning spec |
| `execution_root` | Certified execution result or complete World root | `fresh_genesis` | `specs/sumeragi.md` Appendix E |
| `genesis` | Signed genesis manifest or its consensus metadata | `fresh_genesis` | Deployment genesis files |
| `execution_semantics` | Gas, executor or verifier semantics of anything a stored block contains | `fresh_genesis` | Gas goldens and schedule hash |
| `abi` | ABI V1: syscall list or semantics, pointer types, `abi_hash` | `contract_redeployment`, and a fresh genesis when the history holds contracts or runtime-ABI records | Kotodama goldens, contract artifacts, `crates/ivm/docs/syscalls.md` |
| `proof` | Proof relation, statement, transcript or parameters | `fresh_genesis_when_stored`: a fresh genesis when a stored block carries such a proof or State derived from it, otherwise direct replacement | Parameter and key artifacts, SDK builders |

The guard fails when this table and the inventory list different surfaces or
cutovers.

**Fresh genesis.** A new signed genesis starts a new history: a new `NetworkId`
and instance, and an empty store on every validator. The old store is not
migrated, re-rooted or partially replayed. Contracts are not part of genesis;
their owners recompile and redeploy them on the new history. The pinned history
is regenerated and its new `history_sha256` is recorded in the inventory in the
same change; the guard fails until both agree, so a regenerated history is
always a declared cutover:

```sh
IROHA_CAPTURE_FIRST_RELEASE_HISTORY_DIR="$PWD/fixtures/core/first_release_history" \
  cargo test -p iroha_core --lib capture_pinned_first_release_history -- --ignored
```

Without the variable the capture writes to a scratch directory and the tracked
fixture is untouched.

**Contract redeployment.** Admission compares a contract manifest's `code_hash`
with the hash of the submitted bytecode and its `abi_hash` with the ABI V1 hash
compiled into the node (`validate_manifest_hashes`); a mismatch stays a
rejection. Recompile `.ko` to `.to` and deploy the new manifest. Stored
deployments made under another `abi_hash` no longer replay, so a history that
contains any also needs a fresh genesis. Batch ABI changes per cutover.

Classify by effect on the stored history, not by intent. When unsure, run the
pinned-history test and the read-only probes against a store written by the
previous build.

## Obsolete histories

Keep evidence, not decoders. For a history that the current binary no longer
reads, retain as opaque bytes with digests: the stopped store, its signed
genesis and genesis hash, logs and exported telemetry, the last
`--check-storage` and `--check-config --json` reports of the build that wrote
it, and that build's source revision and build identity.

`scripts/first_release_history_evidence.py record` writes that manifest: every
store file with its size and SHA-256, the genesis, the two reports and what they
state about the history. `verify` recomputes the digests later. The tool hashes
bytes; it opens no store and decodes no block.

Diagnose an obsolete history with a binary built from the recorded revision.
The current tree gains no reader for it; a test that needs an obsolete shape
builds it locally and asserts its rejection. The Taira reset tooling follows the
same rule: a predecessor inventory is custody, never executable input.

## Snapshots and checkpoints

Restart is full replay from the signed genesis. Authenticated snapshot restore
is separate unfinished work: a positive-height snapshot does not replace native
replay today, and no delivery waits for it or assumes it.

A future authenticated checkpoint must state its height, block hash and
certified result, how they are authenticated against the genesis-bound
committee schedule, and the suffix it verifies: the blocks after the checkpoint,
replayed and compared with their certificates. It verifies nothing below its
height and cannot retroactively validate discarded history. A node restored
from one reports the height from which its history is verified.

## Halted chain

A chain that cannot commit is repaired by its operators: install a corrected
signed binary that still replays the current history, or carry out an explicitly
authorized reset. No on-chain approval, activation record or snapshot is part of
that path.

## Inventory and guard

The guard reads source text and the pinned fixture. It fails when:

- a history type admits a second version or a `Version` implementation is unlisted;
- a listed test disappears, is ignored or is conditionally compiled;
- the pinned history differs from its manifest or from the recorded digest;
- a history loader defines or imports a compatibility-named item that is neither
  a listed rejection helper nor a classified definition;
- a retired decoder or shim identifier reappears;
- production source references a helper kept only for tests;
- a retired-artifact refusal changes its names, loses its call from the open
  path, or exists without being listed;
- a surface lacks a concrete artifact with a generator or checking test;
- a probe or custody anchor of the obsolete-history evidence is gone.

The loader check is name-based. It does not see a fallback written inside an
existing function, a version `match` outside the `Version` trait or a neutral
name; review and the listed tests cover those. The guard does not run tests:
`--test-commands` prints the `cargo test` commands for exactly the listed tests,
and `blocked_test_targets` names targets that do not build today, so their tests
are not counted as evidence. Whether a retired artifact is refused at run time
is established by `every_inventoried_retired_store_artifact_stops_strict_startup`,
which plants each listed artifact in a real store.

`open_findings` lists compatibility constructs owned by other tasks, each with a
source anchor. `--report` prints them as `path:line`. When an owner removes one,
the guard reports the stale entry: delete it and list what was removed under
`retired_identifiers`.
