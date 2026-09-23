//! Circuit derivation of the fixed first-release transition-statement SHA transcript.
//!
//! Every body byte comes from assigned state/Guard cells. The one typed asset UUID is
//! SHA-bound to the assigned normalized asset identity through its exact canonical frame.

use core::ops::Range;

use halo2_base::{
    AssignedValue, Context,
    gates::{RangeChip, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::kagemusha_canonical_mint_frame_prefix_v1;

use super::{KagemushaAssignedStateRelationV1, KagemushaStateRelationWitnessV1};
use crate::zk::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    kagemusha_v1_state::KAGEMUSHA_TRANSITION_STATEMENT_BODY_BYTES_V1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};

const DOMAIN: &[u8] = b"iroha:kagemusha:v1:transition-statement\0";
const ASSET_DOMAIN: &[u8] = b"iroha:kagemusha:v1:asset-identity";
const ASSET_FRAME_BYTES: usize = 72;

fn uint_le<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
    bits: usize,
) -> Vec<PastaSha256ByteV1<F>> {
    range.range_check(ctx, value, bits);
    PastaSha256BitV1::decompose(ctx, range.gate(), value, bits)
        .chunks_exact(8)
        .map(|part| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), part))
        .collect()
}

fn digest_le<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: [AssignedValue<F>; 2],
) -> Vec<PastaSha256ByteV1<F>> {
    value
        .into_iter()
        .flat_map(|limb| uint_le(ctx, range, limb, 128))
        .collect()
}

fn bind_typed_asset_identity<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    witness: &KagemushaStateRelationWitnessV1,
    expected: [AssignedValue<F>; 2],
) -> Result<(), String> {
    let predecessor = witness
        .predecessor
        .as_ref()
        .ok_or_else(|| "signed transition has no predecessor asset".to_owned())?;
    let asset = &predecessor.lane.asset;
    let uuid = super::super::guard_bundle::assign_bytes(ctx, range, asset.aid_bytes().as_ref());
    if uuid.len() != 16 {
        return Err("typed asset UUID width changed".to_owned());
    }
    let prefix = kagemusha_canonical_mint_frame_prefix_v1(asset)
        .map_err(|error| format!("invalid canonical asset frame: {error}"))?;
    if prefix.payload_offset() != 40 {
        return Err("canonical asset payload offset changed".to_owned());
    }
    let mut layout = prefix.bytes().to_vec();
    for (slot, byte) in layout[23..31].iter_mut().zip((32_u64).to_le_bytes()) {
        *slot = Some(byte);
    }
    let mut ranges: Vec<Range<usize>> = Vec::with_capacity(16);
    let mut fields: Vec<&[PastaSha256ByteV1<F>]> = Vec::with_capacity(16);
    for (index, byte) in uuid.iter().enumerate() {
        layout.push(Some(1));
        layout.push(None);
        ranges.push((41 + 2 * index)..(42 + 2 * index));
        fields.push(core::slice::from_ref(byte));
    }
    let frame = super::super::canonical_preimage::assemble_canonical_preimage_v1(
        ctx, range, &layout, &ranges, &fields,
    )?;
    if frame.len() != ASSET_FRAME_BYTES {
        return Err("canonical asset frame width changed".to_owned());
    }
    let mut message = super::super::guard_bundle::constant_bytes(ASSET_DOMAIN);
    message.push(PastaSha256ByteV1::constant(0));
    message.extend(super::super::guard_bundle::constant_bytes(
        &(ASSET_FRAME_BYTES as u64).to_le_bytes(),
    ));
    message.extend(frame);
    let digest = super::super::guard_bundle::hash(ctx, jobs, message)?;
    for (actual, committed) in super::super::guard_bundle::digest_limbs_assigned(ctx, &digest)
        .into_iter()
        .zip(expected)
    {
        ctx.constrain_equal(&actual, &committed);
    }
    Ok(())
}

/// Compute the exact native flat transition digest from constrained state cells.
///
/// The witness supplies only a typed asset UUID for a canonical-frame hash preimage;
/// its digest is constrained equal to the state asset identity. No statement field is
/// copied directly from an unconstrained host witness into the final SHA transcript.
#[allow(clippy::too_many_lines)]
pub(in super::super) fn constrain_transition_statement_digest_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state: &KagemushaAssignedStateRelationV1<F>,
    witness: &KagemushaStateRelationWitnessV1,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if witness.operation == super::super::KagemushaOperationV1::Bootstrap {
        return Err("bootstrap has no signed transition statement".to_owned());
    }
    if DOMAIN.len() != 40 {
        return Err("transition statement domain width changed".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    bind_typed_asset_identity(ctx, &range, jobs, witness, state.predecessor.asset_id)?;
    let before = &state.predecessor;
    let after = &state.successor;
    let mut body = Vec::with_capacity(KAGEMUSHA_TRANSITION_STATEMENT_BODY_BYTES_V1);
    body.extend(super::super::guard_bundle::constant_bytes(
        &1_u16.to_le_bytes(),
    ));
    macro_rules! put_uint {
        ($cell:expr, $bits:expr) => {
            body.extend(uint_le(ctx, &range, $cell, $bits));
        };
    }
    macro_rules! put_digest {
        ($cells:expr) => {
            body.extend(digest_le(ctx, &range, $cells));
        };
    }
    put_uint!(before.protocol_version, 16);
    put_digest!(before.suite_id);
    put_digest!(before.vk_digest);
    put_digest!(after.suite_id);
    put_digest!(after.vk_digest);
    put_uint!(state.operation, 8);
    put_uint!(state.amount, 128);
    put_digest!(state.mint_finality_semantic_digest);
    put_digest!(state.mint_finality_proof_binding_digest);
    put_digest!(state.peer_credit_id);
    put_digest!(state.recipient_encryption_key_binding);
    put_digest!(state.lifecycle_binding_digest);
    put_digest!(state.prepared_transition_binding_digest);
    put_digest!(state.receive_credit_binding_digest);
    put_digest!(before.release_id);
    put_digest!(after.release_id);
    put_digest!(before.asset_incarnation);
    put_digest!(before.liability_pool_id);
    put_digest!(before.hardware_profile_id);
    put_uint!(before.policy_epoch, 64);
    put_digest!(before.network_id);
    put_digest!(before.lane_id);
    put_digest!(before.asset_id);
    put_uint!(before.scale, 32);
    put_digest!(state.predecessor_outer);
    put_digest!(state.successor_outer);
    put_uint!(before.sequence, 128);
    put_uint!(after.sequence, 128);
    put_uint!(before.epoch_generation, 128);
    put_digest!(before.epoch_id);
    put_uint!(after.epoch_generation, 128);
    put_digest!(after.epoch_id);
    put_digest!(before.key_reference);
    put_digest!(before.policy_id);
    put_digest!(after.key_reference);
    put_digest!(after.policy_id);
    put_digest!(before.nonce);
    put_digest!(after.nonce);
    put_uint!(state.journal_revision_before, 128);
    put_uint!(state.journal_revision_after, 128);
    put_digest!(state.transition_effect_digest);
    if body.len() != KAGEMUSHA_TRANSITION_STATEMENT_BODY_BYTES_V1 {
        return Err("transition statement body width changed".to_owned());
    }
    let mut message =
        super::super::guard_bundle::constant_bytes(&(DOMAIN.len() as u64).to_be_bytes());
    message.extend(super::super::guard_bundle::constant_bytes(DOMAIN));
    message.extend(super::super::guard_bundle::constant_bytes(
        &(body.len() as u64).to_be_bytes(),
    ));
    message.extend(body);
    super::super::guard_bundle::hash(ctx, jobs, message)
}
