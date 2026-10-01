//! The canonical bootstrap SHA is reconstructed from the same assigned successor State.
//!
//! Model serialization determines framing and byte positions. Every semantic payload byte is
//! replaced by assigned State cells and CRC64 is recomputed; no proposed host digest is admitted.

use core::ops::Range;
use halo2_base::gates::{RangeInstructions as _, circuit::builder::BaseCircuitBuilder};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, asset::AssetDefinitionId, nexus::AxtAssetIncarnationV1};

use super::super::{
    canonical_preimage::assemble_canonical_preimage_v1,
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{constant_bytes, hash},
};
use super::{KagemushaAssignedStateRelationV1, KagemushaStateRelationWitnessV1};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    kagemusha_v1_state::{BootstrapStatementV1, KAGEMUSHA_STATE_VERSION_V1},
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

const DOMAIN: &[u8] = b"iroha:kagemusha:v1:bootstrap-statement\0";
const CHECKSUM: Range<usize> = 31..39;

/// Locate raw semantic bytes by one-byte perturbations of the authoritative encoder.
/// Any length, flags, schema or multiple-payload-byte change refuses the layout.
fn bind_field<F: KagemushaPoseidonFieldV1>(
    original: &BootstrapStatementV1,
    encoded: &[u8],
    layout: &mut [Option<u8>],
    raw: &[u8],
    cells: &[PastaSha256ByteV1<F>],
    ranges: &mut Vec<Range<usize>>,
    fields: &mut Vec<Vec<PastaSha256ByteV1<F>>>,
    change: impl Fn(&mut BootstrapStatementV1, usize, u8) -> Result<(), String>,
) -> Result<(), String> {
    if raw.len() != cells.len() {
        return Err("bootstrap semantic field width differs".into());
    }
    for (index, (byte, cell)) in raw.iter().zip(cells).enumerate() {
        let mut changed = original.clone();
        // Bit one preserves the hash marker and UUID version/variant bits.
        let mut value = *byte ^ 2;
        if change(&mut changed, index, value).is_err() {
            // A valid nonzero hash can contain only this bit. Preserve the type's
            // nonzero invariant while locating the same single encoded byte.
            changed = original.clone();
            value = *byte ^ 4;
            change(&mut changed, index, value)?;
        }
        let changed = norito::encode_canonical(&changed).map_err(|e| e.to_string())?;
        if changed.len() != encoded.len() {
            return Err("bootstrap encoder field changed frame width".into());
        }
        let mut differences =
            encoded
                .iter()
                .zip(&changed)
                .enumerate()
                .filter_map(|(position, (a, b))| {
                    (a != b && !CHECKSUM.contains(&position)).then_some(position)
                });
        let position = differences
            .next()
            .ok_or("bootstrap encoder field byte absent")?;
        if differences.next().is_some()
            || position < 40
            || encoded[position] != *byte
            || changed[position] != value
            || layout[position].take().is_none()
        {
            return Err("bootstrap semantic byte overlaps framing or another field".into());
        }
        ranges.push(position..position + 1);
        fields.push(vec![*cell]);
    }
    Ok(())
}

pub(in super::super) fn constrain_bootstrap_statement_digest_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state: &KagemushaAssignedStateRelationV1<F>,
    witness: &KagemushaStateRelationWitnessV1,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let successor = &witness.successor;
    let original = BootstrapStatementV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        protocol_version: successor.protocol_version,
        suite_id: successor.suite_id,
        vk_digest: successor.vk_digest,
        release_id: successor.release_id,
        asset_incarnation: successor.asset_incarnation,
        liability_pool_id: successor.liability_pool_id,
        hardware_profile_id: successor.hardware_profile_id,
        policy_epoch: successor.policy_epoch,
        lane: successor.lane.clone(),
        hardware_epoch: successor.hardware_epoch,
        device_policy_binding: successor.device_policy_binding,
        next_one_use_key_reference: successor.next_one_use_key_reference,
        state_nonce_commitment: successor.state_nonce_commitment,
        state_commitment: successor.state_commitment,
    };
    let encoded = norito::encode_canonical(&original).map_err(|e| e.to_string())?;
    if encoded.len() < 40 || encoded.len() > 4096 {
        return Err("bootstrap canonical frame outside fixed bound".into());
    }
    let mut layout = encoded.iter().copied().map(Some).collect::<Vec<_>>();
    layout[CHECKSUM].fill(None);
    let mut ranges = Vec::new();
    let mut fields = Vec::new();
    let range = builder.range_chip();
    let g = range.gate();
    let s = &state.successor;
    macro_rules! digest {
        ($raw:expr, $assigned:expr, $target:ident) => {{
            let bytes = assigned_digest_bytes_v1(builder.main(0), g, $assigned);
            bind_field(
                &original,
                &encoded,
                &mut layout,
                &$raw,
                &bytes,
                &mut ranges,
                &mut fields,
                |o, i, byte| {
                    o.$target[i] = byte;
                    Ok(())
                },
            )?;
        }};
    }
    digest!(original.suite_id, s.suite_id, suite_id);
    digest!(original.vk_digest, s.vk_digest, vk_digest);
    digest!(original.release_id, s.release_id, release_id);
    digest!(
        original.liability_pool_id,
        s.liability_pool_id,
        liability_pool_id
    );
    digest!(
        original.hardware_profile_id,
        s.hardware_profile_id,
        hardware_profile_id
    );
    digest!(
        original.next_one_use_key_reference,
        s.next_one_use_key_reference,
        next_one_use_key_reference
    );
    digest!(
        original.state_nonce_commitment,
        s.nonce,
        state_nonce_commitment
    );
    digest!(
        original.state_commitment,
        state.successor_outer,
        state_commitment
    );
    let protocol = assigned_uint_bytes_v1(builder.main(0), g, s.protocol_version, 16);
    bind_field(
        &original,
        &encoded,
        &mut layout,
        &original.protocol_version.to_le_bytes(),
        &protocol,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            let mut raw = o.protocol_version.to_le_bytes();
            raw[i] = byte;
            o.protocol_version = u16::from_le_bytes(raw);
            Ok(())
        },
    )?;
    let policy = assigned_uint_bytes_v1(builder.main(0), g, s.policy_epoch, 64);
    bind_field(
        &original,
        &encoded,
        &mut layout,
        &original.policy_epoch.to_le_bytes(),
        &policy,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            let mut raw = o.policy_epoch.to_le_bytes();
            raw[i] = byte;
            o.policy_epoch = u64::from_le_bytes(raw);
            Ok(())
        },
    )?;
    let incarnation = assigned_digest_bytes_v1(builder.main(0), g, s.asset_incarnation);
    bind_field(
        &original,
        &encoded,
        &mut layout,
        original.asset_incarnation.as_bytes(),
        &incarnation,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            let mut raw = *o.asset_incarnation.as_bytes();
            raw[i] = byte;
            o.asset_incarnation =
                AxtAssetIncarnationV1::try_from_bytes(raw).map_err(|e| e.to_string())?;
            Ok(())
        },
    )?;
    let network = assigned_digest_bytes_v1(builder.main(0), g, s.network_id);
    bind_field(
        &original,
        &encoded,
        &mut layout,
        original.lane.network_id.as_bytes(),
        &network,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            let mut raw = *o.lane.network_id.as_bytes();
            raw[i] = byte;
            o.lane.network_id =
                NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(raw)));
            Ok(())
        },
    )?;
    let lane = assigned_digest_bytes_v1(builder.main(0), g, s.lane_id);
    bind_field(
        &original,
        &encoded,
        &mut layout,
        &original.lane.device_lane_id,
        &lane,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            o.lane.device_lane_id[i] = byte;
            Ok(())
        },
    )?;
    let asset = super::transition_statement_flat::bind_typed_asset_identity(
        builder.main(0),
        &range,
        jobs,
        witness,
        s.asset_id,
    )?;
    bind_field(
        &original,
        &encoded,
        &mut layout,
        &original.lane.asset.aid_bytes(),
        &asset,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            let mut raw = o.lane.asset.aid_bytes();
            raw[i] = byte;
            o.lane.asset = AssetDefinitionId::from_uuid_bytes(raw).map_err(|e| e.to_string())?;
            Ok(())
        },
    )?;
    let scale = assigned_uint_bytes_v1(builder.main(0), g, s.scale, 32);
    bind_field(
        &original,
        &encoded,
        &mut layout,
        &original.lane.scale.to_le_bytes(),
        &scale,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            let mut raw = o.lane.scale.to_le_bytes();
            raw[i] = byte;
            o.lane.scale = u32::from_le_bytes(raw);
            Ok(())
        },
    )?;
    let generation = assigned_uint_bytes_v1(builder.main(0), g, s.epoch_generation, 128);
    bind_field(
        &original,
        &encoded,
        &mut layout,
        &original.hardware_epoch.generation.to_le_bytes(),
        &generation,
        &mut ranges,
        &mut fields,
        |o, i, byte| {
            let mut raw = o.hardware_epoch.generation.to_le_bytes();
            raw[i] = byte;
            o.hardware_epoch.generation = u128::from_le_bytes(raw);
            Ok(())
        },
    )?;
    for (raw, assigned, field) in [
        (original.hardware_epoch.epoch_id, s.epoch_id, 0),
        (
            original.device_policy_binding.device_key_reference,
            s.key_reference,
            1,
        ),
        (
            original.device_policy_binding.hardware_policy_id,
            s.policy_id,
            2,
        ),
    ] {
        let bytes = assigned_digest_bytes_v1(builder.main(0), g, assigned);
        bind_field(
            &original,
            &encoded,
            &mut layout,
            &raw,
            &bytes,
            &mut ranges,
            &mut fields,
            |o, i, byte| {
                match field {
                    0 => o.hardware_epoch.epoch_id[i] = byte,
                    1 => o.device_policy_binding.device_key_reference[i] = byte,
                    _ => o.device_policy_binding.hardware_policy_id[i] = byte,
                }
                Ok(())
            },
        )?;
    }
    let borrowed = fields.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let frame =
        assemble_canonical_preimage_v1(builder.main(0), &range, &layout, &ranges, &borrowed)?;
    let mut preimage = constant_bytes(&(DOMAIN.len() as u64).to_be_bytes());
    preimage.extend(constant_bytes(DOMAIN));
    preimage.extend(constant_bytes(&(encoded.len() as u64).to_be_bytes()));
    preimage.extend(frame);
    hash(builder.main(0), jobs, preimage)
}
