//! Serialized background ordinary credit production from genuine committed Node originals.
//! No HTTP read performs proof generation. Cold startup resumes the authentic immutable
//! checkpoint/output rows; uncertainty closes this process worker rather than replacing bytes.
use super::ordinary_mint_checkpoint::KagemushaOrdinaryMintCheckpointOwnerV1;
use super::ordinary_mint_publication::KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1;
use super::*;
use crate::state::{State, StateReadOnly, StateView};
use iroha_data_model::{
    block::SignedBlock,
    isi::kagemusha_v1::TopUpKagemushaOrdinaryV1,
    kagemusha::{
        KAGEMUSHA_ORDINARY_FINALIZED_MINT_CREDIT_MAX_BYTES_V1,
        KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1,
        KagemushaOrdinaryFinalizedMintCreditOriginalV1, KagemushaOrdinaryTopUpRequestV1,
    },
    transaction::{Executable, ExecutableBatchItem, TransactionEntrypoint},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use std::{num::NonZeroUsize, time::Duration};

const SCAN_INSTRUCTIONS_PER_ROUND: usize = 256;
const CHECKPOINT_EDGES_PER_ROUND: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cursor {
    height: usize,
    entry: usize,
    instruction: usize,
}
impl Default for Cursor {
    fn default() -> Self {
        Self {
            height: 2,
            entry: 0,
            instruction: 0,
        }
    }
}

/// Start the one serialized producer after canonical replay and installed runtime admission.
/// The actual State owns every selected World/Kura/runtime original. Shutdown drains an
/// already admitted non-preemptible proof; it cannot report completion while that proof runs.
/// # Errors
/// Returns an error if the runtime cannot spawn the owned background task.
pub fn start_ordinary_mint_credit_publication_v1(
    state: Arc<State>,
    shutdown: ShutdownSignal,
) -> Result<Child, String> {
    let task = tokio::spawn(async move {
        let mut cursor = Cursor::default();
        loop {
            if shutdown.is_sent() {
                break;
            }
            let held = Arc::clone(&state);
            let outcome = tokio::task::spawn_blocking(move || process_round(&held, cursor)).await;
            match outcome {
                Ok(Ok(next)) => cursor = next,
                Ok(Err(error)) => {
                    iroha_logger::error!(%error,
                        "ordinary Mint credit publication closed; exact durable originals require cold readmission");
                    // No new proof, nonce or replacement is dispatched after an uncertain
                    // outcome. The supervisor retains this closed worker until shutdown.
                    shutdown.receive().await;
                    break;
                }
                Err(error) => {
                    iroha_logger::error!(%error, "ordinary Mint credit publication worker panicked");
                    shutdown.receive().await;
                    break;
                }
            }
            tokio::select! {
                () = shutdown.receive() => break,
                () = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        }
    });
    Ok(Child::new(task, OnShutdown::Drain))
}

/// Complete source/result original working-set ceiling shared by background and read admission.
/// Six finalized/result representations cover input, structured finality, exact reencoding,
/// generated/readback credit and charged response; three full Node submission representations
/// cover the archived original, bounded decoder and historical credential/clock admissions.
/// Loaded keys and proving scratch keep the maintained kernel's separate resource preflight.
pub fn ordinary_mint_credit_publication_working_set_bytes_v1() -> Option<usize> {
    KAGEMUSHA_ORDINARY_FINALIZED_MINT_CREDIT_MAX_BYTES_V1
        .checked_mul(6)?
        .checked_add(KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1.checked_mul(3)?)
}

fn process_round(state: &State, mut cursor: Cursor) -> Result<Cursor, String> {
    let view = state.view();
    let durable = view
        .kura()
        .exact_durable_blocks_count()
        .map_err(|e| e.to_string())?;
    let tip = view.height().min(durable);
    if cursor.height > tip {
        return Ok(cursor);
    }
    // Charge every owned source/result representation to the real existing execution pool
    // before construction. The maintained proving kernels retain separate resource preflight.
    let working = ordinary_mint_credit_publication_working_set_bytes_v1()
        .ok_or("ordinary publication working-set overflow")?;
    let budget = view.prepared_contract_cache().execution_budget().clone();
    let Ok(_originals) = budget.try_reserve_bytes(working) else {
        // No original/proof/write has started; finite pool contention may retry this same cut.
        return Ok(cursor);
    };
    let mut remaining = SCAN_INSTRUCTIONS_PER_ROUND;
    let mut selected = None;
    while cursor.height <= tip && remaining > 0 {
        let height =
            NonZeroUsize::new(cursor.height).ok_or("ordinary producer cursor height is zero")?;
        let block = view
            .canonical_block_by_height(height)
            .map_err(|e| e.to_string())?;
        let original_cursor = cursor;
        let (next, operation) = next_operation(&block, cursor, &mut remaining)?;
        if let Some(operation) = operation {
            selected = Some((original_cursor, next, operation));
            break;
        }
        cursor = next;
    }
    let Some((original_cursor, next, operation)) = selected else {
        return Ok(cursor);
    };
    cursor = original_cursor;
    let source = KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1::authenticate(
        view,
        u64::try_from(cursor.height).map_err(|e| e.to_string())?,
        operation,
    )?;
    if read_existing(&source)?.is_some() {
        return Ok(next);
    }
    let mut checkpoint = KagemushaOrdinaryMintCheckpointOwnerV1::open(&source)?;
    for _ in 0..CHECKPOINT_EDGES_PER_ROUND {
        if checkpoint.ready()?.is_some() {
            break;
        }
        checkpoint.advance_one()?;
    }
    let Some(checkpoint) = checkpoint.ready()? else {
        // Retain the same operation cursor. The fsynced authenticated epoch hint permits the
        // next finite round to resume even after more than sixteen historical boundaries.
        return Ok(cursor);
    };
    let generated = source
        .runtime()?
        .prove_finalized_ordinary_top_up(&source, checkpoint)?;
    source
        .view()
        .kura()
        .store_ordinary_mint_credit_original_v1(&source, &generated)
        .map_err(|e| e.to_string())?;
    if read_existing(&source)?.is_none() {
        return Err("ordinary credit absent after exact durable publication".into());
    }
    cursor = next;
    Ok(cursor)
}

// Scan at most the finite instruction budget. Neither failed transactions nor a different
// instruction family can request credit production. The exact source constructor rechecks
// the complete successful signed submission and immutable record before any proof starts.
fn next_operation(
    block: &SignedBlock,
    mut cursor: Cursor,
    remaining: &mut usize,
) -> Result<(Cursor, Option<[u8; 32]>), String> {
    // One unit per block prevents an unbounded loop through empty historical carriers.
    if *remaining == 0 {
        return Ok((cursor, None));
    }
    *remaining -= 1;
    for (index, entry) in block.network_entrypoints().enumerate().skip(cursor.entry) {
        if *remaining == 0 {
            return Ok((
                Cursor {
                    entry: index,
                    instruction: if index == cursor.entry {
                        cursor.instruction
                    } else {
                        0
                    },
                    ..cursor
                },
                None,
            ));
        }
        *remaining -= 1;
        let TransactionEntrypoint::External(signed) = entry else {
            cursor.entry = index + 1;
            cursor.instruction = 0;
            continue;
        };
        let output = block
            .network_output_at(u32::try_from(index).map_err(|e| e.to_string())?)
            .ok_or("ordinary producer execution output absent")?
            .1;
        if output.result.is_err() {
            cursor.entry = index + 1;
            cursor.instruction = 0;
            continue;
        }
        let instructions: Box<dyn Iterator<Item = &iroha_data_model::isi::InstructionBox> + '_> =
            match signed.instructions() {
                Executable::Instructions(items) => Box::new(items.iter()),
                Executable::Batch(items) => Box::new(items.iter().filter_map(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => Some(instruction),
                    ExecutableBatchItem::ContractCall(_) => None,
                })),
                _ => {
                    cursor.entry = index + 1;
                    cursor.instruction = 0;
                    continue;
                }
            };
        let start = if index == cursor.entry {
            cursor.instruction
        } else {
            0
        };
        for (instruction_index, instruction) in instructions.enumerate().skip(start) {
            if *remaining == 0 {
                return Ok((
                    Cursor {
                        entry: index,
                        instruction: instruction_index,
                        ..cursor
                    },
                    None,
                ));
            }
            *remaining -= 1;
            cursor.entry = index;
            cursor.instruction = instruction_index
                .checked_add(1)
                .ok_or("ordinary instruction cursor overflow")?;
            if let Some(top_up) = instruction
                .as_any()
                .downcast_ref::<TopUpKagemushaOrdinaryV1>()
            {
                let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
                    &top_up.request.topup_request_original,
                )?;
                return Ok((
                    cursor,
                    Some(request.authorization.statement.context.operation_id),
                ));
            }
        }
        cursor.entry = index + 1;
        cursor.instruction = 0;
    }
    Ok((
        Cursor {
            height: cursor
                .height
                .checked_add(1)
                .ok_or("ordinary producer height overflow")?,
            entry: 0,
            instruction: 0,
        },
        None,
    ))
}

fn read_existing(
    source: &KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'_>,
) -> Result<Option<KagemushaOrdinaryFinalizedMintCreditOriginalV1>, String> {
    source.recheck_retained_custody()?;
    let operation = source.record()?.operation_id;
    let Some(value) = source
        .view()
        .kura()
        .ordinary_mint_credit_original_v1(operation)
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    if value.finalized_source_original != source.finalized()?.canonical_bytes()? {
        return Err("ordinary published credit changes its complete finalized source".into());
    }
    let credit = KagemushaMintCreditV1::decode_canonical_shape_exact(&value.mint_credit_original)
        .map_err(|e| e.to_string())?;
    let (bundle, _) = crate::sumeragi::attestation::verify_native_mint_finality_bundle(
        &source.finalized()?.finality.finality_proof,
        source.trust_anchor()?,
    )?;
    let head = bundle
        .message
        .epoch_authorization
        .authorization_id()
        .map_err(|e| e.to_string())?;
    let checkpoint = source
        .view()
        .kura()
        .kagemusha_mint_authority_checkpoint_v1(source.record()?.release_id, head)
        .map_err(|e| e.to_string())?
        .ok_or("ordinary published credit authority checkpoint absent")?;
    source
        .runtime()?
        .verify_finalized_ordinary_top_up(source, &credit, &checkpoint)?;
    source.recheck_retained_custody()?;
    Ok(Some(value))
}

/// Read an already published credit after genuine historical Node source and installed-key
/// verification. Absence is a closed pending result. This method never proves or debits.
/// # Errors
/// Refuses changed originals, missing finality/checkpoints, invalid proofs or another owner.
pub fn read_published_ordinary_mint_credit_v1(
    view: StateView<'_>,
    height: u64,
    operation: [u8; 32],
) -> Result<Option<KagemushaOrdinaryFinalizedMintCreditOriginalV1>, String> {
    if view
        .kura()
        .ordinary_mint_credit_original_v1(operation)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Ok(None);
    }
    let source = KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1::authenticate(
        view, height, operation,
    )?;
    read_existing(&source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_producer_bounded_scan_preserves_exact_position_without_skipping_originals() {
        let fixture =
            iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture::new();
        let block = iroha_data_model::block::decode_framed_signed_block(
            &fixture.genesis_proof().block_wire,
        )
        .unwrap();
        let mut cursor = Cursor {
            height: 2,
            entry: 0,
            instruction: 0,
        };
        let mut zero = 0;
        assert_eq!(
            next_operation(&block, cursor, &mut zero).unwrap(),
            (cursor, None)
        );
        // These known-public genuine signed genesis instructions are inert scan data; they
        // never construct a finalized source, checkpoint, generated credit or financial grant.
        for _ in 0..1024 {
            let mut budget = 3;
            let (next, operation) = next_operation(&block, cursor, &mut budget).unwrap();
            assert!(operation.is_none());
            assert!(next != cursor);
            cursor = next;
            if cursor.height == 3 {
                break;
            }
        }
        assert_eq!(cursor.height, 3);
        assert_eq!((cursor.entry, cursor.instruction), (0, 0));
    }
}
