# State candidate comparison: frozen-source completion

The [candidate comparison](../../../specs/state_commitment_candidates.json) was
published on 2026-11-05 with **151 raw records, 95 native rows, 210 circuit rows
and one genuine BLAKE3 repeat**. Selection remains `null`. Refresh, the default
checker and strict `--reproduce --require-observation-contract ordinary_native_v1`
all exited successfully; the tracked bytes exactly match the reviewed proposal.

All seven runs passed the unchanged E.1 reader under `ordinary_native_v1`: five
unchanged accepted original runs, a fresh whole Rescue run under an eight-hour
scheduling ceiling, and a fresh BLAKE3 repeat under a four-hour ceiling. The
original four-hour Rescue timeout, stdout, manifest and entire partial raw tree
remain retained and excluded. The continuation does not turn that refusal into
a pass.

The recorded measurement checkout is the frozen local
`c63d346bc8ee1313bf39b8ad672aaf95cda10d8a` source and its dirty snapshot, with
Cargo.lock SHA-256
`de2c5b604926e792d35705a4d426bb0547d103779d96fb572103ca96c5bf6e88`.
The retained executable has SHA-256
`07c4507fb2c276e60c7d0fee7367af39bfafcc4edd4a53a6c3958dd8bc306cdc`.
These identities do not establish a fresh build from the latest working tree or
full external transitive build custody.

The shared State inventory changed after measurement, so the original strict
preparation refused; that refusal is retained. A separate reviewed reconciliation
route pins both exact inventories and their full approved JSON delta. All 405
field IDs, roles, shapes, keys and values are identical. The measured canonical
schema remains **215 tables, 58 cells and 38,299 descriptor bytes**, descriptor
SHA-256 `6e9f994ca2abab729b501c98b8531aa2d9313724214862661272c5abd04816b8`.
The other 27 publication inputs match their original hashes. Reconciliation does
not qualify the new lifecycle witness annotations or their implementation.

The unchanged composed finalizer retained the four independent campaign pins,
original timeout exclusion and strict E.1 checks. It verified 14 distinct raw
copies under the original external manifest pins again after all three commands,
preserved the original tracked bytes in its backup, and required exact proposal
bytes throughout publication. The reconciliation receipt is identical across
preparation and publication. Its 25 focused controls passed, including source,
projection, approved-delta and late tracked-byte refusal cases. The original
collector, finalizer, frozen inventory and source were not rewritten.

Independent final review verified the receipt crosslinks, command vectors and log
hashes, proposal and backup bytes, both inventory hashes and the other 27 source
pins. All 52 inspected file hashes stayed unchanged during that review; it did
not rerun the checker or reread raw journals. This comparison supplies local
measured candidate evidence. It selects no construction and makes no release,
network, latest-source or lifecycle-witness qualification claim.

Evidence remains under
`/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/state-candidates-isolated-20261104/`.
This persistent local cache is not permanent release custody. The original
strict refusal is
`continuation-finalization-attempts/prepare-20261105T130207.926209Z-3e17b32604b7/result.json`.
The successful finalizer receipt below binds all three command logs and the
original tracked-byte backup (`595d2f2032f9955888c1c66a4565b823ed101ca2063bc4991ffb39637db93142`).

| Retained artifact relative to that cache | SHA-256 |
| --- | --- |
| `timeout-continuation-01/collection/collection-result.json` | `ec0e7b5b51e395009332109ff1e955f7289fe1b4c270eb7908acc2a8543a3ad1` |
| `inventory-reconciliation-controls-20261105T131953.290131Z.json` | `0ecbdd327b1fc023321f312d0668976f72d6e19ccd99a932de7b704cd3b9a2a9` |
| `inventory-reconciliation-attempts/prepare-20261105T132418.323647Z-2c254ed92cb7/result.json` | `872a47b829d3d07caf1bb8f9296ff9e99b00189f00e4c34ecba14a5f1bc3cfce` |
| `timeout-continuation-01/aggregation/state-commitment-candidates-proposed.json` | `fc9061d501655070faaff27c420f7c011f76bd56e9cce1a467ffa4f8c5fd8932` |
| `inventory-reconciliation-attempts/prepare-20261105T132418.323647Z-2c254ed92cb7/reconciliation.json` | `0fae6ead9a05d6a6fdab11301b13141dac864550931a46165a1ae8d594152993` |
| `root-publication-review-20261105.json` | `8630173a552797d743db053b3578108117dbde2b07e63dd3b5f21de845b5eebc` |
| `inventory-reconciliation-attempts/publish-20261105T133643.932669Z-80cdceb0ce8d/result.json` | `2b79403203ef74231503a49014f42404be88655fb83f5df729785b707e60f579` |
| `continuation-finalization-attempts/publish-20261105T133643.951796Z-3360ae5e94ac/result.json` | `6dccd0a43f4be0a303107335664d71e8f0e1182fe14c4f4fcb4b86473c3c003c` |
| `state-publication-lightweight-review-20261105T140123.540852Z.json` | `b658c2b0eb153cd7f9dbc25b10256e704e91798a524175fd38f187564a96b3dd` |
