#[rustfmt::skip]
use super::super::super::super::{PERSISTENT_COMMITMENT_CHUNKS_V1, PersistentDirectRelationUseSelectorV1, PersistentDirectRelationV1, VerifiedPersistentWitnessDirectRelationUseV1, persistent_commitment_set_digest, persistent_direct_relation_use_digest};
#[rustfmt::skip]
use super::super::{DirectPolynomialObjectV1, DirectRelationPublicObjectsV1, RkgH0ObjectRoleV1, RkgH1ObjectRoleV1};
use super::*;
#[rustfmt::skip]
use crate::vega::{
    MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1, VegaT256PointV1,
    derive_t256_generators_v1,
    sponge::{Keccak256, keccak256},
    zk_ams::mkhe::{ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1, active::{ZkAmsMkheActivePartySecretV1, ZkAmsMkheGovernedActiveRosterV1}, direct_collective_eval_ceremony::{DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1, ZkAmsMkheDirectCeremonyContextV1, ZkAmsMkheDirectCeremonyRoundV1, ZkAmsMkheDirectEvaluatedKeyTargetV1, ZkAmsMkheDirectPolynomialRoleV1, direct_relation_contribution_statement_from_polynomials_v1}, direct_object_transport::{ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheDirectObjectReadAtProviderV1}, manifest::{RELEASE_MODULI_V1, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1}},
};

const ZERO_POLYNOMIAL_BLAKE3_V1: [u8; 32] = [
    0xa0, 0xbd, 0x92, 0x4f, 0xd5, 0x6e, 0x8c, 0xf2, 0x26, 0x7c, 0x52, 0xbc, 0xe9, 0x67, 0x3d, 0x81,
    0x2d, 0x59, 0x44, 0x22, 0x24, 0x02, 0x8f, 0x9e, 0xd0, 0xaf, 0xea, 0x98, 0x18, 0x1d, 0x57, 0xfa,
];

struct DeterministicRandomV1(u64);
impl MaskedRelaxedRandomSourceV1 for DeterministicRandomV1 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        for chunk in destination.chunks_mut(32) {
            let block = keccak256(&self.0.to_be_bytes());
            chunk.copy_from_slice(&block[..chunk.len()]);
            self.0 = self.0.wrapping_add(1);
        }
        Ok(())
    }
}

struct ProceduralZeroProviderV1 {
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    snapshot_calls: usize,
    snapshot_drift_at: Option<usize>,
    wrong_object_len: bool,
    short_read_at: Option<usize>,
    over_read_at: Option<usize>,
    panic_read_at: Option<usize>,
    corrupt_read_at: Option<usize>,
    invalid_residue_at: Option<usize>,
    read_calls: usize,
    max_read_bytes: usize,
    next_offsets: [u64; 2],
}
impl ProceduralZeroProviderV1 {
    fn canonical() -> Self {
        Self {
            provider_identity: [0x91; 32],
            snapshot_identity: [0x92; 32],
            snapshot_calls: 0,
            snapshot_drift_at: None,
            wrong_object_len: false,
            short_read_at: None,
            over_read_at: None,
            panic_read_at: None,
            corrupt_read_at: None,
            invalid_residue_at: None,
            read_calls: 0,
            max_read_bytes: 0,
            next_offsets: [0; 2],
        }
    }
}
#[rustfmt::skip]
impl ZkAmsMkheDirectObjectReadAtProviderV1 for ProceduralZeroProviderV1 {
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> { Ok(self.provider_identity) }
    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let call = self.snapshot_calls; self.snapshot_calls += 1;
        Ok(if self.snapshot_drift_at == Some(call) { [0x93; 32] } else { self.snapshot_identity })
    }
    fn object_len(&mut self, pointer: ZkAmsMkheDirectObjectPointerV1) -> Result<u64, ZkAmsMkheErrorV1> {
        Ok(pointer.payload_bytes() - if self.wrong_object_len { 1 } else { 0 })
    }
    fn read_at(&mut self, pointer: ZkAmsMkheDirectObjectPointerV1, absolute_offset: u64, destination: &mut [u8]) -> Result<usize, ZkAmsMkheErrorV1> {
        let slot = match pointer.kind() {
            ZkAmsMkheDirectObjectKindV1::RkgH0 => 0, ZkAmsMkheDirectObjectKindV1::RkgH1 => 1,
            _ => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        };
        if absolute_offset != self.next_offsets[slot] { return Err(ZkAmsMkheErrorV1::InvalidWireEncoding); }
        let call = self.read_calls; self.read_calls += 1;
        self.max_read_bytes = self.max_read_bytes.max(destination.len());
        if self.panic_read_at == Some(call) { panic!("procedural provider panic"); }
        destination.fill(0);
        if self.invalid_residue_at == Some(call) {
            let limb_bytes = (ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * 8) as u64;
            let limb = usize::try_from(absolute_offset / limb_bytes).unwrap();
            destination[..8].copy_from_slice(&RELEASE_MODULI_V1[limb].to_be_bytes());
        }
        if self.corrupt_read_at == Some(call) { destination[7] = 1; }
        if self.short_read_at == Some(call) { return Ok(destination.len() - 1); }
        if self.over_read_at == Some(call) { return Ok(destination.len() + 1); }
        self.next_offsets[slot] += destination.len() as u64;
        Ok(destination.len())
    }
}

struct ReplayFixtureV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    capability: VerifiedPersistentWitnessDirectRelationUseV1,
    objects: DirectRelationPublicObjectsV1,
    provider: ProceduralZeroProviderV1,
}
impl ReplayFixtureV1 {
    fn begin(&mut self) -> Result<DirectRkgOneH0H1StatementReplayV1, ZkAmsMkheErrorV1> {
        DirectRkgOneH0H1StatementReplayV1::begin(
            self.context,
            &self.capability,
            self.objects,
            &mut self.provider,
        )
    }
}

fn governed_roster() -> ZkAmsMkheGovernedActiveRosterV1 {
    let mut random = DeterministicRandomV1(1);
    let mut secrets: [ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        core::array::from_fn(|_| ZkAmsMkheActivePartySecretV1::generate(&mut random).unwrap());
    secrets.sort_by_key(|secret| secret.party().unwrap());
    let references = core::array::from_fn(|index| &secrets[index]);
    ZkAmsMkheGovernedActiveRosterV1::new(77, references, &mut random).unwrap()
}

fn manual_zero_polynomial_digest(
    context: ZkAmsMkheDirectCeremonyContextV1,
    party: ZkAmsMkhePartyIdV1,
    role: ZkAmsMkheDirectPolynomialRoleV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1);
    hash.update(&context.digest());
    hash.update(&[
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne as u8,
        role as u8,
    ]);
    hash.update(&[0]);
    hash.update(&party.to_bytes());
    hash.update(&(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32).to_be_bytes());
    hash.update(&[RELEASE_MODULI_V1.len() as u8]);
    let zeroes = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    for (limb, modulus) in RELEASE_MODULI_V1.iter().enumerate() {
        hash.update(&[limb as u8]);
        hash.update(&modulus.to_be_bytes());
        for _ in 0..READ_CALLS_PER_LIMB_V1 {
            hash.update(&zeroes);
        }
    }
    hash.finalize()
}

fn pointer(kind: ZkAmsMkheDirectObjectKindV1) -> ZkAmsMkheDirectObjectPointerV1 {
    ZkAmsMkheDirectObjectPointerV1::new(
        kind,
        EXACT_POLYNOMIAL_BYTES_V1 as u64,
        ZERO_POLYNOMIAL_BLAKE3_V1,
    )
    .unwrap()
}

fn commitments(label: &[u8]) -> [VegaT256PointV1; PERSISTENT_COMMITMENT_CHUNKS_V1] {
    derive_t256_generators_v1(label, PERSISTENT_COMMITMENT_CHUNKS_V1)
        .unwrap()
        .try_into()
        .unwrap()
}

#[rustfmt::skip]
fn fixture(statement_digest_tweak: bool) -> ReplayFixtureV1 {
    let roster = governed_roster();
    let lineages = core::array::from_fn(|index| keccak256(&[0x51, index as u8]));
    let context = ZkAmsMkheDirectCeremonyContextV1::new(&roster, [0x61; 32], [0x62; 32], lineages, ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization, 3).unwrap();
    let party = roster.participants()[0].party();
    let h0_digest = manual_zero_polynomial_digest(context, party, ZkAmsMkheDirectPolynomialRoleV1::RkgH0);
    let mut h1_digest = manual_zero_polynomial_digest(context, party, ZkAmsMkheDirectPolynomialRoleV1::RkgH1);
    if statement_digest_tweak { h1_digest[0] ^= 1; }
    let objects = DirectRelationPublicObjectsV1::RkgRoundOne {
        h0: DirectPolynomialObjectV1::<RkgH0ObjectRoleV1>::new(h0_digest, pointer(ZkAmsMkheDirectObjectKindV1::RkgH0)).unwrap(),
        h1: DirectPolynomialObjectV1::<RkgH1ObjectRoleV1>::new(h1_digest, pointer(ZkAmsMkheDirectObjectKindV1::RkgH1)).unwrap(),
    };
    let prior = [0x63; 32];
    let contribution = direct_relation_contribution_statement_from_polynomials_v1(context, ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne, prior, 0, party, &[h0_digest, h1_digest]).unwrap();
    let selector = PersistentDirectRelationUseSelectorV1::new(PersistentDirectRelationV1::RkgRoundOne, context.digest(), prior, 0, context.digit_index(), 0, [0x64; 32], [0; 32], [0; 32], [0; 32], contribution, [0x65; 32]).unwrap();
    let secret_basis = [0x66; 32];
    let secret_commitments = commitments(b"rkg-one-replay-secret");
    let ephemeral_commitments = commitments(b"rkg-one-replay-ephemeral");
    let mut capability = VerifiedPersistentWitnessDirectRelationUseV1 {
        binding_set_root: context.secret_lineage_root(),
        collective_public_key_digest: context.collective_public_key_digest(),
        party_index: 0,
        party,
        secret_identity_digest: lineages[0],
        secret_generator_basis_digest: secret_basis,
        secret_commitment_set_digest: persistent_commitment_set_digest(secret_basis, &secret_commitments).unwrap(),
        secret_commitments,
        ephemeral_identity_digest: [0x68; 32],
        ephemeral_commitment_set_digest: persistent_commitment_set_digest(secret_basis, &ephemeral_commitments).unwrap(),
        ephemeral_source_context_digest: context.digest(),
        ephemeral_source_statement_digest: [0x69; 32],
        ephemeral_record_index: 1,
        ephemeral_commitments: Some(ephemeral_commitments),
        selector,
        use_digest: [0; 32],
    };
    capability.use_digest = persistent_direct_relation_use_digest(&capability).unwrap();
    capability.validate().unwrap();
    ReplayFixtureV1 { context, capability, objects, provider: ProceduralZeroProviderV1::canonical() }
}

macro_rules! assert_error {
    ($result:expr, $expected:expr) => {
        match $result {
            Ok(_) => panic!("expected failure"),
            Err(error) => assert_eq!(error, $expected),
        }
    };
}
macro_rules! assert_first_replay_error {
    ($fixture:expr, $expected:expr) => {{
        let mut replay = $fixture.begin().unwrap();
        let mut h0 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
        let mut h1 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
        assert_eq!(
            replay.replay_next_limb_pair_into(&mut $fixture.provider, &mut h0, &mut h1),
            Err($expected)
        );
    }};
}

fn replay_all(fixture: &mut ReplayFixtureV1) -> Result<(), ZkAmsMkheErrorV1> {
    let mut replay = fixture.begin()?;
    let mut h0 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let mut h1 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    for _ in 0..RELEASE_RNS_LIMBS_V1 {
        replay.replay_next_limb_pair_into(&mut fixture.provider, &mut h0, &mut h1)?;
    }
    replay.finish(&mut fixture.provider)?;
    Ok(())
}

#[test]
fn full_zero_kat_is_role_separated_and_exactly_accounted() {
    let mut blake3 = norito::streaming::Blake3Hasher::new();
    let zeroes = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    for _ in 0..READ_CALLS_PER_OBJECT_V1 {
        blake3.update(&zeroes);
    }
    assert_eq!(blake3.finalize(), ZERO_POLYNOMIAL_BLAKE3_V1);
    let mut fixture = fixture(false);
    let DirectRelationPublicObjectsV1::RkgRoundOne { h0, h1 } = fixture.objects else {
        unreachable!()
    };
    assert_ne!(h0.statement_digest, h1.statement_digest);
    replay_all(&mut fixture).unwrap();
    assert_eq!(fixture.provider.read_calls, READ_CALLS_PER_PAIR_V1);
    assert_eq!(fixture.provider.max_read_bytes, 8_192);
    assert_eq!(fixture.provider.next_offsets, [39_845_888; 2]);
    assert_eq!(EXACT_PAIR_BYTES_V1, 79_691_776);
    assert_eq!(INTERLEAVED_COMMON_A_WORKSPACE_BYTES_V1, 3_153_920);
}

#[test]
fn sealed_relation_context_and_object_splices_fail_at_begin() {
    let mut object_splice = fixture(false);
    let DirectRelationPublicObjectsV1::RkgRoundOne { mut h0, h1 } = object_splice.objects else {
        unreachable!()
    };
    h0.statement_digest[0] ^= 1;
    object_splice.objects = DirectRelationPublicObjectsV1::RkgRoundOne { h0, h1 };
    assert_error!(
        DirectRkgOneH0H1StatementReplayV1::begin(
            object_splice.context,
            &object_splice.capability,
            object_splice.objects,
            &mut object_splice.provider,
        ),
        ZkAmsMkheErrorV1::InvalidKeyMaterial
    );

    let mut context_splice = fixture(false);
    context_splice.capability.selector.context_digest[0] ^= 1;
    context_splice.capability.use_digest =
        persistent_direct_relation_use_digest(&context_splice.capability).unwrap();
    assert_error!(
        DirectRkgOneH0H1StatementReplayV1::begin(
            context_splice.context,
            &context_splice.capability,
            context_splice.objects,
            &mut context_splice.provider,
        ),
        ZkAmsMkheErrorV1::InvalidKeyMaterial
    );

    let mut relation_splice = fixture(false);
    relation_splice.capability.selector.relation = PersistentDirectRelationV1::RkgRoundTwo;
    assert_error!(
        relation_splice.begin(),
        ZkAmsMkheErrorV1::InvalidKeyMaterial
    );
}

#[test]
fn workspace_residue_partial_and_extra_states_are_poisoned() {
    let mut width = fixture(false);
    let mut replay = width.begin().unwrap();
    let mut short = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 1];
    let mut h0 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let mut h1 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    assert_eq!(
        replay.replay_next_limb_pair_into(&mut width.provider, &mut short, &mut h1),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert_eq!(
        replay.replay_next_limb_pair_into(&mut width.provider, &mut h0, &mut h1),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );

    let mut residue = fixture(false);
    residue.provider.invalid_residue_at = Some(0);
    assert_first_replay_error!(residue, ZkAmsMkheErrorV1::InvalidPolynomial);

    let mut partial = fixture(false);
    let replay = partial.begin().unwrap();
    assert_error!(
        replay.finish(&mut partial.provider),
        ZkAmsMkheErrorV1::InvalidPolynomial
    );

    let mut extra = fixture(false);
    let mut replay = extra.begin().unwrap();
    for _ in 0..RELEASE_RNS_LIMBS_V1 {
        replay
            .replay_next_limb_pair_into(&mut extra.provider, &mut h0, &mut h1)
            .unwrap();
    }
    assert_eq!(
        replay.replay_next_limb_pair_into(&mut extra.provider, &mut h0, &mut h1),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
}

#[test]
fn provider_length_read_and_snapshot_failures_are_exact() {
    for failure in 0..5 {
        let mut fixture = fixture(false);
        match failure {
            0 => fixture.provider.wrong_object_len = true,
            1 => fixture.provider.short_read_at = Some(0),
            2 => fixture.provider.over_read_at = Some(0),
            3 => fixture.provider.snapshot_drift_at = Some(2),
            _ => fixture.provider.provider_identity = [0; 32],
        }
        if matches!(failure, 0 | 4) {
            assert_error!(fixture.begin(), ZkAmsMkheErrorV1::InvalidKeyMaterial);
            continue;
        }
        let expected = if failure == 3 {
            ZkAmsMkheErrorV1::InvalidKeyMaterial
        } else {
            ZkAmsMkheErrorV1::InvalidWireEncoding
        };
        assert_first_replay_error!(fixture, expected);
    }
}

#[test]
fn statement_digest_and_blake3_are_independently_required() {
    let mut statement = fixture(true);
    assert_eq!(
        replay_all(&mut statement),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    let mut content = fixture(false);
    content.provider.corrupt_read_at = Some(0);
    assert_eq!(
        replay_all(&mut content),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    );
}

#[test]
fn caught_provider_panic_cannot_resume_the_pair() {
    let mut fixture = fixture(false);
    fixture.provider.panic_read_at = Some(0);
    let mut replay = fixture.begin().unwrap();
    let mut h0 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let mut h1 = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = replay.replay_next_limb_pair_into(&mut fixture.provider, &mut h0, &mut h1);
    }));
    assert!(caught.is_err());
    assert_eq!(
        replay.replay_next_limb_pair_into(&mut fixture.provider, &mut h0, &mut h1),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
}

#[test]
fn replay_surface_is_opaque_allocation_bounded_and_fail_closed() {
    let source = include_str!("rkg_one_h0_h1_replay_v1.rs");
    for forbidden in "Vec<|Vec::|derive(Clone|derive(Copy|impl Clone|impl Copy|Deref|AsRef|callback|decode|pub fn pointer|pub fn digest|pub fn bytes".split('|') {
        assert!(
            !source.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
    assert!(source.contains("[0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1]"));
    assert!(source.contains("self.poisoned = true;"));
    let active = include_str!("../../../active_exact_binding.rs");
    assert!(active.contains("Err(ZkAmsMkheErrorV1::ReleaseUnavailable)"));
    assert!(active.contains("let canonical_complete_wire_certified = false;"));
}
