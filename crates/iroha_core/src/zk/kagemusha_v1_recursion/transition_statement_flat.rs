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

/// State-cell source of the canonical transition digest and exact journal revision.
///
/// Only `constrain_transition_statement_source_v1` constructs this from the SHA transcript.
/// The recursive State column exports the digest for non-bootstrap operations. A terminal
/// still needs an authenticated prepared-intent opening before it can grant outgoing authority.
#[derive(Clone, Copy)]
pub(in super::super) struct KagemushaDerivedTransitionStatementSourceV1<F: KagemushaPoseidonFieldV1>
{
    digest: [PastaSha256ByteV1<F>; 32],
    journal_revision_after: AssignedValue<F>,
}

impl<F: KagemushaPoseidonFieldV1> KagemushaDerivedTransitionStatementSourceV1<F> {
    /// Bind a proposed journal opening to the SHA-derived digest and State revision cells.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "verified prepared-intent proof is not installed")
    )]
    pub(in super::super) fn constrain_journal_opening_v1(
        &self,
        ctx: &mut Context<F>,
        digest: [AssignedValue<F>; 2],
        journal_revision_after: AssignedValue<F>,
    ) {
        let derived = super::super::guard_bundle::digest_limbs_assigned(ctx, &self.digest);
        for (derived, claimed) in derived.into_iter().zip(digest) {
            ctx.constrain_equal(&derived, &claimed);
        }
        ctx.constrain_equal(&self.journal_revision_after, &journal_revision_after);
    }
}

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
    // Use the successor's typed asset for every operation so Bootstrap builds the
    // same SHA circuit shape. The State relation constrains predecessor and
    // successor asset identity to remain equal on non-bootstrap transitions.
    let asset = &witness.successor.lane.asset;
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
/// Bootstrap also computes a dummy transcript to keep the fixed circuit shape; callers
/// must select zero for Bootstrap because it has no signed transition statement.
pub(in super::super) fn constrain_transition_statement_digest_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state: &KagemushaAssignedStateRelationV1<F>,
    witness: &KagemushaStateRelationWitnessV1,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    Ok(constrain_transition_statement_source_v1(builder, jobs, state, witness)?.digest)
}

/// Export the canonical transition SHA result without reassigning host digest bytes.
#[allow(clippy::too_many_lines)]
pub(in super::super) fn constrain_transition_statement_source_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state: &KagemushaAssignedStateRelationV1<F>,
    witness: &KagemushaStateRelationWitnessV1,
) -> Result<KagemushaDerivedTransitionStatementSourceV1<F>, String> {
    if DOMAIN.len() != 40 {
        return Err("transition statement domain width changed".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    bind_typed_asset_identity(ctx, &range, jobs, witness, state.successor.asset_id)?;
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
    let digest = super::super::guard_bundle::hash(ctx, jobs, message)?;
    Ok(KagemushaDerivedTransitionStatementSourceV1 {
        digest,
        journal_revision_after: state.journal_revision_after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_v1_recursion::guard_bundle::{assign_bytes, digest_limbs_assigned};
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    #[test]
    fn derived_transition_source_binds_journal_digest_and_revision_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>(mutation: usize) -> bool {
            const K: u32 = 10;
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(K as usize)
                .use_lookup_bits(9)
                .use_instance_columns(1);
            let range = builder.range_chip();
            let ctx = builder.main(0);
            // This isolated test checks the exported cells' equality binding. The production
            // constructor obtains `digest` only from the constrained canonical SHA job above.
            let digest: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &[0x5a; 32])
                .try_into()
                .expect("digest width");
            let revision = ctx.load_witness(F::from(7));
            let source = KagemushaDerivedTransitionStatementSourceV1 {
                digest,
                journal_revision_after: revision,
            };
            let mut claim = digest_limbs_assigned(ctx, &digest);
            if mutation == 1 {
                claim[0] = ctx.load_witness(*claim[0].value() + F::ONE);
            }
            if mutation == 2 {
                claim[1] = ctx.load_witness(*claim[1].value() + F::ONE);
            }
            let revision_claim = ctx.load_witness(F::from(7 + u64::from(mutation == 3)));
            source.constrain_journal_opening_v1(ctx, claim, revision_claim);
            builder.assigned_instances = vec![Vec::new()];
            builder.calculate_params(Some(9));
            MockProver::run(K, &builder, vec![Vec::new()])
                .expect("transition source identity circuit")
                .verify()
                .is_ok()
        }

        for mutation in 0..=3 {
            for pass in [check::<Fp>(mutation), check::<Fq>(mutation)] {
                assert_eq!(pass, mutation == 0, "mutation {mutation}");
            }
        }
    }
}
