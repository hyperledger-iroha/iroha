//! Additive V2 transcript contract for the sole joint radix/global lookup `z`.
//! It stops after `rho`, constructs no inverses, has an uninhabited production input, and exposes no scalar getter.

#[rustfmt::skip]
use crate::vega::{VegaT256ScalarV1 as Scalar, bulletproof_t256::ZeroizingT256ScalarCopyV1, sponge::Keccak256};
use core::convert::Infallible;

const GLOBAL_LOOKUP_VERSION_V2: u8 = 2;
const TRANSCRIPT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-lookup.transcript\0";
const CHALLENGE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-lookup.challenge\0";
const MANIFEST_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-lookup.challenge-manifest\0";
const TOPOLOGY_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-lookup.topology\0";
const BOUND_CONTEXT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.global-lookup.bound-context\0";
const FRAME_TAG_V2: u8 = 0x52;
const MAX_CHALLENGE_ATTEMPTS_V2: u8 = 128;
const LOOKUP_TABLE_VALUES_V2: u16 = 1 << 15;
const LOOKUP_DIMENSIONS_V2: usize = 29;
const PRE_Z_COMMITMENTS_V2: usize = 39_338;
const RADIX_EXISTING_INVERSES_V2: usize = 11_696;
const CROSS_FIELD_ADDED_INVERSES_V2: usize = 20_072;
const ALIASED_INVERSES_V2: usize = RADIX_EXISTING_INVERSES_V2;
const GLOBAL_CUMULATIVE_INVERSES_V2: usize = 31_768;
const Z_ORDINAL_V2: u32 = 0;
const RHO_FIRST_ORDINAL_V2: u32 = 1;
const RHO_LAST_ORDINAL_V2: u32 = 29;
const AFTER_RHO_ORDINAL_V2: u32 = 30;

const FRAME_CHALLENGE_MANIFEST_V2: &[u8] = b"challenge-manifest";
const FRAME_TOPOLOGY_V2: &[u8] = b"global-lookup-topology";
const FRAME_FIXED_AXES_V2: &[u8] = b"fixed-axes";
const FRAME_SOURCE_BINDING_V2: &[u8] = b"source-binding";
const FRAME_RADIX_PRE_Z_V2: &[u8] = b"radix-pre-z";
const FRAME_PACKING_V2: &[u8] = b"packing";
const FRAME_CROSS_FIELD_PRE_Z_V2: &[u8] = b"cross-field-pre-z";
const FRAME_QPCS_INITIAL_ROOT_V2: &[u8] = b"qpcs-initial-root";
const FRAME_PRE_Z_INVENTORY_V2: &[u8] = b"pre-z-commitment-inventory";
const FRAME_RADIX_POST_Z_V2: &[u8] = b"radix-post-z-existing-inverses";
const FRAME_CROSS_FIELD_POST_Z_V2: &[u8] = b"cross-field-post-z-added-inverses";
const FRAME_ALIAS_MAP_V2: &[u8] = b"radix-global-inverse-alias-map";
const FRAME_GLOBAL_POST_Z_V2: &[u8] = b"global-post-z-inverse-inventory";
const FRAME_CHALLENGE_PURPOSE_V2: &[u8] = b"challenge-purpose";
const FRAME_CHALLENGE_ORDINAL_V2: &[u8] = b"challenge-ordinal";
const FRAME_CHALLENGE_ATTEMPT_V2: &[u8] = b"challenge-attempt";
const FRAME_CHALLENGE_SCALAR_V2: &[u8] = b"challenge-scalar";
const NO_COORDINATE_V2: u16 = u16::MAX;

const TRANSCRIPT_FRAME_ORDER_V2: [&[u8]; 13] = [
    FRAME_CHALLENGE_MANIFEST_V2,
    FRAME_TOPOLOGY_V2,
    FRAME_FIXED_AXES_V2,
    FRAME_SOURCE_BINDING_V2,
    FRAME_RADIX_PRE_Z_V2,
    FRAME_PACKING_V2,
    FRAME_CROSS_FIELD_PRE_Z_V2,
    FRAME_QPCS_INITIAL_ROOT_V2,
    FRAME_PRE_Z_INVENTORY_V2,
    FRAME_RADIX_POST_Z_V2,
    FRAME_CROSS_FIELD_POST_Z_V2,
    FRAME_ALIAS_MAP_V2,
    FRAME_GLOBAL_POST_Z_V2,
];
const ORDINAL_LANGUAGE_V2: &[u8] =
    b"outer-ordinals:global-radix-lookup-z=0;rho[0..28]=1..29;next=30";
const RENDEZVOUS_LANGUAGE_V2: &[u8] = b"one-global-z;pre-z=39338;derive-z-outside-{0..32767};post-z-order=radix-existing-11696,cross-field-added-20072,alias-map-11696,global-cumulative-31768;rho-after-all-post-z-bindings;no-z-getter;no-second-radix-z";
const ALIAS_LANGUAGE_V2: &[u8] = b"for-i=0..5847:RadixDifferenceInverse[i]=GlobalExistingDifferenceInverse[i];for-i=0..5847:RadixSumInverse[i]=GlobalExistingSumInverse[i];same-physical-commitment-ticket-point-and-blinding;no-duplicate-wire";

const GLOBAL_LOOKUP_PROOF_VERIFIED_V2: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V2: bool = false;
const PRE_Z_CONTEXT_AUTHENTICATED_V2: bool = false;
const POST_Z_BINDINGS_VERIFIED_V2: bool = false;
const STREAMING_OWNERS_WIRED_V2: bool = false;
const TRANSCRIPT_Z_ALIAS_INSTANTIATED_V2: bool = false;
const COMPLETE_ACCOUNTING_QUALIFIED_V2: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const AUTHORITY_MINTED_V2: bool = false;
const RSS_QUALIFIED_V2: bool = false;
const RELEASE_READY_V2: bool = false;

#[rustfmt::skip]
const _: () = {
    assert!(RADIX_EXISTING_INVERSES_V2 + CROSS_FIELD_ADDED_INVERSES_V2 == GLOBAL_CUMULATIVE_INVERSES_V2);
    assert!(ALIASED_INVERSES_V2 == RADIX_EXISTING_INVERSES_V2);
    assert!(RHO_LAST_ORDINAL_V2 - RHO_FIRST_ORDINAL_V2 + 1 == LOOKUP_DIMENSIONS_V2 as u32);
    assert!(AFTER_RHO_ORDINAL_V2 == RHO_LAST_ORDINAL_V2 + 1);
    assert!(!GLOBAL_LOOKUP_PROOF_VERIFIED_V2);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V2);
    assert!(!PRE_Z_CONTEXT_AUTHENTICATED_V2 && !POST_Z_BINDINGS_VERIFIED_V2 && !STREAMING_OWNERS_WIRED_V2);
    assert!(!TRANSCRIPT_Z_ALIAS_INSTANTIATED_V2);
    assert!(!COMPLETE_ACCOUNTING_QUALIFIED_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!AUTHORITY_MINTED_V2);
    assert!(!RSS_QUALIFIED_V2);
    assert!(!RELEASE_READY_V2);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe) enum GlobalLookupChallengeErrorV2 {
    Shape,
    Order,
    ChallengeExhausted,
}

#[derive(Clone, Copy)]
pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupPreZContextV2 {
    fixed_axes_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    radix_pre_z_digest: [u8; 32],
    packing_digest: [u8; 32],
    cross_field_pre_z_digest: [u8; 32],
    qpcs_initial_root: [u8; 32],
}

#[rustfmt::skip]
impl GlobalLookupPreZContextV2 {
    pub(in crate::vega::zk_ams::mkhe) const fn new_v2(
        fixed_axes_digest: [u8; 32],
        source_binding_digest: [u8; 32],
        radix_pre_z_digest: [u8; 32],
        packing_digest: [u8; 32],
        cross_field_pre_z_digest: [u8; 32],
        qpcs_initial_root: [u8; 32],
    ) -> Self {
        Self { fixed_axes_digest, source_binding_digest, radix_pre_z_digest, packing_digest, cross_field_pre_z_digest, qpcs_initial_root }
    }
}

#[derive(Clone, Copy)]
pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupPreZInventoryV2 {
    inventory_digest: [u8; 32],
    commitments: usize,
}

#[rustfmt::skip]
impl GlobalLookupPreZInventoryV2 {
    pub(in crate::vega::zk_ams::mkhe) const fn new_v2(
        inventory_digest: [u8; 32],
        commitments: usize,
    ) -> Self {
        Self { inventory_digest, commitments }
    }
    fn validate_v2(self) -> Result<(), GlobalLookupChallengeErrorV2> {
        if self.inventory_digest == [0; 32] || self.commitments != PRE_Z_COMMITMENTS_V2 {
            return Err(GlobalLookupChallengeErrorV2::Shape);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupPostZBindingsV2 {
    radix_existing_inverse_digest: [u8; 32],
    cross_field_added_inverse_digest: [u8; 32],
    alias_map_digest: [u8; 32],
    global_cumulative_inverse_digest: [u8; 32],
    radix_existing_inverses: usize,
    cross_field_added_inverses: usize,
    aliases: usize,
    global_cumulative_inverses: usize,
}

#[rustfmt::skip]
impl GlobalLookupPostZBindingsV2 {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::vega::zk_ams::mkhe) const fn new_v2(
        radix_existing_inverse_digest: [u8; 32],
        cross_field_added_inverse_digest: [u8; 32],
        alias_map_digest: [u8; 32],
        global_cumulative_inverse_digest: [u8; 32],
        radix_existing_inverses: usize,
        cross_field_added_inverses: usize,
        aliases: usize,
        global_cumulative_inverses: usize,
    ) -> Self {
        Self { radix_existing_inverse_digest, cross_field_added_inverse_digest, alias_map_digest, global_cumulative_inverse_digest, radix_existing_inverses, cross_field_added_inverses, aliases, global_cumulative_inverses }
    }
    fn validate_v2(self) -> Result<(), GlobalLookupChallengeErrorV2> {
        if [self.radix_existing_inverse_digest, self.cross_field_added_inverse_digest, self.alias_map_digest, self.global_cumulative_inverse_digest].contains(&[0; 32])
            || (self.radix_existing_inverses, self.cross_field_added_inverses, self.aliases, self.global_cumulative_inverses)
                != (RADIX_EXISTING_INVERSES_V2, CROSS_FIELD_ADDED_INVERSES_V2, ALIASED_INVERSES_V2, GLOBAL_CUMULATIVE_INVERSES_V2)
        {
            return Err(GlobalLookupChallengeErrorV2::Shape);
        }
        Ok(())
    }
}

#[rustfmt::skip]
pub(in crate::vega::zk_ams::mkhe) enum GlobalLookupPreZInputSealV2 {
    Production { authenticated_pre_z_inventory: Infallible },
    #[cfg(test)]
    TestOnly,
}

pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupPreZTranscriptV2 {
    state: Keccak256,
    bound_context_digest: [u8; 32],
    pre_z: GlobalLookupPreZInventoryV2,
    seal: GlobalLookupPreZInputSealV2,
}

struct GlobalLookupDerivedZLiveV2 {
    state: Keccak256,
    bound_context_digest: [u8; 32],
    challenge_ordinal: u32,
    z: ZeroizingT256ScalarCopyV1,
    seal: GlobalLookupPreZInputSealV2,
}

/// Move-only authority proving that the sole V2 global/radix `z` was derived.
///
/// It has no `Clone`, `Deref`, serialization implementation, or scalar getter.
pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupDerivedZV2 {
    live: Option<GlobalLookupDerivedZLiveV2>,
}

pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupRhoBoundV2 {
    transcript_digest: [u8; 32],
    bound_context_digest: [u8; 32],
    rho: [Scalar; LOOKUP_DIMENSIONS_V2],
    _seal: GlobalLookupPreZInputSealV2,
}

#[derive(Clone, Copy)]
struct ChallengePurposeV2 {
    label: &'static [u8],
    statement: u16,
    coordinate: u16,
}
#[rustfmt::skip]
#[repr(u8)]
#[derive(Clone, Copy)]
enum ChallengePredicateV2 { OutsideLookupTable = 1, Nonzero = 2 }

#[rustfmt::skip]
impl ChallengePurposeV2 {
    const fn z_v2() -> Self {
        Self { label: b"global-radix-lookup-z", statement: NO_COORDINATE_V2, coordinate: NO_COORDINATE_V2 }
    }
    const fn rho_v2(coordinate: usize) -> Self {
        Self { label: b"lookup-rho-coordinate", statement: 15, coordinate: coordinate as u16 }
    }
}

fn absorb_frame_header_v2(
    state: &mut Keccak256,
    label: &[u8],
    payload_len: usize,
) -> Result<(), GlobalLookupChallengeErrorV2> {
    let label_len = u16::try_from(label.len()).map_err(|_| GlobalLookupChallengeErrorV2::Shape)?;
    let payload_len =
        u64::try_from(payload_len).map_err(|_| GlobalLookupChallengeErrorV2::Shape)?;
    state.update(&[FRAME_TAG_V2]);
    state.update(&label_len.to_be_bytes());
    state.update(label);
    state.update(&payload_len.to_be_bytes());
    Ok(())
}

fn absorb_frame_v2(
    state: &mut Keccak256,
    label: &[u8],
    payload: &[u8],
) -> Result<(), GlobalLookupChallengeErrorV2> {
    absorb_frame_header_v2(state, label, payload.len())?;
    state.update(payload);
    Ok(())
}

fn absorb_purpose_v2(
    state: &mut Keccak256,
    purpose: ChallengePurposeV2,
) -> Result<(), GlobalLookupChallengeErrorV2> {
    absorb_frame_header_v2(state, FRAME_CHALLENGE_PURPOSE_V2, purpose.label.len() + 4)?;
    state.update(purpose.label);
    state.update(&purpose.statement.to_be_bytes());
    state.update(&purpose.coordinate.to_be_bytes());
    Ok(())
}

fn challenge_is_outside_table_v2(challenge: Scalar) -> bool {
    let bytes = challenge.to_le_bytes();
    bytes[2..].iter().any(|byte| *byte != 0)
        || u16::from_le_bytes([bytes[0], bytes[1]]) >= LOOKUP_TABLE_VALUES_V2
}

fn derive_challenge_v2(
    state: &mut Keccak256,
    ordinal: &mut u32,
    purpose: ChallengePurposeV2,
    predicate: ChallengePredicateV2,
) -> Result<Scalar, GlobalLookupChallengeErrorV2> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V2 {
        let mut wide = [0_u8; 64];
        for branch in 0_u8..=1 {
            let mut fork = state.fork_v1();
            fork.update(CHALLENGE_DOMAIN_V2);
            fork.update(&ordinal.to_be_bytes());
            fork.update(&[attempt, branch]);
            absorb_purpose_v2(&mut fork, purpose)?;
            let start = usize::from(branch) * 32;
            wide[start..start + 32].copy_from_slice(&fork.finalize());
        }
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        let accepted = match predicate {
            ChallengePredicateV2::OutsideLookupTable => challenge_is_outside_table_v2(challenge),
            ChallengePredicateV2::Nonzero => !challenge.is_zero(),
        };
        if accepted {
            absorb_purpose_v2(state, purpose)?;
            absorb_frame_v2(state, FRAME_CHALLENGE_ORDINAL_V2, &ordinal.to_be_bytes())?;
            absorb_frame_v2(state, FRAME_CHALLENGE_ATTEMPT_V2, &[attempt])?;
            absorb_frame_v2(state, FRAME_CHALLENGE_SCALAR_V2, &challenge.to_le_bytes())?;
            *ordinal = ordinal
                .checked_add(1)
                .ok_or(GlobalLookupChallengeErrorV2::Order)?;
            return Ok(challenge);
        }
    }
    Err(GlobalLookupChallengeErrorV2::ChallengeExhausted)
}

fn require_nonzero_v2(digest: [u8; 32]) -> Result<[u8; 32], GlobalLookupChallengeErrorV2> {
    if digest == [0; 32] {
        Err(GlobalLookupChallengeErrorV2::Shape)
    } else {
        Ok(digest)
    }
}

fn context_entries_v2(context: &GlobalLookupPreZContextV2) -> [(&'static [u8], [u8; 32]); 6] {
    [
        (FRAME_FIXED_AXES_V2, context.fixed_axes_digest),
        (FRAME_SOURCE_BINDING_V2, context.source_binding_digest),
        (FRAME_RADIX_PRE_Z_V2, context.radix_pre_z_digest),
        (FRAME_PACKING_V2, context.packing_digest),
        (FRAME_CROSS_FIELD_PRE_Z_V2, context.cross_field_pre_z_digest),
        (FRAME_QPCS_INITIAL_ROOT_V2, context.qpcs_initial_root),
    ]
}

fn bound_context_digest_v2(
    context: &GlobalLookupPreZContextV2,
) -> Result<[u8; 32], GlobalLookupChallengeErrorV2> {
    let mut state = Keccak256::new();
    state.update(BOUND_CONTEXT_DOMAIN_V2);
    state.update(&[GLOBAL_LOOKUP_VERSION_V2]);
    state.update(&challenge_manifest_digest_v2());
    state.update(&global_lookup_topology_digest_v2());
    for (label, digest) in context_entries_v2(context) {
        absorb_frame_v2(&mut state, label, &require_nonzero_v2(digest)?)?;
    }
    require_nonzero_v2(state.finalize())
}

#[rustfmt::skip]
impl GlobalLookupPreZTranscriptV2 {
    pub(in crate::vega::zk_ams::mkhe) fn begin_v2(
        context: GlobalLookupPreZContextV2,
        pre_z: GlobalLookupPreZInventoryV2,
        seal: GlobalLookupPreZInputSealV2,
    ) -> Result<Self, GlobalLookupChallengeErrorV2> {
        pre_z.validate_v2()?;
        let bound_context_digest = bound_context_digest_v2(&context)?;
        let mut state = Keccak256::new();
        state.update(TRANSCRIPT_DOMAIN_V2);
        state.update(&[GLOBAL_LOOKUP_VERSION_V2]);
        absorb_frame_v2(&mut state, FRAME_CHALLENGE_MANIFEST_V2, &challenge_manifest_digest_v2())?;
        absorb_frame_v2(&mut state, FRAME_TOPOLOGY_V2, &global_lookup_topology_digest_v2())?;
        for (label, digest) in context_entries_v2(&context) {
            absorb_frame_v2(&mut state, label, &require_nonzero_v2(digest)?)?;
        }
        Ok(Self { state, bound_context_digest, pre_z, seal })
    }

    pub(in crate::vega::zk_ams::mkhe) fn derive_global_z_v2(
        mut self,
    ) -> Result<GlobalLookupDerivedZV2, GlobalLookupChallengeErrorV2> {
        absorb_frame_v2(&mut self.state, FRAME_PRE_Z_INVENTORY_V2, &self.pre_z.inventory_digest)?;
        let mut ordinal = Z_ORDINAL_V2;
        let mut z = derive_challenge_v2(&mut self.state, &mut ordinal, ChallengePurposeV2::z_v2(), ChallengePredicateV2::OutsideLookupTable)?;
        Ok(GlobalLookupDerivedZV2 { live: Some(GlobalLookupDerivedZLiveV2 { state: self.state, bound_context_digest: self.bound_context_digest, challenge_ordinal: ordinal, z: ZeroizingT256ScalarCopyV1::take(&mut z), seal: self.seal }) })
    }
}

#[rustfmt::skip]
impl GlobalLookupDerivedZV2 {
    pub(in crate::vega::zk_ams::mkhe) fn bind_post_z_and_derive_rho_v2(
        mut self,
        post_z: GlobalLookupPostZBindingsV2,
    ) -> Result<GlobalLookupRhoBoundV2, GlobalLookupChallengeErrorV2> {
        let mut live = self.live.take().ok_or(GlobalLookupChallengeErrorV2::Order)?;
        post_z.validate_v2()?;
        for (label, digest) in [
            (FRAME_RADIX_POST_Z_V2, post_z.radix_existing_inverse_digest),
            (FRAME_CROSS_FIELD_POST_Z_V2, post_z.cross_field_added_inverse_digest),
            (FRAME_ALIAS_MAP_V2, post_z.alias_map_digest),
            (FRAME_GLOBAL_POST_Z_V2, post_z.global_cumulative_inverse_digest),
        ] {
            absorb_frame_v2(&mut live.state, label, &digest)?;
        }
        if live.challenge_ordinal != RHO_FIRST_ORDINAL_V2 {
            return Err(GlobalLookupChallengeErrorV2::Order);
        }
        let mut rho = [Scalar::zero(); LOOKUP_DIMENSIONS_V2];
        for (coordinate, destination) in rho.iter_mut().enumerate() {
            *destination = derive_challenge_v2(
                &mut live.state,
                &mut live.challenge_ordinal,
                ChallengePurposeV2::rho_v2(coordinate),
                ChallengePredicateV2::Nonzero,
            )?;
        }
        if live.challenge_ordinal != AFTER_RHO_ORDINAL_V2 {
            return Err(GlobalLookupChallengeErrorV2::Order);
        }
        drop(live.z);
        Ok(GlobalLookupRhoBoundV2 { transcript_digest: live.state.finalize(), bound_context_digest: live.bound_context_digest, rho, _seal: live.seal })
    }

    #[cfg(test)]
    fn test_only_z_bytes_v2(&self) -> [u8; 32] {
        self.live.as_ref().expect("live test owner").z.as_ref().to_le_bytes()
    }
}

fn hash_manifest_frame_v2(hash: &mut Keccak256, label: &[u8]) {
    hash.update(&[0x46]);
    hash.update(&(label.len() as u16).to_be_bytes());
    hash.update(label);
}

#[rustfmt::skip]
fn hash_manifest_challenge_v2(
    hash: &mut Keccak256,
    ordinal: u32,
    purpose: ChallengePurposeV2,
    predicate: u8,
) {
    hash.update(&[0x43]);
    hash.update(&ordinal.to_be_bytes());
    hash.update(&[predicate]);
    hash.update(&(purpose.label.len() as u16).to_be_bytes());
    hash.update(purpose.label);
    hash.update(&purpose.statement.to_be_bytes());
    hash.update(&purpose.coordinate.to_be_bytes());
    hash.update(&[0x66]);
    hash_manifest_frame_v2(hash, FRAME_CHALLENGE_PURPOSE_V2);
    hash.update(&[0x61]);
    for label in [FRAME_CHALLENGE_PURPOSE_V2, FRAME_CHALLENGE_ORDINAL_V2, FRAME_CHALLENGE_ATTEMPT_V2, FRAME_CHALLENGE_SCALAR_V2] {
        hash_manifest_frame_v2(hash, label);
    }
}

#[rustfmt::skip]
pub(in crate::vega::zk_ams::mkhe) fn challenge_manifest_digest_v2() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V2);
    hash.update(&[GLOBAL_LOOKUP_VERSION_V2, FRAME_TAG_V2]);
    for value in [MAX_CHALLENGE_ATTEMPTS_V2 as u64, LOOKUP_DIMENSIONS_V2 as u64, PRE_Z_COMMITMENTS_V2 as u64, RADIX_EXISTING_INVERSES_V2 as u64, CROSS_FIELD_ADDED_INVERSES_V2 as u64, ALIASED_INVERSES_V2 as u64, GLOBAL_CUMULATIVE_INVERSES_V2 as u64, AFTER_RHO_ORDINAL_V2 as u64] {
        hash.update(&value.to_be_bytes());
    }
    for language in [ORDINAL_LANGUAGE_V2, RENDEZVOUS_LANGUAGE_V2, ALIAS_LANGUAGE_V2] {
        hash.update(&(language.len() as u64).to_be_bytes());
        hash.update(language);
    }
    hash.update(TRANSCRIPT_DOMAIN_V2);
    hash.update(CHALLENGE_DOMAIN_V2);
    for label in &TRANSCRIPT_FRAME_ORDER_V2[..9] {
        hash_manifest_frame_v2(&mut hash, label);
    }
    hash_manifest_challenge_v2(&mut hash, Z_ORDINAL_V2, ChallengePurposeV2::z_v2(), ChallengePredicateV2::OutsideLookupTable as u8);
    for label in &TRANSCRIPT_FRAME_ORDER_V2[9..] {
        hash_manifest_frame_v2(&mut hash, label);
    }
    for coordinate in 0..LOOKUP_DIMENSIONS_V2 {
        hash_manifest_challenge_v2(&mut hash, RHO_FIRST_ORDINAL_V2 + coordinate as u32, ChallengePurposeV2::rho_v2(coordinate), ChallengePredicateV2::Nonzero as u8);
    }
    hash.finalize()
}

#[rustfmt::skip]
pub(in crate::vega::zk_ams::mkhe) fn global_lookup_topology_digest_v2() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TOPOLOGY_DOMAIN_V2);
    hash.update(&[GLOBAL_LOOKUP_VERSION_V2]);
    hash.update(&super::global_lookup_topology_digest_v1());
    hash.update(&challenge_manifest_digest_v2());
    for value in [PRE_Z_COMMITMENTS_V2, RADIX_EXISTING_INVERSES_V2, CROSS_FIELD_ADDED_INVERSES_V2, ALIASED_INVERSES_V2, GLOBAL_CUMULATIVE_INVERSES_V2] {
        hash.update(&(value as u64).to_be_bytes());
    }
    hash.update(&(ALIAS_LANGUAGE_V2.len() as u64).to_be_bytes());
    hash.update(ALIAS_LANGUAGE_V2);
    hash.finalize()
}

#[cfg(test)]
#[path = "challenge_v2_tests.rs"]
mod tests;
