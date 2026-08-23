//! Additive sole-global-`z` rendezvous contract.
//!
//! This child freezes a dense physical inventory and the private typestate seam
//! which will eventually lend the one global lookup `z` to radix D/S inverse
//! and global U materializers.  Production cannot enter either transition:
//! challenge-independent roles 344..39,337 have no adopters and every future
//! owner seal is `Infallible`.  No commitment, witness, proof, receipt, or
//! authority is fabricated here.

use super::*;

const GLOBAL_Z_RENDEZVOUS_VERSION_V2: u8 = 2;
const PHYSICAL_MANIFEST_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-z.physical-manifest\0";
const ALIAS_MANIFEST_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-z.alias-manifest\0";
const RADIX_PRE_Z_BINDING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-z.radix-pre-z\0";
const GLOBAL_PRE_Z_BINDING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-z.global-pre-z\0";
const RADIX_POST_Z_BINDING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-z.radix-post-z\0";
const GLOBAL_POST_Z_BINDING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-z.global-post-z\0";
const Z_BINDING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-z.scalar-binding\0";
const ACCOUNTING_LANGUAGE_V2: &[u8] = b"known=32844686+V+F;cap=33554432;margin=709746-V-F;V=three-vector-arithmetic-proof-wire;F=new-serialized-envelope-framing";

const PRE_Z_PHYSICAL_COMMITMENTS_V2: u32 = 39_338;
const SHARED_INVERSE_COMMITMENTS_V2: u32 = 11_696;
const ADDED_INVERSE_COMMITMENTS_V2: u32 = 20_072;
const GLOBAL_INVERSE_COMMITMENTS_V2: u32 = 31_768;
const POST_Z_COMPLETE_ORDINAL_V2: u32 = 71_106;
const POST_DELTA_RESIDUAL_COMMITMENTS_V2: u32 = 3;
const DENSE_PHYSICAL_INVENTORY_V2: u32 = 71_109;
const REMOVED_DUAL_Z_COMMITMENTS_V2: u64 = 11_696;
const PROOF_WIRE_SAVING_BYTES_V2: u64 = 385_968;
const BLINDING_SAVING_BYTES_V2: u64 = 374_272;
const SEMANTIC_SAVING_BYTES_V2: u64 = 760_240;
const AUTH_TAG_SAVING_BYTES_V2: u64 = 187_136;
const FILE_SAVING_BYTES_V2: u64 = 947_376;
const WRITE_AND_SEAL_IO_SAVING_BYTES_V2: u64 = 1_894_752;
const KNOWN_UNIFIED_LOWER_BOUND_BYTES_V2: u64 = 32_844_686;
const CONDITIONAL_CAP_BYTES_V2: u64 = 33_554_432;
const PROVISIONAL_MARGIN_BYTES_V2: u64 = 709_746;
const VECTOR_PROOF_WIRE_BYTES_V2: Option<u64> = None;
const NEW_ENVELOPE_FRAMING_BYTES_V2: Option<u64> = None;
const CONDITIONAL_TOTAL_BYTES_V2: Option<u64> = None;
const CONDITIONAL_MARGIN_BYTES_V2: Option<u64> = None;

const PRE_Z_COMPLETION_INHABITED_V2: bool = false;
const POST_Z_MATERIALIZERS_INHABITED_V2: bool = false;
const PROOF_VERIFIED_V2: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V2: bool = false;
const COMPLETE_ACCOUNTING_QUALIFIED_V2: bool = false;
const AUTHORITY_MINTED_V2: bool = false;
const RSS_QUALIFIED_V2: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const RELEASE_READY_V2: bool = false;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalPhaseV2 {
    ChallengeIndependent = 1,
    JointPostZ = 2,
    PostDeltaResidual = 3,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalPurposeV2 {
    Source = 1,
    ExistingDifferenceLow = 2,
    ExistingSumLow = 3,
    ComparatorDifferenceTop = 4,
    ComparatorSumTop = 5,
    ComparatorDifferenceDigit = 6,
    ComparatorBorrow = 7,
    ComparatorMixedTop = 8,
    SmallSigned = 9,
    SmallNegativeMagnitude = 10,
    QMaskDigit = 11,
    QMaskComplementDigit = 12,
    Multiplicity = 13,
    SumcheckMask = 14,
    SharedDifferenceInverse = 15,
    SharedSumInverse = 16,
    ComparatorDifferenceInverse = 17,
    SmallSignedInverse = 18,
    SmallNegativeInverse = 19,
    QMaskDigitInverse = 20,
    QMaskComplementInverse = 21,
    ResidualQ3 = 22,
    ResidualQ5 = 23,
    ResidualQ8 = 24,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PhysicalRoleV2 {
    phase: PhysicalPhaseV2,
    purpose: PhysicalPurposeV2,
    first_ordinal: u32,
    count: u32,
}

const fn physical_role_v2(
    phase: PhysicalPhaseV2,
    purpose: PhysicalPurposeV2,
    first_ordinal: u32,
    count: u32,
) -> PhysicalRoleV2 {
    PhysicalRoleV2 {
        phase,
        purpose,
        first_ordinal,
        count,
    }
}

#[rustfmt::skip]
const PHYSICAL_ROLES_V2: [PhysicalRoleV2; 24] = [
    physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::Source, 0, 344), physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::ExistingDifferenceLow, 344, 5_848),
    physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::ExistingSumLow, 6_192, 5_848), physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::ComparatorDifferenceTop, 12_040, 344),
    physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::ComparatorSumTop, 12_384, 344), physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::ComparatorDifferenceDigit, 12_728, 5_848),
    physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::ComparatorBorrow, 18_576, 6_192), physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::ComparatorMixedTop, 24_768, 344),
    physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::SmallSigned, 25_112, 1_032), physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::SmallNegativeMagnitude, 26_144, 1_032),
    physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::QMaskDigit, 27_176, 6_080), physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::QMaskComplementDigit, 33_256, 6_080),
    physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::Multiplicity, 39_336, 1), physical_role_v2(PhysicalPhaseV2::ChallengeIndependent, PhysicalPurposeV2::SumcheckMask, 39_337, 1),
    physical_role_v2(PhysicalPhaseV2::JointPostZ, PhysicalPurposeV2::SharedDifferenceInverse, 39_338, 5_848), physical_role_v2(PhysicalPhaseV2::JointPostZ, PhysicalPurposeV2::SharedSumInverse, 45_186, 5_848),
    physical_role_v2(PhysicalPhaseV2::JointPostZ, PhysicalPurposeV2::ComparatorDifferenceInverse, 51_034, 5_848), physical_role_v2(PhysicalPhaseV2::JointPostZ, PhysicalPurposeV2::SmallSignedInverse, 56_882, 1_032),
    physical_role_v2(PhysicalPhaseV2::JointPostZ, PhysicalPurposeV2::SmallNegativeInverse, 57_914, 1_032), physical_role_v2(PhysicalPhaseV2::JointPostZ, PhysicalPurposeV2::QMaskDigitInverse, 58_946, 6_080), physical_role_v2(PhysicalPhaseV2::JointPostZ, PhysicalPurposeV2::QMaskComplementInverse, 65_026, 6_080),
    physical_role_v2(PhysicalPhaseV2::PostDeltaResidual, PhysicalPurposeV2::ResidualQ3, 71_106, 1), physical_role_v2(PhysicalPhaseV2::PostDeltaResidual, PhysicalPurposeV2::ResidualQ5, 71_107, 1), physical_role_v2(PhysicalPhaseV2::PostDeltaResidual, PhysicalPurposeV2::ResidualQ8, 71_108, 1),
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogicalInversePurposeV2 {
    RadixDifference = 1,
    GlobalDifference = 2,
    RadixSum = 3,
    GlobalSum = 4,
}

struct SharedInverseAliasV2 {
    alias_ordinal: u32,
    radix_purpose: LogicalInversePurposeV2,
    global_purpose: LogicalInversePurposeV2,
    purpose_ordinal: u32,
    physical_ordinal: u32,
}

fn shared_inverse_alias_v2(alias_ordinal: u32) -> Result<SharedInverseAliasV2, ZkAmsMkheErrorV1> {
    let (radix_purpose, global_purpose, purpose_ordinal, physical_ordinal) = match alias_ordinal {
        0..=5_847 => (
            LogicalInversePurposeV2::RadixDifference,
            LogicalInversePurposeV2::GlobalDifference,
            alias_ordinal,
            39_338 + alias_ordinal,
        ),
        5_848..=11_695 => (
            LogicalInversePurposeV2::RadixSum,
            LogicalInversePurposeV2::GlobalSum,
            alias_ordinal - 5_848,
            45_186 + alias_ordinal - 5_848,
        ),
        _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
    };
    Ok(SharedInverseAliasV2 {
        alias_ordinal,
        radix_purpose,
        global_purpose,
        purpose_ordinal,
        physical_ordinal,
    })
}

fn physical_manifest_digest_v2() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(PHYSICAL_MANIFEST_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2]);
    hash.update(&(PHYSICAL_ROLES_V2.len() as u16).to_be_bytes());
    hash.update(&DENSE_PHYSICAL_INVENTORY_V2.to_be_bytes());
    for role in PHYSICAL_ROLES_V2 {
        hash.update(&[role.phase as u8, role.purpose as u8]);
        hash.update(&role.first_ordinal.to_be_bytes());
        hash.update(&role.count.to_be_bytes());
    }
    hash.finalize()
}

fn alias_manifest_digest_v2() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(ALIAS_MANIFEST_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2]);
    hash.update(&SHARED_INVERSE_COMMITMENTS_V2.to_be_bytes());
    for ordinal in 0..SHARED_INVERSE_COMMITMENTS_V2 {
        let alias = shared_inverse_alias_v2(ordinal)?;
        hash.update(&alias.alias_ordinal.to_be_bytes());
        hash.update(&[alias.radix_purpose as u8, alias.global_purpose as u8]);
        hash.update(&alias.purpose_ordinal.to_be_bytes());
        hash.update(&alias.physical_ordinal.to_be_bytes());
    }
    Ok(hash.finalize())
}

struct RadixPreZBindingRecordV2 {
    fixed_axes_digest: [u8; 32],
    materialization_record_digest: [u8; 32],
    mapping_digest: [u8; 32],
    commitment_root: [u8; 32],
    commitment_count: u32,
    binding_digest: [u8; 32],
}

impl RadixPreZBindingRecordV2 {
    fn validate_v2(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if [
            self.fixed_axes_digest,
            self.materialization_record_digest,
            self.mapping_digest,
            self.commitment_root,
        ]
        .contains(&[0; 32])
            || self.commitment_count != 12_384
            || self.binding_digest != radix_pre_z_digest_v2(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

fn radix_pre_z_digest_v2(record: &RadixPreZBindingRecordV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(RADIX_PRE_Z_BINDING_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2]);
    for digest in [
        record.fixed_axes_digest,
        record.materialization_record_digest,
        record.mapping_digest,
        record.commitment_root,
    ] {
        hash.update(&digest);
    }
    hash.update(&record.commitment_count.to_be_bytes());
    hash.finalize()
}

struct GlobalPreZBindingRecordV2 {
    proof_session_context_digest: [u8; 32],
    inventory_root: [u8; 32],
    radix_pre_z_binding_digest: [u8; 32],
    cross_field_pre_z_binding_digest: [u8; 32],
    global_context_digest: [u8; 32],
    commitment_count: u32,
    binding_digest: [u8; 32],
}

impl GlobalPreZBindingRecordV2 {
    fn validate_v2(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if [
            self.proof_session_context_digest,
            self.inventory_root,
            self.radix_pre_z_binding_digest,
            self.cross_field_pre_z_binding_digest,
            self.global_context_digest,
        ]
        .contains(&[0; 32])
            || self.commitment_count != PRE_Z_PHYSICAL_COMMITMENTS_V2
            || self.binding_digest != global_pre_z_digest_v2(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

fn global_pre_z_digest_v2(record: &GlobalPreZBindingRecordV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(GLOBAL_PRE_Z_BINDING_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2]);
    hash.update(&physical_manifest_digest_v2());
    for digest in [
        record.proof_session_context_digest,
        record.inventory_root,
        record.radix_pre_z_binding_digest,
        record.cross_field_pre_z_binding_digest,
        record.global_context_digest,
    ] {
        hash.update(&digest);
    }
    hash.update(&record.commitment_count.to_be_bytes());
    hash.finalize()
}

enum PreZInventoryOwnerV2 {
    Production {
        complete_challenge_independent_inventory: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

enum PostZMaterializerAuthorityV2 {
    Production {
        radix_inverse_materializer: Infallible,
        global_inverse_materializer: Infallible,
    },
    #[cfg(test)]
    TestOnly { panic_after_radix: bool },
}

impl PostZMaterializerAuthorityV2 {
    fn panic_after_radix_v2(&self) -> bool {
        match self {
            Self::Production { .. } => false,
            #[cfg(test)]
            Self::TestOnly { panic_after_radix } => *panic_after_radix,
        }
    }
}

enum DerivedGlobalZSealV2 {
    Production {
        global_lookup_transcript: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

struct DerivedGlobalZOwnerV2 {
    _seal: DerivedGlobalZSealV2,
    scalar: ZeroizingT256ScalarCopyV1,
    pre_z_transcript_digest: [u8; 32],
}

impl DerivedGlobalZOwnerV2 {
    #[cfg(test)]
    fn test_only_v2(value: Scalar, digest: [u8; 32]) -> Result<Self, ZkAmsMkheErrorV1> {
        let encoded = value.to_le_bytes();
        let outside_table = encoded[2..].iter().any(|byte| *byte != 0)
            || u16::from_le_bytes([encoded[0], encoded[1]]) >= 1 << 15;
        if !outside_table || digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(Self {
            _seal: DerivedGlobalZSealV2::TestOnly,
            scalar: ZeroizingT256ScalarCopyV1::new(value),
            pre_z_transcript_digest: digest,
        })
    }
}

pub(super) struct PreZCompleteV2;
pub(super) struct PostZCompleteV2;

struct SessionLiveV2 {
    _owner: PreZInventoryOwnerV2,
    radix_pre_z: RadixPreZBindingRecordV2,
    global_pre_z: GlobalPreZBindingRecordV2,
    next_physical_ordinal: u32,
    post_z_binding_digest: Option<[u8; 32]>,
}

pub(super) struct GlobalLookupCommitmentSessionV2<State> {
    live: Option<SessionLiveV2>,
    state: PhantomData<State>,
}

struct RadixDsInverseInputsV2 {
    _seal: PostZMaterializerInputSealV2,
    shared_inverse_root: [u8; 32],
    commitment_count: u32,
}

struct AddedLookupInverseInputsV2 {
    _seal: PostZMaterializerInputSealV2,
    added_inverse_root: [u8; 32],
    global_inverse_root: [u8; 32],
    commitment_count: u32,
}

enum PostZMaterializerInputSealV2 {
    Production {
        authenticated_materialization: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

struct RadixPostZBindingRecordV2 {
    pre_z_binding_digest: [u8; 32],
    z_binding_digest: [u8; 32],
    shared_inverse_root: [u8; 32],
    alias_manifest_digest: [u8; 32],
    binding_digest: [u8; 32],
}

struct GlobalPostZBindingRecordV2 {
    pre_z_binding_digest: [u8; 32],
    radix_post_z_binding_digest: [u8; 32],
    added_inverse_root: [u8; 32],
    global_inverse_root: [u8; 32],
    alias_root: [u8; 32],
    binding_digest: [u8; 32],
}

struct GlobalZRendezvousLiveV2 {
    session: GlobalLookupCommitmentSessionV2<PreZCompleteV2>,
    derived_z: DerivedGlobalZOwnerV2,
    authority: PostZMaterializerAuthorityV2,
}

pub(super) struct GlobalZRendezvousV2 {
    live: Option<GlobalZRendezvousLiveV2>,
}

pub(super) struct GlobalPostZBoundV2 {
    session: GlobalLookupCommitmentSessionV2<PostZCompleteV2>,
    radix: RadixPostZBindingRecordV2,
    global: GlobalPostZBindingRecordV2,
}

impl GlobalLookupCommitmentSessionV2<PreZCompleteV2> {
    fn rendezvous_v2(
        mut self,
        derived_z: DerivedGlobalZOwnerV2,
        authority: PostZMaterializerAuthorityV2,
    ) -> Result<GlobalZRendezvousV2, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.radix_pre_z.validate_v2()?;
        live.global_pre_z.validate_v2()?;
        if live.global_pre_z.radix_pre_z_binding_digest != live.radix_pre_z.binding_digest {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        if live.next_physical_ordinal != PRE_Z_PHYSICAL_COMMITMENTS_V2
            || live.post_z_binding_digest.is_some()
            || derived_z.pre_z_transcript_digest != live.global_pre_z.binding_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(GlobalZRendezvousV2 {
            live: Some(GlobalZRendezvousLiveV2 {
                session: GlobalLookupCommitmentSessionV2 {
                    live: Some(live),
                    state: PhantomData,
                },
                derived_z,
                authority,
            }),
        })
    }
}

impl GlobalZRendezvousV2 {
    fn materialize_post_z_v2(
        mut self,
        radix_inputs: RadixDsInverseInputsV2,
        added_inputs: AddedLookupInverseInputsV2,
    ) -> Result<GlobalPostZBoundV2, ZkAmsMkheErrorV1> {
        self.materialize_post_z_in_place_v2(radix_inputs, added_inputs)
    }

    fn materialize_post_z_in_place_v2(
        &mut self,
        radix_inputs: RadixDsInverseInputsV2,
        added_inputs: AddedLookupInverseInputsV2,
    ) -> Result<GlobalPostZBoundV2, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if radix_inputs.shared_inverse_root == [0; 32]
            || radix_inputs.commitment_count != SHARED_INVERSE_COMMITMENTS_V2
            || added_inputs.added_inverse_root == [0; 32]
            || added_inputs.global_inverse_root == [0; 32]
            || added_inputs.commitment_count != ADDED_INVERSE_COMMITMENTS_V2
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let radix = bind_radix_post_z_v2(
            live.derived_z.scalar.as_ref(),
            &live.session,
            radix_inputs.shared_inverse_root,
        )?;
        assert!(
            !live.authority.panic_after_radix_v2(),
            "intentional joint-z unwind"
        );
        let global = bind_global_post_z_v2(
            live.derived_z.scalar.as_ref(),
            &live.session,
            &radix,
            added_inputs.added_inverse_root,
            added_inputs.global_inverse_root,
        )?;
        let mut session_live = live
            .session
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        session_live.next_physical_ordinal = POST_Z_COMPLETE_ORDINAL_V2;
        session_live.post_z_binding_digest = Some(global.binding_digest);
        let session = GlobalLookupCommitmentSessionV2 {
            live: Some(session_live),
            state: PhantomData,
        };
        Ok(GlobalPostZBoundV2 {
            session,
            radix,
            global,
        })
    }
}

fn bind_radix_post_z_v2(
    z: &Scalar,
    session: &GlobalLookupCommitmentSessionV2<PreZCompleteV2>,
    shared_inverse_root: [u8; 32],
) -> Result<RadixPostZBindingRecordV2, ZkAmsMkheErrorV1> {
    let live = session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let z_binding_digest = z_binding_digest_v2(z, live.global_pre_z.binding_digest);
    let alias_manifest_digest = alias_manifest_digest_v2()?;
    let mut record = RadixPostZBindingRecordV2 {
        pre_z_binding_digest: live.radix_pre_z.binding_digest,
        z_binding_digest,
        shared_inverse_root,
        alias_manifest_digest,
        binding_digest: [0; 32],
    };
    record.binding_digest = radix_post_z_digest_v2(&record);
    Ok(record)
}

fn bind_global_post_z_v2(
    z: &Scalar,
    session: &GlobalLookupCommitmentSessionV2<PreZCompleteV2>,
    radix: &RadixPostZBindingRecordV2,
    added_inverse_root: [u8; 32],
    global_inverse_root: [u8; 32],
) -> Result<GlobalPostZBindingRecordV2, ZkAmsMkheErrorV1> {
    let live = session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if radix.z_binding_digest != z_binding_digest_v2(z, live.global_pre_z.binding_digest) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let alias_root = shared_alias_root_v2(radix.shared_inverse_root, radix.alias_manifest_digest);
    let mut record = GlobalPostZBindingRecordV2 {
        pre_z_binding_digest: live.global_pre_z.binding_digest,
        radix_post_z_binding_digest: radix.binding_digest,
        added_inverse_root,
        global_inverse_root,
        alias_root,
        binding_digest: [0; 32],
    };
    record.binding_digest = global_post_z_digest_v2(&record);
    Ok(record)
}

fn z_binding_digest_v2(z: &Scalar, pre_z_digest: [u8; 32]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(Z_BINDING_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2]);
    hash.update(&pre_z_digest);
    hash.update(&z.to_le_bytes());
    hash.finalize()
}

fn radix_post_z_digest_v2(record: &RadixPostZBindingRecordV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(RADIX_POST_Z_BINDING_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2]);
    for digest in [
        record.pre_z_binding_digest,
        record.z_binding_digest,
        record.shared_inverse_root,
        record.alias_manifest_digest,
    ] {
        hash.update(&digest);
    }
    hash.update(&SHARED_INVERSE_COMMITMENTS_V2.to_be_bytes());
    hash.finalize()
}

fn shared_alias_root_v2(shared_root: [u8; 32], manifest: [u8; 32]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(ALIAS_MANIFEST_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2, 0x72]);
    hash.update(&manifest);
    hash.update(&shared_root);
    hash.finalize()
}

fn global_post_z_digest_v2(record: &GlobalPostZBindingRecordV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(GLOBAL_POST_Z_BINDING_DOMAIN_V2);
    hash.update(&[GLOBAL_Z_RENDEZVOUS_VERSION_V2]);
    for digest in [
        record.pre_z_binding_digest,
        record.radix_post_z_binding_digest,
        record.added_inverse_root,
        record.global_inverse_root,
        record.alias_root,
    ] {
        hash.update(&digest);
    }
    for count in [
        SHARED_INVERSE_COMMITMENTS_V2,
        ADDED_INVERSE_COMMITMENTS_V2,
        GLOBAL_INVERSE_COMMITMENTS_V2,
        POST_Z_COMPLETE_ORDINAL_V2,
        DENSE_PHYSICAL_INVENTORY_V2,
    ] {
        hash.update(&count.to_be_bytes());
    }
    hash.finalize()
}

const _: () = {
    assert!(DENSE_PHYSICAL_INVENTORY_V2 == 39_338 + 31_768 + 3);
    assert!(REMOVED_DUAL_Z_COMMITMENTS_V2 * 33 == PROOF_WIRE_SAVING_BYTES_V2);
    assert!(REMOVED_DUAL_Z_COMMITMENTS_V2 * 32 == BLINDING_SAVING_BYTES_V2);
    assert!(SEMANTIC_SAVING_BYTES_V2 == PROOF_WIRE_SAVING_BYTES_V2 + BLINDING_SAVING_BYTES_V2);
    assert!(REMOVED_DUAL_Z_COMMITMENTS_V2 * 16 == AUTH_TAG_SAVING_BYTES_V2);
    assert!(FILE_SAVING_BYTES_V2 == SEMANTIC_SAVING_BYTES_V2 + AUTH_TAG_SAVING_BYTES_V2);
    assert!(WRITE_AND_SEAL_IO_SAVING_BYTES_V2 == 2 * FILE_SAVING_BYTES_V2);
    assert!(
        CONDITIONAL_CAP_BYTES_V2 - KNOWN_UNIFIED_LOWER_BOUND_BYTES_V2
            == PROVISIONAL_MARGIN_BYTES_V2
    );
    assert!(VECTOR_PROOF_WIRE_BYTES_V2.is_none());
    assert!(NEW_ENVELOPE_FRAMING_BYTES_V2.is_none());
    assert!(CONDITIONAL_TOTAL_BYTES_V2.is_none());
    assert!(CONDITIONAL_MARGIN_BYTES_V2.is_none());
    assert!(!PRE_Z_COMPLETION_INHABITED_V2);
    assert!(!POST_Z_MATERIALIZERS_INHABITED_V2);
    assert!(!PROOF_VERIFIED_V2);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V2);
    assert!(!COMPLETE_ACCOUNTING_QUALIFIED_V2);
    assert!(!AUTHORITY_MINTED_V2);
    assert!(!RSS_QUALIFIED_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!RELEASE_READY_V2);
};

#[cfg(test)]
#[path = "global_z_rendezvous_v2_tests.rs"]
mod tests;
