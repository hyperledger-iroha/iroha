# Native Race V1

Native races are an immutable, versioned Iroha state machine. Wallets authorize an exact entry stake and a fresh Ed25519 gameplay key; that key only authenticates race checkpoints, input commitments, and challenges. Any fee-paying transaction account can relay gameplay evidence. Payout recipients are the immutable joined wallet accounts. There is no contract owner, administrator settlement signature, spending allowance, or mandatory relay.

The native proof profile is deliberately **unqualified** until the execution-relation, mutation, adversarial multiplayer, four-validator network, and release resource gates pass. `JoinRaceV1` rejects funding while this gate is false. A deployed endpoint advertising support alone does not authorize the browser to accept stakes.

## Interface

All instructions use the registry prefix `iroha.instruction.v1::race::`.

| Instruction | Authority and effect |
| --- | --- |
| `OpenRaceV1` | Creates a unique lobby with compiled rules, configured fee-XOR asset, positive exact stake, 2–8 seats, and a bounded join height. |
| `JoinRaceV1` | Transaction wallet pays precisely the lobby stake and registers a distinct gameplay key and car skin. |
| `StartRaceV1` | Any payer starts a full lobby, or an expired lobby containing at least two entrants. |
| `CommitRaceCheckpointV1` | Any payer supplies every active slot's signature over a cumulative checkpoint, optionally with its pending commitment frontier. |
| `ChallengeRaceV1` | Any payer relays a current active slot's signed challenge; opens a fixed checkpoint-selection window. |
| `CommitRaceInputsV1` | Relays a signed exact-slot commitment during forced commit. Commitments cannot be replaced. |
| `RevealRaceInputsV1` | Supplies six controls and salt matching the retained commitment; no additional wallet signature is needed. |
| `AdvanceRaceDeadlineV1` | Any payer advances an expired consensus-height phase. |
| `SubmitRaceProofV1` | Any payer submits the complete native execution proof; verification and the exact escrow payout occur atomically. |
| `ExpireRaceV1` | Any payer refunds an expired lobby containing fewer than two entrants. |

`FindRaceById` is the typed singular query. Public discovery routes are `GET /v1/races/capabilities`, `GET /v1/races?cursor=<exclusive-hash>`, and `GET /v1/races/{race_id}`. A list page contains at most 32 summaries. Full records expose the checkpoint, forced history, liability, revision, and final proof-derived result. Ordinary Torii JSON remains node evidence until a client verifies chain finality. `RaceEventV1` carries race id, revision, phase, and selected dispute root.

## Live play and canonical disputes

The simulation uses 30 ticks per second, six-tick input batches, up to eight simultaneous cars, three compiled tracks and three laps, with a maximum of 5,400 ticks. Cars collide in deterministic slot order. Skins have identical mechanical parameters. The proof relation is defined by `execution_proofs`, not by the graphics frame rate.

Before disclosing any batch controls, every active player signs the same complete commitment-set body. Before the first disclosure, peers also sign the tick-zero checkpoint. A later checkpoint signs the canonical complete input-transcript root and simulation-state root. Input reveals include race, epoch, tick, slot, and a random salt. Each gameplay domain uses the canonical compact-Norito body and the `iroha:race:gameplay:v1\0` prefix plus exact NetworkId.

A challenge opens 30 consensus blocks for the highest certified checkpoint and pending commitment frontier. A same-tick frontier cannot be discarded or replaced once retained. A terminal certificate submitted within selection does not shorten this window. Once selection closes, its transcript prefix is fixed; late earlier proofs cannot settle another history.

Forced commit and reveal phases each have 15 consensus blocks. Anybody can make a retained reveal available. Only the expired native deadline can mark an absent slot DNF, at the exact next simulation tick. Its collision body then disappears; completed finishes are preserved by the execution relation. Remaining racers continue. Every resolved six-tick batch is retained, increments the epoch, and contributes to the selected dispute root.

Resuming live play requires a jointly signed checkpoint at the exact end of a newly completed forced batch, in its new epoch, and newer than the retained checkpoint. A pre-dispute checkpoint cannot be replayed to restart deadlines. Already published forced commitments prevent resumption until their batch resolves.

## Settlement and custody

The native verifier checks the immutable profile, network, race, roster, rules, track, selected history, controls and DNF events against the physics proof. Results cannot be accepted merely because a digest or checkpoint has valid signatures. Certified transcript roots and every forced batch are authenticated in addition to the execution relation.

A successful proof pays all tied earliest finishers, splitting at the asset precision with any indivisible remainder assigned to the final ordered winning slot. If no car finishes, each wallet receives its entry stake back. A finished or refunded record cannot pay twice. There is no post-start timeout refund that can override another player's provable win.

Custody keys are derived with the non-signing public-key construction and a race-specific domain. Generic numeric transfers and burns cannot debit retained native custody, including closed records. A private one-shot movement capability limits funding and settlement to exact retained stakes, original recipients, and total liability. Wallet account removal/rekey and asset-definition removal are blocked while outstanding stakes depend on them. Custody identities remain reserved after settlement.

## Qualification evidence

- `iroha_data_model` grouped integration test `race_v1_codec` compares independent browser and native compact-Norito bytes and gameplay digests.
- Core `smartcontracts::isi::race::tests` exercises duplicate/missing/stale signatures, frontier preservation, fixed challenge deadlines, DNF progression, custody retention, disabled funding, and checkpoint replay prevention.
- The four-state deterministic replay test is a unit replication check. It is not a four-validator network qualification run.
- Live-stake release additionally requires successful execution-proof semantic mutation tests, actual four-validator finality/dispute/settlement runs, browser authenticated-finality verification, and bounded proof time, verification time, memory, transaction size and gas evidence.
