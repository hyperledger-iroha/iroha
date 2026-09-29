//! Exact canonical State value for installed lane compliance policy.
//!
//! Filesystem source paths and parsed engine handles are not authority. The
//! ordered policy values and audit/enforcement mode determine admission.
//! TODO: Fund policy cloning and encoding from the complete State-root resource
//! owner before this projection is used during finalized publication.

use super::{LaneComplianceEngine, LaneCompliancePolicy};
use norito::{Decode, Encode, NoritoSchema};

/// Active optional policy, distinguished from an installed empty policy set.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:lane_compliance:v1")]
pub(crate) struct LaneComplianceAuthorityV1 {
    active: bool,
    audit_only: bool,
    policies: Vec<LaneCompliancePolicy>,
}

impl LaneComplianceAuthorityV1 {
    /// Capture each policy in ascending lane order, preserving every rule.
    pub(crate) fn from_engine(engine: Option<&LaneComplianceEngine>) -> Self {
        let Some(engine) = engine else {
            return Self {
                active: false,
                audit_only: false,
                policies: Vec::new(),
            };
        };
        Self {
            active: true,
            audit_only: engine.audit_only,
            policies: engine
                .policies
                .values()
                .map(|policy| policy.as_ref().clone())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;
    use iroha_data_model::nexus::{
        AuditControls, JurisdictionSet, LaneCompliancePolicyId, LaneComplianceRule,
        ParticipantSelector,
    };
    use iroha_model_base::{
        metadata::Metadata,
        topology::{DataSpaceId, LaneId},
    };

    fn policy(lane: u32) -> LaneCompliancePolicy {
        LaneCompliancePolicy {
            id: LaneCompliancePolicyId::new(Hash::prehashed([u8::try_from(lane).unwrap(); 32])),
            version: 1,
            lane_id: LaneId::new(lane),
            dataspace_id: DataSpaceId::UNIVERSAL,
            jurisdiction: JurisdictionSet::default(),
            deny: Vec::new(),
            allow: Vec::new(),
            transfer_limits: Vec::new(),
            audit_controls: AuditControls::default(),
            metadata: Metadata::default(),
        }
    }

    fn frame(engine: Option<&LaneComplianceEngine>) -> Vec<u8> {
        norito::encode_canonical(&LaneComplianceAuthorityV1::from_engine(engine)).unwrap()
    }

    #[test]
    fn absent_and_installed_empty_policy_are_distinct_and_canonical() {
        let absent = LaneComplianceAuthorityV1::from_engine(None);
        assert_eq!(
            LaneComplianceAuthorityV1::nominal_name(),
            "iroha:state:lane_compliance:v1"
        );
        let bytes = norito::encode_canonical(&absent).unwrap();
        assert_eq!(
            norito::decode_canonical::<LaneComplianceAuthorityV1>(&bytes).unwrap(),
            absent
        );
        let installed = LaneComplianceEngine::from_policies(Vec::new(), false).unwrap();
        assert_ne!(frame(None), frame(Some(&installed)));
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(norito::encode_canonical(&absent).unwrap(), bytes);
    }

    #[test]
    fn audit_mode_and_complete_rule_values_change_policy() {
        let baseline = LaneComplianceEngine::from_policies(vec![policy(1)], false).unwrap();
        let expected = frame(Some(&baseline));
        let audit = LaneComplianceEngine::from_policies(vec![policy(1)], true).unwrap();
        assert_ne!(frame(Some(&audit)), expected);

        let mut revised = policy(1);
        revised.version += 1;
        let revised = LaneComplianceEngine::from_policies(vec![revised], false).unwrap();
        assert_ne!(frame(Some(&revised)), expected);

        let mut ruled = policy(1);
        ruled.deny.push(LaneComplianceRule {
            selector: ParticipantSelector::default(),
            reason_code: Some("policy-denial".to_owned()),
            jurisdiction_override: JurisdictionSet::default(),
        });
        let ruled = LaneComplianceEngine::from_policies(vec![ruled], false).unwrap();
        assert_ne!(frame(Some(&ruled)), expected);
    }

    #[test]
    fn policy_order_is_lanes_not_input_or_directory_order() {
        let ascending =
            LaneComplianceEngine::from_policies(vec![policy(1), policy(2)], false).unwrap();
        let descending =
            LaneComplianceEngine::from_policies(vec![policy(2), policy(1)], false).unwrap();
        assert_eq!(frame(Some(&ascending)), frame(Some(&descending)));
    }
}
