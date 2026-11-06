# Ironwood discovery and BOI scenario integration

This is the source-level contract implemented in the private `../app-ironwood`
copy. The live BOI app is untouched. It is not a successful localnet or Taira
execution record.

## Authenticated discovery transport

Fetch complete native finality frames **anonymously** from existing public routes:

- `GET /v1/bridge/finality/{height}`: one `SumeragiFinalityProof`.
- `GET /v1/bridge/finality/interval/{from}/{to}`: canonical vector, at most 64 heights.

Request `application/x-norito`; send no account signature or merchant identifier.
Use bounded broad height ranges rather than identifying only the payment block.
The app acquires buckets of up to 64 heights and overlaps preceding history for
the newest partial bucket. Retain verified frames/checkpoints durably before reporting
credit. Recovery re-verifies those retained originals locally; it does not re-fetch
witness bytes or use their hashes as credit identities. Fresh discovery fetches
bounded broad ranges with explicit cache bypass to avoid narrowing cover traffic
to the paying block. This provides cover requests, not a
guarantee against timing, IP or intersection analysis. These frames were already public: the
removed signed Ironwood aliases added no confidentiality and linked an account to
its scanning heights. Both routes retain the native finality producer, finite
budgets, deadlines and canonical complete signed bodies/results. Neither receives
wallet keys nor filters notes by recipient. The app's
`LedgerClient.finality_proof_wires` returns exact individual frames; alternatively
hex-encode consecutive single-height responses.
Begin each scanner page at its independently trusted checkpoint; that first block
is authenticated but only the strict extension is scanned. Discovery returns stable
transaction/action identities and never establishes a credit by itself.

## Pool state and registry prerequisites

The authenticated cut path is `POST /v1/privacy/ironwood/pool-cut/{height}` with the
closed JSON body `{ "pool_id_hex": "...", "payer_account_id": "..." }`, canonical
request authentication and `Accept: application/x-norito`. A dedicated **operator
proof reader** must hold `CanReadAllLedgerData`; never use the merchant's discovery
identity. The response is an opaque canonical `IronwoodPoolCutProofV1` for the exact
current certified cut. It is not an unauthenticated current-pool hint and cannot
retrieve a discarded historical World snapshot.

Call `verify_ironwood_pool_cut_v1(proof, checkpoint_hex=...,
proof_frames_hex=[...], network_id_hex=..., chain_discriminant=...,
pool_id_hex=..., asset_definition_id=..., payer_account_id=...,
expected_profile_digest_hex=...)` with independently selected pins and a trusted
canonical checkpoint. The native verifier authenticates the complete consecutive
finality prefix, World root and accepted native schema, pool state/bootstrap,
retained head/root, selected asset/Global partition, payer and reserve balances.
It returns the anchor and epoch, exact certified height, Iroha header identity
(`block_hash_hex`), consensus core identity (`core_hash_hex`), execution context
(`context_id_hex`) and balance quantities. The app matches header and context to
its authenticated ordinary transaction projection. Retain the opaque cut and its
checkpoint/consecutive proof frames at preparation height **C** before signing.
The approved expiry is exactly **C + 32**. A race obtaining the current cut is a
resumable preparation refusal; it cannot strand an already-submitted payment.

At committed payment height **H**, call `verify_ironwood_payment_effect_v1` with
the retained cut proof and its original checkpoint/frames, exact original signed
transaction, expected pool/profile/asset/payer/amount, and consecutive certified
frames **C..H**. The verifier first authenticates C, derives the checkpoint for C,
then verifies C..H and the exact successful transaction at H. It checks the
anchor/epoch, expiry, Global IntoPool balance and immutable governed reserve.
A successful native effect establishes the exact **transaction-local payer debit
and reserve credit**. Whole-block balance differences cannot do that when unrelated
payments share a block. The app independently verifies the exact registry batch,
its required events/counters and recipient receipt against the same H/header/context.

No cut at H−1 or H is required, and C need not equal H−1. Retained originals allow
crash recovery after the chain advances, even if no historical World cut exists.
Missing certified frames remain resumable `RecordUnavailable`; archive enough
finality history for the stated recovery horizon. No JSON balance query can replace
these proofs, and no uncertified/rejected transaction produces an effect or credit.

Complete authenticated Core replay from the verified bootstrap remains an
alternative pool-state source, but filtering only direct privacy submissions is
insufficient when contracts, triggers or proposals can execute pool actions. The
note scanner is not a frontier/root-history or spending-witness reconstruction API.

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
   The app passes its existing scenario seed over the authenticated private worker
   pipe and erases its mutable buffer in `finally`; Rust erases the frame and owner
   material. No durable plaintext seed file is created. Old `payer-*` residues are
   removed on startup. No merchant spending authority reaches the payer.
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

The private `app-ironwood` copy now renders only ANON-01 and
STAT-01/03/04/06/07 through `scenario_ironwood.py`. Its explicitly admitted
`boi.ironwood-catalog-selection.v1` owner passes the worker-signed original into
`PreparedTransaction`, the ordinary Kasumi original-capsule journal and the native
`PublicExecutionJournal` receipt pipeline. Exact-payload fee admission runs after
worker signing but before journaling/submission; the original approved fee intent
is unchanged, and the app never splices or re-signs the batch. Existing Privacy Lab
Orchard paths retain their profile and APIs.

The ordinary catalog business policy checks both registry completion markers, no
`Moved`, every recipe expectation and STAT deltas against the authenticated S9
baseline. Native pool cuts check exact payer debit/reserve credit. The merchant
verifier has only its incoming-viewing credential and exact independent invoice
expectation. `ironwood_receipt_journal.py` atomically stores the credit, receipt
identity, invoice and checkpoint reference; a duplicate consume refuses, while
recovery reauthenticates evidence and reads the identical previously committed
credit. Memos are domain-separated by network/pool/run/scenario/namespace/label and
recipient/amount; they remain stable for a retry of the same logical invoice.

`tools/ironwood_campaign.py` and `specs/ironwood_campaign.md` in the app copy prepare
the four-peer campaign. It has not been run. Original candidate artifact/source
qualification and runtime admission records must be produced by the operator;
component tests are not a substitute. Retain all four node revisions,
genesis/committee/checkpoints, activation/bootstrap/grant/revocation proofs, signed
originals, native receipts, note identities and event/counter assertions. The live
app repository and Taira remain untouched.
