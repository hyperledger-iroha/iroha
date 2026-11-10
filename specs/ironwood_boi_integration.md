# Ironwood discovery and BOI scenario integration

The in-tree Python SDK implements Ironwood State reads, payment preparation and
receipt verification for the BOI scenario. The private application checkout
referenced by earlier integration work is unavailable in this workspace. Its
adoption of these APIs and its four-peer campaign remain unverified.

## Authenticated discovery transport

Fetch complete native finality frames **anonymously** from existing public routes:

- `GET /v1/bridge/finality/{height}`: one `SumeragiFinalityProof`.
- `GET /v1/bridge/finality/interval/{from}/{to}`: canonical vector, at most 64 heights.

Request `application/x-norito`; send no account signature or merchant identifier.
Use bounded broad height ranges rather than identifying only the payment block.
Acquire buckets of up to 64 heights and overlap preceding history for the newest
partial bucket. Retain verified frames/checkpoints durably before reporting
credit. Recovery re-verifies those retained originals locally; it does not re-fetch
witness bytes or use their hashes as credit identities. Fresh discovery fetches
bounded broad ranges with explicit cache bypass to avoid narrowing cover traffic
to the paying block. This provides cover requests, not a
guarantee against timing, IP or intersection analysis. These frames were already public: the
removed signed Ironwood aliases added no confidentiality and linked an account to
its scanning heights. Both routes retain the native finality producer, finite
budgets, deadlines and canonical complete signed bodies/results. Neither receives
wallet keys nor filters notes by recipient. Supply exact individual frames to the scanner. An application integration may
use `LedgerClient.finality_proof_wires` or hex-encode consecutive single-height
responses.
Begin each scanner page at its independently trusted checkpoint; that first block
is authenticated but only the strict extension is scanned. Discovery returns stable
transaction/action identities and never establishes a credit by itself.

## Pool state and registry prerequisites

Fetch the required State rows with signed `POST /v1/ledger/state-proof/{height}`
requests at the current certified Global tip. Use an operator proof reader with
`CanReadAllLedgerData` or a direct `CanReadStateRow` grant for each requested
position. The merchant's anonymous discovery path stays independent.

Construct `IronwoodStateSchemaV1(schema_bundle, expected_schema_hash_hex)` from
the independently selected schema digest and the deployed selection and storage
definitions. `ToriiClient.get_ironwood_pool_context` checks the pool state and
root head first, then derives the provenance and Global payer/reserve keys and
verifies their presence or absence at the same certified block. Supply the
trusted checkpoint, consecutive finality frames, network/chain, pool, asset,
payer and expected profile digest. A previous verified context can supply the
predicted keys for one batch; a changed anchor requires the missing row at the
same height.

Retain the original signed read responses, `pool_rows` and `balance_rows`,
checkpoint and proof frames for preparation height **C** before signing.
`verify_ironwood_state_rows_v1` reopens the retained pool state, anchor, epoch
and exact balances under the independently checked schema. Match the returned `block_hash_hex`, `core_hash_hex` and `context_id_hex` to its ordinary
transaction projection. The approved expiry is **C + 32**. If the height
advances between reads, restart preparation with a fresh certified height.

At committed payment height **H**, call `verify_ironwood_payment_effect_from_rows_v1`
with the retained row proofs and their original checkpoint/frames, exact original signed
transaction, expected pool/profile/asset/payer/amount, and consecutive certified
frames **C..H**. The verifier first authenticates C, derives the checkpoint for C,
then verifies C..H and the exact successful transaction at H. It checks the
anchor/epoch, expiry, Global IntoPool balance and immutable governed reserve.
A successful native effect establishes the exact **transaction-local payer debit
and reserve credit**. Whole-block balance differences cannot do that when unrelated
payments share a block. Independently verify the registry batch, its required
events and counters, and the recipient receipt against the same H/header/context.

No cut at H−1 or H is required, and C need not equal H−1. Retained originals allow
crash recovery after the chain advances, even if no historical World cut exists.
Missing certified frames remain resumable `RecordUnavailable`; archive enough
finality history for the stated recovery horizon. No JSON balance query can replace
these proofs, and no uncertified/rejected transaction produces an effect or credit.

Complete authenticated Core replay from the verified bootstrap remains an
alternative pool-state source, but filtering only direct privacy submissions is
insufficient when contracts, triggers or proposals can execute pool actions. The
note scanner is not a frontier/root-history or spending-witness reconstruction API.

For the selected LtHash16 World profile, the operator retains a private paired
`world.nrt` and `originals.nrt` after complete matched Core replay at C. The native
`verify_ironwood_world_replay_v1` SDK entry point authenticates these bounded
originals against the independent Global checkpoint, C height and header hash,
governed selection, compiled World schema and Ironwood proof profile. Its frozen
`VerifiedIronwoodWorldReplayContextV1` retains the authenticated public preparation
fields. `context_json()` exposes those fields and `verify_payment_effect()` verifies
the exact signed payment using the consecutive C-to-H finality extension. Recovery
reopens and verifies the retained pair against the same independent pins. World
frames are bounded at 32 MiB, typed originals at 4 MiB, and each finality page at
64 frames and 16 MiB. The full World stays in operator custody.

The operator's one-shot native producer is:

```sh
iroha3d --sora --config /private/replay/candidate.toml --check-replay \
  --ironwood-context-export-request /private/requests/context.json \
  --ironwood-context-export-output "/private/evidence/cut-00000000000000012345-<block-hash>"
```

Use the independently qualified release and a fresh, privately copied Kura and
snapshot cut. The replay must execute at least one block after its authenticated
start and match the entire retained history through the requested C. A restored
snapshot already at C cannot produce this evidence.

The closed JSON request has schema
`iroha.ironwood.replayed-context-request.v1` and fields `network_id`, `chain_id`,
`genesis_hash`, `height`, `block_hash`, `world_schema_hash_hex`, `selection_id_hex`,
`pool_id_hex`, `asset_definition`, `payer` and `reserve`. Supply native canonical
identifier/Hash JSON and lowercase nonzero 32-byte hex selectors. Bind these to
the independently authenticated C and reviewed runtime selection/schema. The
producer derives the core hash, certified context ID and World root from the
maintained native certified-cut reader and reports them with the original byte
identities on stdout.

The output name is `cut-<20-digit-height>-<lowercase-block-hash>`. Its existing
parent must be owner-private and outside the copied node/configuration paths.
The producer publishes both complete files together under a private directory
with exclusive creation. Existing destinations are never replaced. An interrupted
run or publication error requires reconciliation of the retained attempt before
another invocation. Separate private payer directories can hold distinct typed
originals for the same C. No full-World HTTP route or signing action is added.

Before either registry transaction, deploy and authenticate the actual registry's
contract address, expected code hash, manifest and invocation permissions. Initialize
the scenario's namespace, participant roles, request state and counters under the
registry's own authorization rules. Register the canonical accounts and any required
alias bindings separately. Fund the payer's selected public asset balance and fee
budget. A worker-produced privacy action does not deploy or initialize the registry,
grant entrypoint permissions, or satisfy the first authorization transaction.

## Required app calls

1. Construct `PrivacyWalletWorkerControllerV1(worker_executable,
   expected_worker_sha256=independently_qualified_worker_sha256)` for each owner.
   Merchant: `worker.derive_ironwood_address(spending_path)` exports only an
   `IronwoodRecipientV1`. Use `export_ironwood_viewing_key(spending_path, viewing_path)`
   once under native owner custody. The receipt verifier receives the incoming
   viewing file, never spending authority. Keep an independent trusted checkpoint,
   network/chain, asset, fresh Global pool, compiled profile, and invoice amount/memo.
2. Existing first registry transaction remains
   `boi_registry.authorize_anonymous_payment(ns, request_id, payer, amount, channel)`.
   Authenticate successful execution before constructing the second transaction.
3. Payer: call `ironwood_deposit_public_action_v1(asset_definition_id=..., pool_id=...,
   anchor=..., anchor_epoch=..., expiry_height=..., recipient=merchant_export,
   amount=atomic_amount, memo=invoice_memo_512_bytes, expected_profile_digest=...)`.
   The expected digest is independently selected, not copied from merchant input.
   Use the exact returned object in native owner bundle, public intent and plan;
   algorithm/protocol `ironwood-halo2-actions-v1`, schema `ironwood_note_action_v1`,
   witness `{"spends":[],"changes":[]}`. An already funded payer note cannot replace
   the public debit required by this registry. Advanced spends must separately
   approve public change address/value/memo and the same exact net public debit.
4. Call `lease = worker.import_ironwood_deposit_seed(payer_seed_bytearray, binding,
   canonical_public_action, ttl_millis=...,
   chain_discriminant=ledger.profile.chain_discriminant)`, then `worker.execute(lease.handle, binding,
   canonical_public_intent=..., canonical_execution_plan=...)`. Both canonical
   public objects must contain the identical closed ordered-call object:

   ```json
   {"orchard_contract_calls":{"schema_version":1,"before":[{"contract_address":"<authenticated deployed registry address>","expected_code_hash":"<authenticated code hash>","entrypoint":"open_anonymous","arguments":{"ns":"<namespace>","request_id":"<request>"}}],"after":[{"contract_address":"<same registry address>","expected_code_hash":"<same code hash>","entrypoint":"close_anonymous","arguments":{"ns":"<namespace>","request_id":"<request>"}}]}}
   ```

   This shows the field contract; `arguments` must be the native serialized argument
   record, obtained through the admitted SDK's compiled-artifact argument encoder
   and `TransactionBuilder` serialization, not the illustrative object above.
   Native command 15 retains the explicitly selected address prefix with the lease
   and constructs the empty-spend/empty-change owner bundle internally. Do not
   substitute the worker's default prefix (753) for the campaign's selected 369;
   doing so derives a different authority spelling and fails the execution binding.
   Pass the scenario seed over the authenticated private worker pipe and erase
   the mutable buffer in `finally`. Keep the seed out of durable files and remove
   any old `payer-*` residues on startup. The worker owns its frame and key material.
   The payer receives only the merchant's public recipient.
   The placeholders are not an executable fixture. Each call has exactly
   `contract_address`, `expected_code_hash`, `entrypoint`, and `arguments`.
   Submit the returned signed transaction as one Batch:
   `[boi_registry.open_anonymous(ns, request_id), SubmitPrivacyIronwoodProofV1,
   boi_registry.close_anonymous(ns, request_id)]`. Preserve the original gate's
   request ID. Native preparation randomizes first, binds the entire transaction,
   then authorizes once. Do not splice or re-sign a prepared action into another batch.
5. Share the signed transaction hash, `ironwood_direct_instruction_index`, and
   `ironwood_output_action_indices[0]` with the merchant. The mapping identifies
   the requested merchant output despite action randomization and dummy padding.
6. Merchant: request complete blocks through the routes above, then call
   `scan_ironwood_receipts_with_viewing_key(viewing_path, canonical_request)` with
   the independently selected pins and exact `expected_receipt` (transaction hash,
   direct/action indices, exported recipient, amount, all 512 memo bytes). The
   worker checks finality, successful direct submission, decryption and recomputed
   commitment. Discovery with no expected receipt is not payment acceptance.
7. Atomically record the returned `(transaction_hash_hex, action_index)` receipt
   identity with the credit, invoice and new checkpoint, scoped by network/pool/
   profile. Reject an already consumed identity. Use a unique memo per invoice.
   Replaying a prior page returns stable identities; the stateless worker is not a
   durable consumed-receipt ledger. Receipt verification does not report spentness.
   The Ironwood transaction-intent grammar permits exactly one direct
   `SubmitPrivacyIronwoodProofV1` (`iroha.privacy.submit_ironwood_proof.v1`), making
   this pair unique within its supported transaction form; the receipt expectation
   separately checks the direct instruction position. The frozen Exact12
   `SubmitPrivacyProofV1` does not carry Ironwood.
8. Assert `AnonymousOpened`/`AnonymousClosed`, no registry `Moved`, exact public
   balance decrease and reserve increase, and the ANON/STAT recipe's counters using
   authenticated committed results. A registry event alone cannot prove receipt.

## App ownership and pending four-peer qualification

Earlier integration notes identified ANON-01 and STAT-01/03/04/06/07 as
`scenario_ironwood.py` scenarios and named `boi.ironwood-catalog-selection.v1`
as the application owner. To qualify the application, confirm that it sends
the worker-signed original through `PreparedTransaction`, the Kasumi
original-capsule journal and the native `PublicExecutionJournal` receipt
pipeline. Run exact-payload fee admission after worker signing and before
journaling or submission. Preserve the approved fee intent and signed batch.

Check both registry completion markers, the absence of `Moved`, the recipe
expectations and the STAT deltas against the authenticated S9 baseline.
The native payment-effect verifier checks the transaction-local payer debit
and reserve credit against the retained preparation rows. Supply the merchant
verifier only its incoming-viewing credential and independent invoice
expectation. Atomically store the credit, receipt identity, invoice and
checkpoint reference, reject duplicate consumption, and recover the same
credit after reauthenticating its originals. Bind memos to network, pool,
run, scenario, namespace, label, recipient and amount, and keep them stable
across retries of the same logical invoice.

The earlier notes named `ironwood_receipt_journal.py`,
`tools/ironwood_campaign.py` and `specs/ironwood_campaign.md` in the private
checkout. Verify their current implementation when that checkout is available.
Four-peer qualification requires the candidate node revisions, genesis and
committee pins, checkpoints, activation/bootstrap/grant/revocation proofs,
signed originals, native receipts, note identities and event/counter assertions.
