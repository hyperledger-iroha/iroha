//! Bounded untrusted witness preparation; every native prediction is constrained.

use super::*;
use crate::finality::{event::PREFIX, result_scan::marked_hash_one_block};
use iroha_plonk_gadgets::bytes::p_bytes_native;

impl LoadReceiptProjection {
    fn from_transcript(bytes: &[u8; LoadReceiptCells::BYTES]) -> Result<Self, Error> {
        let identity = |at: usize| -> [u8; 32] { core::array::from_fn(|i| bytes[at + i]) };
        let integer = |at: usize| u128::from_le_bytes(core::array::from_fn(|i| bytes[at + i]));
        Ok(Self {
            digest: p_bytes_native(LoadReceiptCells::DOMAIN, bytes),
            scheme: identity(2),
            asset: identity(34),
            wallet: identity(66),
            request: identity(98),
            transaction: identity(210),
            payer: identity(250),
            ordinal: integer(130),
            amount: integer(146),
            online_charge: integer(162),
            charge_quote: Option::<Fp>::from(Fp::from_repr(identity(178)))
                .ok_or(Error::Synthesis)?,
            height: u64::from_le_bytes(core::array::from_fn(|i| bytes[242 + i])),
        })
    }
}

/// Prepare all 35 source leaves from the original receipt and retained counted
/// event path. All supplied values remain proposals: bad receipt terms, incorrect
/// paths or prefix mismatches fail their circuits. This function never verifies
/// finality and returns no monetary capability. The caller must independently
/// authenticate this exact result tape through the complete result-scan source.
/// # Errors
/// Oversized result, noncanonical quote field, or fixed bounded witness failure.
pub fn prepare_load_source(
    frame: &[u8],
    receipt: &[u8; LoadReceiptCells::BYTES],
    event_root: [u8; 32],
    event_count: u64,
    event_index: u32,
    siblings: &[[u8; 32]; 32],
) -> Result<Vec<LoadSourceCircuit>, Error> {
    if frame.len() > MAX_RESULT_BYTES as usize {
        return Err(Error::Synthesis);
    }
    let tape = Arc::new(ResultTapeWitness::from_frame(&Value::known(
        frame.to_vec(),
    ))?);
    let mut root = None;
    let _ = tape.root().map(|value| root = Some(value));
    let context = LoadSourceContext {
        result_root: root.ok_or(Error::Synthesis)?,
        result_frame_len: u32::try_from(frame.len()).map_err(|_| Error::BoundsFailure)?,
        receipt: LoadReceiptProjection::from_transcript(receipt)?,
        event_root,
        event_count,
        event_index,
    };
    let mut event = PREFIX.to_vec();
    event.extend_from_slice(&context.receipt.digest.to_repr());
    let event_hash = marked_hash_one_block(&event)?;
    let mut message = b"iroha:merkle:leaf:v1\0".to_vec();
    message.extend_from_slice(&event_hash);
    let mut state = PathState {
        index: event_index,
        width: event_count,
        digest: marked_hash_one_block(&message)?,
    };
    let leaf = |plan, cursor, before, after, sibling| LoadSourceCircuit {
        plan,
        context,
        cursor,
        before,
        after,
        sibling,
        receipt: *receipt,
        tape: Arc::clone(&tape),
        known: true,
    };
    let mut leaves = Vec::with_capacity(PROGRAM_LENGTH as usize);
    leaves.push(leaf(
        LoadSourcePlan::Start,
        0,
        PathState::default(),
        state,
        [0; 32],
    ));
    for (i, sibling) in siblings.iter().enumerate() {
        let before = state;
        if state.width > 1 {
            if (u64::from(state.index) ^ 1) < state.width {
                let mut message = b"iroha:merkle:internal:v1\0".to_vec();
                let (left, right) = if state.index & 1 == 1 {
                    (sibling, &state.digest)
                } else {
                    (&state.digest, sibling)
                };
                message.extend_from_slice(left);
                message.extend_from_slice(right);
                state.digest = marked_hash_one_block(&message)?;
            }
            state.index >>= 1;
            state.width = (state.width >> 1) + (state.width & 1);
        }
        leaves.push(leaf(
            LoadSourcePlan::Path,
            u32::try_from(i).map_err(|_| Error::BoundsFailure)? + 1,
            before,
            state,
            *sibling,
        ));
    }
    leaves.push(leaf(LoadSourcePlan::Prefix, 33, state, state, [0; 32]));
    leaves.push(leaf(
        LoadSourcePlan::Finish,
        34,
        state,
        PathState::default(),
        [0; 32],
    ));
    Ok(leaves)
}
