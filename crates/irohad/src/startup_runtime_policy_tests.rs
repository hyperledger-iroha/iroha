//! Startup policy loading must precede replay and remain frozen through runtime handoff.
use super::*;
use iroha_data_model::nexus::{
    AuditControls, JurisdictionSet, LaneCompliancePolicy, LaneCompliancePolicyId,
};
use iroha_model_base::{
    metadata::Metadata,
    topology::{DataSpaceId, LaneId},
};

fn policy() -> LaneCompliancePolicy {
    LaneCompliancePolicy {
        id: LaneCompliancePolicyId::new(iroha_crypto::Hash::prehashed([0x51; 32])),
        version: 1,
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        jurisdiction: JurisdictionSet::default(),
        deny: Vec::new(),
        allow: Vec::new(),
        transfer_limits: Vec::new(),
        audit_controls: AuditControls::default(),
        metadata: Metadata::default(),
    }
}

#[test]
fn startup_compliance_is_installed_before_execution_policy_derivation_and_reused() {
    let directory = tempfile::tempdir().expect("policy directory");
    let path = directory.path().join("default.norito");
    std::fs::write(&path, policy().encode()).expect("write canonical policy");
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.compliance.enabled = true;
    nexus.compliance.policy_dir = Some(directory.path().to_path_buf());
    let manifests = freeze_lane_manifests_for_startup_replay(&nexus).expect("frozen manifest");
    let mut state = State::new_for_testing(
        World::new(), Kura::blank_kura_for_testing(), LiveQueryStore::start_test(),
    );
    state.set_nexus(nexus.clone()).expect("configure fixture Nexus");
    state.install_lane_manifests(&manifests);
    assert!(state.execution_policy_digest_v1().is_err(), "enabled compliance cannot derive policy before loading");
    let engine = freeze_lane_compliance_for_startup_replay(&nexus)
        .expect("load frozen compliance").expect("enabled engine");
    state.install_lane_compliance_engine(Some(Arc::clone(&engine)));
    let digest = state.execution_policy_digest_v1().expect("policy is ready before replay");
    let candidate = state.execution_policy_digest_with_runtime_policies_v1(
        &nexus, manifests.as_ref(), Some(engine.as_ref()),
    ).expect("explicit frozen candidate");
    assert_eq!(candidate, digest);
    let mut changed = policy();
    changed.version = 2;
    std::fs::write(&path, changed.encode()).expect("replace directory after freeze");
    engine.validate_active_catalog(&nexus.lane_catalog).expect("post-replay coverage uses frozen source");
    let retained = state.lane_compliance_engine().expect("same State engine");
    assert!(Arc::ptr_eq(&engine, &retained));
    assert_eq!(state.execution_policy_digest_v1().expect("retained policy"), digest);
    let rescanned = freeze_lane_compliance_for_startup_replay(&nexus)
        .expect("changed policy is independently valid").expect("enabled engine");
    assert_ne!(rescanned.consensus_policy_digest(), engine.consensus_policy_digest(),
               "a forbidden second directory load would change the frozen policy");
}

#[test]
fn startup_compliance_rejects_missing_and_wrong_lane_policy_before_replay() {
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.compliance.enabled = true;
    nexus.compliance.policy_dir = None;
    assert!(freeze_lane_compliance_for_startup_replay(&nexus).is_err());
    let directory = tempfile::tempdir().expect("policy directory");
    let mut wrong = policy();
    wrong.lane_id = LaneId::new(9);
    std::fs::write(directory.path().join("wrong.norito"), wrong.encode()).expect("write other-lane policy");
    nexus.compliance.policy_dir = Some(directory.path().to_path_buf());
    assert!(freeze_lane_compliance_for_startup_replay(&nexus).is_err());
    nexus.compliance.enabled = false;
    std::fs::remove_dir_all(directory.path()).expect("remove disabled directory");
    assert!(freeze_lane_compliance_for_startup_replay(&nexus).expect("disabled compliance never reads directory").is_none());
}
