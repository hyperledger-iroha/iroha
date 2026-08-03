//! Domain-separated governance proposal fingerprints.

use iroha_crypto::blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use norito::codec::Encode;

pub const DEPLOY_CONTRACT_V1: &[u8] = b"iroha.governance.proposal.deploy_contract.v1";
pub const RUNTIME_UPGRADE_V1: &[u8] = b"iroha.governance.proposal.runtime_upgrade.v1";
pub const SCCP_ROUTE_GOVERNANCE_V1: &[u8] = b"iroha.governance.proposal.sccp_route_governance.v1";
pub const VALIDATION_FEE_POLICY_V1: &[u8] = b"iroha.governance.proposal.validation_fee_policy.v1";
pub const VALIDATION_FEE_PAYOUT_LIFECYCLE_V1: &[u8] =
    b"iroha.governance.proposal.validation_fee_payout_lifecycle.v1";
pub(crate) const MUSUBI_REGISTRY_GOVERNANCE_V1: &[u8] =
    b"iroha.governance.proposal.musubi_registry_governance.v1";

pub fn fingerprint(domain: &[u8], proposal: &impl Encode) -> [u8; 32] {
    let encoded = proposal.encode();
    let domain_len = u64::try_from(domain.len())
        .expect("protocol-defined digest domains fit in u64")
        .to_le_bytes();
    let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length is fixed and valid");
    hasher.update(&domain_len);
    hasher.update(domain);
    hasher.update(&encoded);
    let mut fingerprint = [0_u8; 32];
    hasher
        .finalize_variable(&mut fingerprint)
        .expect("fingerprint output has the configured Blake2b length");
    fingerprint
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn proposal_domains_are_unique() {
        let domains = [
            DEPLOY_CONTRACT_V1,
            RUNTIME_UPGRADE_V1,
            SCCP_ROUTE_GOVERNANCE_V1,
            VALIDATION_FEE_POLICY_V1,
            VALIDATION_FEE_PAYOUT_LIFECYCLE_V1,
            MUSUBI_REGISTRY_GOVERNANCE_V1,
        ];
        assert_eq!(
            domains.into_iter().collect::<BTreeSet<_>>().len(),
            domains.len()
        );
    }

    #[test]
    fn fingerprint_binds_the_proposal_domain() {
        let proposal = 7_u32;
        assert_ne!(
            fingerprint(DEPLOY_CONTRACT_V1, &proposal),
            fingerprint(RUNTIME_UPGRADE_V1, &proposal)
        );
    }
}
