//! One-shot role-filtered replay permits for retained quadratic plane openings.

use crate::vega::sponge::Keccak256;

use super::{
    GlobalLookupPlaneRoleV1, PLANE_COUNT_V1, PLANE_RECORD_DOMAIN_V1, PlaneOpeningErrorV1,
    plane_coordinate_v1, require_nonzero_v1,
};

pub(super) const REPLAY_PURPOSE_COUNT_V1: usize = 6;
const REPLAY_PURPOSE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.vector-arithmetic-plane.replay-purpose\0";
const REPLAY_BINDING_LANGUAGE_V1: &[u8] =
    b"permits-are-one-shot-and-independent;each-replay-is-global-plane-order-filtered-by-authorized-role;s3-derived=(bD,bS,q3);s5-derived=(bD,beta,m,q5);s8-derived=(x,n,q8);q3/q5/q8-coefficient-IPA=matching-q-only;q_s-has-exactly-derived-O-plus-coefficient-IPA-authorization";

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlaneOpeningReplayPurposeV1 {
    Statement3DerivedLro = 1,
    Statement3CoefficientIpa = 2,
    Statement5DerivedLro = 3,
    Statement5CoefficientIpa = 4,
    Statement8DerivedLro = 5,
    Statement8CoefficientIpa = 6,
}

impl PlaneOpeningReplayPurposeV1 {
    const ALL: [Self; REPLAY_PURPOSE_COUNT_V1] = [
        Self::Statement3DerivedLro,
        Self::Statement3CoefficientIpa,
        Self::Statement5DerivedLro,
        Self::Statement5CoefficientIpa,
        Self::Statement8DerivedLro,
        Self::Statement8CoefficientIpa,
    ];

    const fn index_v1(self) -> usize {
        self as usize - 1
    }

    pub(super) const fn statement_v1(self) -> u8 {
        match self {
            Self::Statement3DerivedLro | Self::Statement3CoefficientIpa => 3,
            Self::Statement5DerivedLro | Self::Statement5CoefficientIpa => 5,
            Self::Statement8DerivedLro | Self::Statement8CoefficientIpa => 8,
        }
    }

    pub(super) const fn is_derived_lro_v1(self) -> bool {
        matches!(
            self,
            Self::Statement3DerivedLro | Self::Statement5DerivedLro | Self::Statement8DerivedLro
        )
    }

    pub(super) const fn accepts_role_v1(self, role: GlobalLookupPlaneRoleV1) -> bool {
        match self {
            Self::Statement3DerivedLro => matches!(
                role,
                GlobalLookupPlaneRoleV1::BooleanD
                    | GlobalLookupPlaneRoleV1::BooleanS
                    | GlobalLookupPlaneRoleV1::ResidualQ3
            ),
            Self::Statement3CoefficientIpa => {
                matches!(role, GlobalLookupPlaneRoleV1::ResidualQ3)
            }
            Self::Statement5DerivedLro => matches!(
                role,
                GlobalLookupPlaneRoleV1::BooleanD
                    | GlobalLookupPlaneRoleV1::ComparatorBorrow
                    | GlobalLookupPlaneRoleV1::MixedTop
                    | GlobalLookupPlaneRoleV1::ResidualQ5
            ),
            Self::Statement5CoefficientIpa => {
                matches!(role, GlobalLookupPlaneRoleV1::ResidualQ5)
            }
            Self::Statement8DerivedLro => matches!(
                role,
                GlobalLookupPlaneRoleV1::SmallSigned
                    | GlobalLookupPlaneRoleV1::SmallNegativeMagnitude
                    | GlobalLookupPlaneRoleV1::ResidualQ8
            ),
            Self::Statement8CoefficientIpa => {
                matches!(role, GlobalLookupPlaneRoleV1::ResidualQ8)
            }
        }
    }
}

pub(super) fn replay_plane_count_v1(
    purpose: PlaneOpeningReplayPurposeV1,
) -> Result<usize, PlaneOpeningErrorV1> {
    let mut count = 0_usize;
    for ordinal in 0..PLANE_COUNT_V1 {
        if purpose.accepts_role_v1(plane_coordinate_v1(ordinal)?.role) {
            count = count.checked_add(1).ok_or(PlaneOpeningErrorV1::Resource)?;
        }
    }
    Ok(count)
}

fn purpose_binding_digest_v1(
    context_digest: [u8; 32],
    mapping_digest: [u8; 32],
    purpose: PlaneOpeningReplayPurposeV1,
) -> Result<[u8; 32], PlaneOpeningErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(REPLAY_PURPOSE_DOMAIN_V1);
    hash.update(&require_nonzero_v1(context_digest)?);
    hash.update(&require_nonzero_v1(mapping_digest)?);
    hash.update(&[
        purpose as u8,
        purpose.statement_v1(),
        purpose.is_derived_lro_v1() as u8,
    ]);
    hash.update(&(replay_plane_count_v1(purpose)? as u16).to_be_bytes());
    hash.update(&(REPLAY_BINDING_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(REPLAY_BINDING_LANGUAGE_V1);
    for ordinal in 0..PLANE_COUNT_V1 {
        let coordinate = plane_coordinate_v1(ordinal)?;
        if purpose.accepts_role_v1(coordinate.role) {
            hash.update(&coordinate.ordinal.to_be_bytes());
            hash.update(&[coordinate.role as u8]);
        }
    }
    require_nonzero_v1(hash.finalize())
}

struct PlaneOpeningReplayPermitV1 {
    purpose: PlaneOpeningReplayPurposeV1,
    context_digest: [u8; 32],
    mapping_digest: [u8; 32],
    purpose_binding_digest: [u8; 32],
}

pub(super) struct PlaneOpeningReplayPermitsV1 {
    context_digest: [u8; 32],
    mapping_digest: [u8; 32],
    permits: [Option<PlaneOpeningReplayPermitV1>; REPLAY_PURPOSE_COUNT_V1],
}

impl PlaneOpeningReplayPermitsV1 {
    pub(super) fn new_v1(
        context_digest: [u8; 32],
        mapping_digest: [u8; 32],
    ) -> Result<Self, PlaneOpeningErrorV1> {
        require_nonzero_v1(context_digest)?;
        require_nonzero_v1(mapping_digest)?;
        let mut permits: [Option<PlaneOpeningReplayPermitV1>; REPLAY_PURPOSE_COUNT_V1] =
            core::array::from_fn(|_| None);
        for purpose in PlaneOpeningReplayPurposeV1::ALL {
            permits[purpose.index_v1()] = Some(PlaneOpeningReplayPermitV1 {
                purpose,
                context_digest,
                mapping_digest,
                purpose_binding_digest: purpose_binding_digest_v1(
                    context_digest,
                    mapping_digest,
                    purpose,
                )?,
            });
        }
        Ok(Self {
            context_digest,
            mapping_digest,
            permits,
        })
    }

    pub(super) fn take_cursor_v1(
        &mut self,
        purpose: PlaneOpeningReplayPurposeV1,
        expected_context_digest: [u8; 32],
        expected_mapping_digest: [u8; 32],
    ) -> Result<PlaneOpeningReplayCursorV1, PlaneOpeningErrorV1> {
        let permit = self.permits[purpose.index_v1()]
            .take()
            .ok_or(PlaneOpeningErrorV1::Replay)?;
        if self.context_digest != expected_context_digest
            || self.mapping_digest != expected_mapping_digest
            || permit.context_digest != expected_context_digest
            || permit.mapping_digest != expected_mapping_digest
        {
            return Err(PlaneOpeningErrorV1::Context);
        }
        permit.bind_v1(expected_context_digest, expected_mapping_digest)
    }

    pub(super) fn complete_v1(self) -> Result<[u8; 32], PlaneOpeningErrorV1> {
        if self.permits.iter().any(Option::is_some) {
            return Err(PlaneOpeningErrorV1::Replay);
        }
        let mut hash = Keccak256::new();
        hash.update(REPLAY_PURPOSE_DOMAIN_V1);
        hash.update(b"all-replays-consumed\0");
        hash.update(&self.context_digest);
        hash.update(&self.mapping_digest);
        require_nonzero_v1(hash.finalize())
    }
}

impl PlaneOpeningReplayPermitV1 {
    fn bind_v1(
        self,
        expected_context_digest: [u8; 32],
        expected_mapping_digest: [u8; 32],
    ) -> Result<PlaneOpeningReplayCursorV1, PlaneOpeningErrorV1> {
        let expected_binding = purpose_binding_digest_v1(
            expected_context_digest,
            expected_mapping_digest,
            self.purpose,
        )?;
        if self.context_digest != expected_context_digest
            || self.mapping_digest != expected_mapping_digest
            || self.purpose_binding_digest != expected_binding
        {
            return Err(PlaneOpeningErrorV1::Context);
        }
        Ok(PlaneOpeningReplayCursorV1 {
            purpose: self.purpose,
            context_digest: self.context_digest,
            mapping_digest: self.mapping_digest,
            purpose_binding_digest: self.purpose_binding_digest,
            next_search_ordinal: 0,
            absorbed_planes: 0,
            required_planes: replay_plane_count_v1(self.purpose)?,
            poisoned: false,
        })
    }
}

pub(super) struct PlaneOpeningReplayCursorV1 {
    purpose: PlaneOpeningReplayPurposeV1,
    context_digest: [u8; 32],
    mapping_digest: [u8; 32],
    purpose_binding_digest: [u8; 32],
    next_search_ordinal: usize,
    absorbed_planes: usize,
    required_planes: usize,
    poisoned: bool,
}

impl PlaneOpeningReplayCursorV1 {
    fn next_expected_ordinal_v1(&self) -> Result<Option<usize>, PlaneOpeningErrorV1> {
        for ordinal in self.next_search_ordinal..PLANE_COUNT_V1 {
            if self
                .purpose
                .accepts_role_v1(plane_coordinate_v1(ordinal)?.role)
            {
                return Ok(Some(ordinal));
            }
        }
        Ok(None)
    }

    pub(super) fn absorb_next_plane_v1(
        &mut self,
        ordinal: usize,
    ) -> Result<(), PlaneOpeningErrorV1> {
        if self.poisoned {
            return Err(PlaneOpeningErrorV1::Replay);
        }
        let Some(expected) = self.next_expected_ordinal_v1()? else {
            self.poisoned = true;
            return Err(PlaneOpeningErrorV1::Order);
        };
        if ordinal != expected {
            self.poisoned = true;
            return Err(PlaneOpeningErrorV1::Order);
        }
        let coordinate = plane_coordinate_v1(ordinal)?;
        if !self.purpose.accepts_role_v1(coordinate.role) {
            self.poisoned = true;
            return Err(PlaneOpeningErrorV1::Replay);
        }
        self.next_search_ordinal = ordinal
            .checked_add(1)
            .ok_or(PlaneOpeningErrorV1::Resource)?;
        self.absorbed_planes = self
            .absorbed_planes
            .checked_add(1)
            .ok_or(PlaneOpeningErrorV1::Resource)?;
        Ok(())
    }

    pub(super) fn complete_v1(self) -> Result<[u8; 32], PlaneOpeningErrorV1> {
        if self.poisoned
            || self.absorbed_planes != self.required_planes
            || self.next_expected_ordinal_v1()?.is_some()
        {
            return Err(PlaneOpeningErrorV1::Replay);
        }
        let mut hash = Keccak256::new();
        hash.update(PLANE_RECORD_DOMAIN_V1);
        hash.update(b"purpose-replay-complete\0");
        hash.update(&self.context_digest);
        hash.update(&self.mapping_digest);
        hash.update(&self.purpose_binding_digest);
        hash.update(&(self.absorbed_planes as u16).to_be_bytes());
        require_nonzero_v1(hash.finalize())
    }
}

const _: () = {
    assert!(REPLAY_PURPOSE_COUNT_V1 == 6);
};
