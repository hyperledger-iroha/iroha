---
lang: am
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
---

% FastPQ Transfer Gadget Design

# Overview

The current FASTPQ planner records every primitive operation involved in a `TransferAsset` instruction, which means each transfer pays for balance arithmetic, hash rounds, and SMT updates separately. To reduce trace rows per transfer we introduce a dedicated gadget that verifies only the minimal arithmetic/commitment checks while the host continues to execute the canonical state transition.

- **Scope**: single transfers and small batches emitted via the existing Kotodama/IVM `TransferAsset` syscall surface.
- **Goal**: cut FFT/LDE column footprint for high-volume transfers by sharing lookup tables and collapsing per-transfer arithmetic into a compact constraint block.

# Architecture

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## Transcript Format

The host emits a `TransferTranscript` per syscall invocation:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` ties the transcript to the transaction entrypoint hash for replay protection.
- `authority_digest` is the host’s hash over sorted signers/quorum data; the gadget checks equality but does not redo signature verification. Concretely the host Norito-encodes the `AccountId` (which already embeds the canonical multisig controller) and hashes `b"iroha:fastpq:v1:authority|" || encoded_account` with Blake2b-256, storing the resulting `Hash`.
- `poseidon_preimage_digest` = Poseidon(account_from || account_to || asset || amount || batch_hash); ensures the gadget recomputes the same digest as the host. The preimage bytes are constructed as `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` using bare Norito encoding before passing them through the shared Poseidon2 helper. This digest is present for single-delta transcripts and omitted for multi-delta batches.

All fields are serialized via Norito so existing determinism guarantees hold.
Both `from_path` and `to_path` are emitted as Norito blobs using the
`TransferMerkleProofV1` schema: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
Future versions can extend the schema while the prover enforces the version tag
before decoding. `TransitionBatch` metadata embeds the Norito-encoded transcript
vector under the `transfer_transcripts` key so the prover can decode the witness
without performing out-of-band queries. Public inputs (`dsid`, `slot`, roots,
`perm_root`, `tx_set_hash`) are carried in `FastpqTransitionBatch.public_inputs`,
leaving metadata for entry hash/transcript count bookkeeping. Until host plumbing
lands, the prover synthetically derives proofs from the key/balance pairs so rows
always include a deterministic SMT path even when the transcript omits the optional fields.

## Gadget Layout

1. **Balance Arithmetic Block**
   - Inputs: `from_balance_before`, `amount`, `to_balance_before`.
   - Checks:
     - `from_balance_before >= amount` (range gadget with shared RNS decomposition).
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - Packed into a custom gate so all three equations consume one row group.

2. **Poseidon Commitment Block**
   - Recomputes `poseidon_preimage_digest` using the shared Poseidon lookup table already used in other gadgets. No per-transfer Poseidon rounds in the trace.

3. **Merkle Path Block**
   - Extends the existing Kaigi SMT gadget with a "paired update" mode. Two leaves (sender, receiver) share the same column for sibling hashes, reducing duplicated rows.

4. **Authority Digest Check**
   - Simple equality constraint between the host-provided digest and the witness value. Signatures remain in their dedicated gadget.

5. **Batch Loop**
   - Programs call `transfer_v1_batch_begin()` before a loop of `transfer_asset` builders and `transfer_v1_batch_end()` afterwards. While the scope is active the host buffers each transfer and replays them as a single `TransferAssetBatch`, reusing the Poseidon/SMT context once per batch. Each additional delta adds only the arithmetic and two leaf checks. The transcript decoder now accepts multi-delta batches and surfaces them as `TransferGadgetInput::deltas` so the planner can fold witnesses without re-reading Norito. Contracts that already have a Norito payload handy (e.g., CLI/SDKs) can skip the scope entirely by calling `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`, which hands the host a fully encoded batch in one syscall.

# Host & Prover Changes

| Layer | Changes |
|-------|---------|
| `ivm::syscalls` | Add `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) so programs can bracket multiple `transfer_v1` syscalls without emitting intermediate ISIs, plus `transfer_v1_batch_apply` (`0x2B`) for pre-encoded batches. |
| `ivm::host` & tests | Core/Default hosts treat `transfer_v1` as a batch append while the scope is active, surface `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}`, and the mock WSV host buffers entries before committing so regression tests can assert deterministic balance updates.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs:3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Emit `TransferTranscript` after the state transition, build `FastpqTransitionBatch` records with explicit `public_inputs` during `StateBlock::capture_exec_witness`, and run the FASTPQ prover lane so both Torii/CLI tooling and the Stage 6 backend receive canonical `TransitionBatch` inputs. `TransferAssetBatch` groups sequential transfers into a single transcript, omitting the poseidon digest for multi-delta batches so the gadget can iterate across entries deterministically. |
| `fastpq_prover` | `gadgets::transfer` now validates multi-delta transcripts (balance arithmetic + Poseidon digest) and surfaces structured witnesses (including placeholder paired SMT blobs) for the planner (`crates/fastpq_prover/src/gadgets/transfer.rs`). `trace::build_trace` decodes those transcripts out of batch metadata, rejects transfer batches missing the `transfer_transcripts` payload, attaches the validated witnesses to `Trace::transfer_witnesses`, and `TracePolynomialData::transfer_plan()` keeps the aggregated plan alive until the planner consumes the gadget (`crates/fastpq_prover/src/trace.rs`). The row-count regression harness now ships via `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), covering scenarios up to 65 536 padded rows, while the paired SMT wiring remains behind the TF-3 batch-helper milestone (placeholders keep the trace layout stable until that swap lands). |
| Kotodama | Lowers the `transfer_batch((from,to,asset,amount), …)` helper into `transfer_v1_batch_begin`, sequential `transfer_asset` calls, and `transfer_v1_batch_end`. Each tuple argument must follow the `(AccountId, AccountId, AssetDefinitionId, int)` shape; single transfers keep the existing builder. |

Example Kotodama usage:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` executes the same permission and arithmetic checks as individual `Transfer::asset_numeric` calls, but records all deltas inside a single `TransferTranscript`. Multi-delta transcripts elide the poseidon digest until per-delta commitments land in a follow-up. The Kotodama builder now emits the begin/end syscalls automatically, so contracts can deploy batched transfers without hand-encoding Norito payloads.

## Row-count Regression Harness

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) synthesizes FASTPQ transition batches with configurable selector counts and reports the resulting `row_usage` summary (`total_rows`, per-selector counts, ratio) alongside the padded length/log₂. Capture benchmarks for the 65 536-row ceiling with:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```

The emitted JSON mirrors the FASTPQ batch artifacts that `iroha_cli audit witness` now emits by default (pass `--no-fastpq-batches` to suppress them), so `scripts/fastpq/check_row_usage.py` and the CI gate can diff the synthetic runs against prior snapshots when validating planner changes.

# Rollout Plan

1. **TF-1 (Transcript plumbing)**: ✅ `StateTransaction::record_transfer_transcripts` now emits Norito transcripts for every `TransferAsset`/batch, `sumeragi::witness::record_fastpq_transcript` stores them inside the global witness, and `StateBlock::capture_exec_witness` builds `fastpq_batches` with explicit `public_inputs` for operators and the prover lane (use `--no-fastpq-batches` if you need a slimmer output).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (Gadget implementation)**: ✅ `gadgets::transfer` now validates multi-delta transcripts (balance arithmetic + Poseidon digest), synthesises paired SMT proofs when hosts omit them, exposes structured witnesses via `TransferGadgetPlan`, and `trace::build_trace` threads those witnesses into `Trace::transfer_witnesses` while populating SMT columns from the proofs. `fastpq_row_bench` captures the 65 536-row regression harness so planners track row usage without replaying Norito payloads.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (Batch helper)**: Enable the batch syscall + Kotodama builder, including host-level sequential application and gadget loop.
4. **TF-4 (Telemetry & docs)**: Update `fastpq_plan.md`, `fastpq_migration_guide.md`, and dashboard schemas to surface allocation of transfer rows vs other gadgets.

# Open Questions

- **Domain limits**: current FFT planner panics for traces beyond 2¹⁴ rows. TF-2 should either raise the domain size or document a reduced benchmark target.
- **Multi-asset batches**: initial gadget assumes the same asset ID per delta. If we need heterogeneous batches, we must ensure the Poseidon witness includes the asset each time to prevent cross-asset replay.
- **Authority digest reuse**: long term we can reuse the same digest for other permissioned operations to avoid recomputing signer lists per syscall.


This document tracks design decisions; keep it in sync with roadmap entries when milestones land.
