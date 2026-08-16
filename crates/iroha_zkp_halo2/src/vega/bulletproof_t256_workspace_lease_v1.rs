//! Non-reentrant process-local workspace lease for exact T256 membership.

use super::{
    Scalar, ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1,
    ZkAmsT256MembershipBoundV1, ZkAmsT256MembershipErrorV1, exact_small, membership_proof_len,
    membership_shape,
};
use crate::generalized_bulletproof::{GeneralizedBulletproofErrorV1, ProofScalar as _};
use std::{
    cell::Cell,
    sync::{Mutex, MutexGuard},
};

std::thread_local! {
    static T256_MEMBERSHIP_WORKSPACE_HELD_V1: Cell<bool> = const { Cell::new(false) };
}
pub(super) static ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum T256MembershipWorkspaceAcquireErrorV1 {
    Poisoned,
    Reentered,
}

pub(in crate::vega) struct T256MembershipWorkspaceGuardV1<'a> {
    _guard: MutexGuard<'a, ()>,
}

impl Drop for T256MembershipWorkspaceGuardV1<'_> {
    fn drop(&mut self) {
        T256_MEMBERSHIP_WORKSPACE_HELD_V1.with(|held| held.set(false));
    }
}

fn acquire_t256_membership_workspace_raw_v1(
    lease: &Mutex<()>,
) -> Result<T256MembershipWorkspaceGuardV1<'_>, T256MembershipWorkspaceAcquireErrorV1> {
    if T256_MEMBERSHIP_WORKSPACE_HELD_V1.with(Cell::get) {
        return Err(T256MembershipWorkspaceAcquireErrorV1::Reentered);
    }
    let guard = lease
        .lock()
        .map_err(|_| T256MembershipWorkspaceAcquireErrorV1::Poisoned)?;
    T256_MEMBERSHIP_WORKSPACE_HELD_V1.with(|held| held.set(true));
    Ok(T256MembershipWorkspaceGuardV1 { _guard: guard })
}

#[derive(Clone, Copy)]
pub(super) enum T256MembershipWorkspaceRoleV1 {
    Proving,
    Commitment,
    Verification,
}

pub(super) fn acquire_zk_ams_t256_membership_workspace_v1(
    lease: &Mutex<()>,
    role: T256MembershipWorkspaceRoleV1,
) -> Result<T256MembershipWorkspaceGuardV1<'_>, ZkAmsT256MembershipErrorV1> {
    acquire_t256_membership_workspace_raw_v1(lease).map_err(|error| match error {
        T256MembershipWorkspaceAcquireErrorV1::Reentered => {
            ZkAmsT256MembershipErrorV1::WorkspaceLeaseReentered
        }
        T256MembershipWorkspaceAcquireErrorV1::Poisoned => match role {
            T256MembershipWorkspaceRoleV1::Proving => {
                ZkAmsT256MembershipErrorV1::ProvingLeasePoisoned
            }
            T256MembershipWorkspaceRoleV1::Commitment => {
                ZkAmsT256MembershipErrorV1::CommitmentLeasePoisoned
            }
            T256MembershipWorkspaceRoleV1::Verification => {
                ZkAmsT256MembershipErrorV1::VerificationLeasePoisoned
            }
        },
    })
}

pub(in crate::vega) fn acquire_zk_ams_t256_cpk_workspace_v1()
-> Result<T256MembershipWorkspaceGuardV1<'static>, GeneralizedBulletproofErrorV1> {
    acquire_zk_ams_t256_membership_workspace_v1(
        &ZK_AMS_T256_MEMBERSHIP_WORKSPACE_LEASE_V1,
        T256MembershipWorkspaceRoleV1::Commitment,
    )
    .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)
}

fn first_out_of_range_coefficient_v1(
    coefficients: &[i8],
    bound: ZkAmsT256MembershipBoundV1,
) -> Option<usize> {
    coefficients
        .iter()
        .position(|coefficient| !bound.contains(*coefficient))
}

pub(super) fn preflight_zk_ams_t256_membership_opening_v1(
    coefficients: &[i8],
    bound: ZkAmsT256MembershipBoundV1,
    blinding: &Scalar,
) -> Result<(), ZkAmsT256MembershipErrorV1> {
    if coefficients.len() != ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    if blinding.is_zero() {
        return Err(ZkAmsT256MembershipErrorV1::Blinding);
    }
    if let Some(index) = first_out_of_range_coefficient_v1(coefficients, bound) {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientOutOfRange { index });
    }
    membership_shape(coefficients.len(), bound).map(|_| ())
}

pub(super) fn preflight_zk_ams_t256_membership_proving_v1(
    context_digest: [u8; 32],
    chunk_ordinal: u16,
    coefficients: &[i8],
    bound: ZkAmsT256MembershipBoundV1,
    blinding: &Scalar,
) -> Result<(), ZkAmsT256MembershipErrorV1> {
    if coefficients.len() != ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    if context_digest == [0; 32] {
        return Err(ZkAmsT256MembershipErrorV1::Context);
    }
    if chunk_ordinal > ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1 {
        return Err(ZkAmsT256MembershipErrorV1::ChunkOrdinal);
    }
    let (_, padded_gates, _) = membership_shape(coefficients.len(), bound)?;
    exact_small::ExactSmallCoefficientConstraintSourceV1::new(
        coefficients.len(),
        bound.exact_source(),
    )?;
    membership_proof_len(padded_gates)?;
    if blinding.is_zero() {
        return Err(ZkAmsT256MembershipErrorV1::Blinding);
    }
    if let Some(index) = first_out_of_range_coefficient_v1(coefficients, bound) {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientOutOfRange { index });
    }
    Ok(())
}
