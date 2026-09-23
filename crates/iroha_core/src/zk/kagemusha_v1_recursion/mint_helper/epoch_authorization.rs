//! Circuit constraints for scheduling authorization independently of key generation.

use super::*;
use halo2_base::gates::GateChip;
use iroha_data_model::isi::kagemusha_v1::{
    BeaconEpochBindingV1, InstalledBeaconEpochBindingV1, KagemushaMintFinalityEpochAuthorizationV1,
};

const AUTHORIZATION_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-finality-epoch-authorization";

pub(super) struct AssignedEpochAuthorization<F: KagemushaPoseidonFieldV1> {
    pub(super) digest: [PastaSha256ByteV1<F>; 32],
    pub(super) network: Vec<PastaSha256ByteV1<F>>,
    pub(super) epoch: AssignedValue<F>,
    pub(super) first_height: AssignedValue<F>,
    pub(super) last_height: AssignedValue<F>,
    pub(super) generation: AssignedValue<F>,
    pub(super) authority_id: Vec<PastaSha256ByteV1<F>>,
    pub(super) beacon_installed: AssignedValue<F>,
    pub(super) beacon_session: Vec<PastaSha256ByteV1<F>>,
    pub(super) beacon_transcript: Vec<PastaSha256ByteV1<F>>,
    pub(super) previous: Vec<PastaSha256ByteV1<F>>,
    pub(super) genesis: AssignedValue<F>,
    pub(super) activate: AssignedValue<F>,
}

fn bytes_equal_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &GateChip<F>,
    left: &[PastaSha256ByteV1<F>],
    right: &[PastaSha256ByteV1<F>],
    enabled: AssignedValue<F>,
) {
    assert_eq!(left.len(), right.len(), "fixed authorization byte widths");
    for (left, right) in left.iter().zip(right) {
        let difference = gate.sub(ctx, left.quantum_cell(), right.quantum_cell());
        let gated = gate.mul(ctx, Existing(difference), Existing(enabled));
        gate.assert_is_const(ctx, &gated, &F::ZERO);
    }
}

fn bytes_zero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &GateChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
) -> AssignedValue<F> {
    let sum = bytes.iter().fold(ctx.load_zero(), |sum, byte| {
        gate.add(ctx, Existing(sum), byte.quantum_cell())
    });
    gate.is_zero(ctx, sum)
}

fn require_bit_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &GateChip<F>,
    actual: AssignedValue<F>,
    expected: bool,
    enabled: AssignedValue<F>,
) {
    let expected = ctx.load_constant(F::from(u64::from(expected)));
    constrain_equal_if(ctx, gate, actual, expected, enabled);
}

pub(super) fn constrain_epoch_authorization<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    sha: &mut PastaSha256JobsV1<F>,
    value: Option<&KagemushaMintFinalityEpochAuthorizationV1>,
    enabled: AssignedValue<F>,
) -> Result<AssignedEpochAuthorization<F>, String> {
    let gate = range.gate();
    let network = assign_bytes(
        ctx,
        range,
        value.map_or(&[0; 32], |v| v.network_id.as_bytes()),
    );
    let epoch = assign_uint(ctx, range, u128::from(value.map_or(0, |v| v.epoch)), 64);
    let first_height = assign_uint(
        ctx,
        range,
        u128::from(value.map_or(0, |v| v.first_height)),
        64,
    );
    let last_height = assign_uint(
        ctx,
        range,
        u128::from(value.map_or(0, |v| v.last_height)),
        64,
    );
    let generation = assign_uint(
        ctx,
        range,
        u128::from(value.map_or(0, |v| v.authority_generation)),
        64,
    );
    let authority_id = assign_bytes(ctx, range, &value.map_or([0; 32], |v| v.authority_id));
    let previous = assign_bytes(
        ctx,
        range,
        &value.map_or([0; 32], |v| v.previous_authorization_id),
    );
    let transition = assign_bytes(ctx, range, &value.map_or([0; 32], |v| v.transition_id));
    let decision = assign_uint(
        ctx,
        range,
        u128::from(value.map_or(0, |v| v.decision as u8)),
        2,
    );
    let genesis = gate.is_zero(ctx, decision);
    let activate = gate.is_equal(ctx, decision, Constant(F::ONE));
    let retain = gate.is_equal(ctx, decision, Constant(F::from(2)));
    let (installed, session, transcript) = match value.map(|v| v.beacon) {
        Some(BeaconEpochBindingV1::Installed(binding)) => {
            (true, binding.session_id, binding.transcript_hash)
        }
        None | Some(BeaconEpochBindingV1::Bootstrap) => (false, [0; 32], [0; 32]),
    };
    let beacon_installed = ctx.load_witness(F::from(u64::from(installed)));
    gate.assert_bit(ctx, beacon_installed);
    let beacon_session = assign_bytes(ctx, range, &session);
    let beacon_transcript = assign_bytes(ctx, range, &transcript);
    let not_genesis = gate.not(ctx, genesis);
    constrain_equal_if(ctx, gate, beacon_installed, not_genesis, enabled);
    for bytes in [&network, &authority_id] {
        let zero = bytes_zero(ctx, gate, bytes);
        require_bit_if(ctx, gate, zero, false, enabled);
    }
    for bytes in [&beacon_session, &beacon_transcript, &previous] {
        let zero = bytes_zero(ctx, gate, bytes);
        constrain_equal_if(ctx, gate, zero, genesis, enabled);
    }
    let transition_zero = bytes_zero(ctx, gate, &transition);
    let no_attempt = gate.add(ctx, Existing(genesis), Existing(retain));
    constrain_equal_if(ctx, gate, transition_zero, no_attempt, enabled);
    let epoch_zero = gate.is_zero(ctx, epoch);
    constrain_equal_if(ctx, gate, epoch_zero, genesis, enabled);
    let first_zero = gate.is_zero(ctx, first_height);
    require_bit_if(ctx, gate, first_zero, false, enabled);
    let first_one = gate.is_equal(ctx, first_height, Constant(F::ONE));
    constrain_equal_if(ctx, gate, first_one, genesis, enabled);
    let inverted = range.is_less_than(ctx, last_height, first_height, 64);
    require_bit_if(ctx, gate, inverted, false, enabled);
    let genesis_enabled = gate.mul(ctx, Existing(genesis), Existing(enabled));
    let generation_zero = gate.is_zero(ctx, generation);
    require_bit_if(ctx, gate, generation_zero, true, genesis_enabled);

    let preimage = [
        constant_bytes(AUTHORIZATION_DOMAIN),
        vec![PastaSha256ByteV1::constant(0)],
        constant_bytes(&KAGEMUSHA_CHAIN_VERSION_V1.to_le_bytes()),
        network.clone(),
        uint_bytes_le(ctx, gate, epoch, 64),
        uint_bytes_le(ctx, gate, first_height, 64),
        uint_bytes_le(ctx, gate, last_height, 64),
        uint_bytes_le(ctx, gate, generation, 64),
        authority_id.clone(),
        vec![PastaSha256ByteV1::range_checked(
            ctx,
            range,
            beacon_installed,
        )],
        beacon_session.clone(),
        beacon_transcript.clone(),
        previous.clone(),
        transition,
        uint_bytes_le(ctx, gate, decision, 8),
    ]
    .concat();
    let digest = sha_digest(ctx, sha, preimage)?;
    Ok(AssignedEpochAuthorization {
        digest,
        network,
        epoch,
        first_height,
        last_height,
        generation,
        authority_id,
        beacon_installed,
        beacon_session,
        beacon_transcript,
        previous,
        genesis,
        activate,
    })
}

pub(super) fn constrain_authorization_successor<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    current: &AssignedEpochAuthorization<F>,
    next: &AssignedEpochAuthorization<F>,
    enabled: AssignedValue<F>,
) {
    let gate = range.gate();
    require_bit_if(ctx, gate, next.genesis, false, enabled);
    bytes_equal_if(ctx, gate, &next.network, &current.network, enabled);
    bytes_equal_if(ctx, gate, &next.previous, &current.digest, enabled);
    // Both operands are range constrained to 64 bits; equality cannot wrap at u64::MAX.
    let successor_epoch = gate.add(ctx, Existing(current.epoch), Constant(F::ONE));
    constrain_equal_if(ctx, gate, next.epoch, successor_epoch, enabled);
    let successor_height = gate.add(ctx, Existing(current.last_height), Constant(F::ONE));
    constrain_equal_if(ctx, gate, next.first_height, successor_height, enabled);
    let activation = gate.mul(ctx, Existing(next.activate), Existing(enabled));
    let not_activate = gate.not(ctx, next.activate);
    let retention = gate.mul(ctx, Existing(not_activate), Existing(enabled));
    let successor_generation = gate.add(ctx, Existing(current.generation), Constant(F::ONE));
    constrain_equal_if(ctx, gate, next.generation, successor_generation, activation);
    constrain_equal_if(ctx, gate, next.generation, current.generation, retention);
    bytes_equal_if(
        ctx,
        gate,
        &next.authority_id,
        &current.authority_id,
        retention,
    );
    let mut authority_equal = ctx.load_constant(F::ONE);
    for (next, current) in next.authority_id.iter().zip(&current.authority_id) {
        let equal = gate.is_equal(ctx, next.quantum_cell(), current.quantum_cell());
        authority_equal = gate.mul(ctx, Existing(authority_equal), Existing(equal));
    }
    require_bit_if(ctx, gate, authority_equal, false, activation);
    // The first retention installs the separately authenticated genesis ceremony. Every later
    // retention preserves the exact session and transcript instead of relabeling another key.
    let installed_retention =
        gate.mul(ctx, Existing(retention), Existing(current.beacon_installed));
    bytes_equal_if(
        ctx,
        gate,
        &next.beacon_session,
        &current.beacon_session,
        installed_retention,
    );
    bytes_equal_if(
        ctx,
        gate,
        &next.beacon_transcript,
        &current.beacon_transcript,
        installed_retention,
    );
}
