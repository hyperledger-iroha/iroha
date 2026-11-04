//! Install actual governed DA admission before exercising native ingest validation.

use eyre::{Result, ensure, eyre};
use iroha_data_model::{
    account::AccountId,
    da::ingest::{DaIngestAdmissionLaneV1, DaIngestAdmissionPolicyV1},
    isi::SetParameter,
    nexus::LaneLifecycleStatusV1,
    parameter::Parameter,
    transaction::FeePaymentIntent,
};
use iroha_model_base::topology::LaneId;
use iroha_test_network::{Network, read_on_dedicated_thread};

/// Admit the network's existing account for one exact active lane and producer epoch.
///
/// The test network's genesis grants its authority `CanSetParameters`. This helper
/// submits an ordinary, fee-quoted parameter transaction and verifies the committed
/// result; it does not add a node-local bypass for DA admission.
///
/// # Errors
/// Returns lifecycle, policy, signing, submission, finality or committed-read failures.
pub async fn install_producer_policy(network: &Network, lane_id: LaneId, epoch: u64) -> Result<()> {
    let client = network.client();
    read_on_dedicated_thread(move || {
        let status = client.client().get_lane_lifecycle_status()?;
        let policy = producer_policy(&status, client.client().account().clone(), lane_id, epoch)?;
        client.submit(
            SetParameter::new(Parameter::Custom(policy.clone().into_custom_parameter())),
            FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let parameters = client.client().get_parameters()?;
        let retained = parameters
            .custom()
            .get(&DaIngestAdmissionPolicyV1::parameter_id())
            .ok_or_else(|| eyre!("committed DA admission policy is missing"))?;
        ensure!(
            DaIngestAdmissionPolicyV1::from_custom_parameter(retained)? == Some(policy),
            "committed DA admission policy differs from the selected producer scope"
        );
        Ok(())
    })
    .await
}

fn producer_policy(
    status: &LaneLifecycleStatusV1,
    owner: AccountId,
    lane_id: LaneId,
    epoch: u64,
) -> Result<DaIngestAdmissionPolicyV1> {
    status.validate()?;
    let lane_incarnation = status
        .incarnations
        .iter()
        .find(|entry| entry.lane_id == lane_id)
        .ok_or_else(|| eyre!("DA fixture lane {lane_id} is absent from the committed catalog"))?
        .incarnation;
    let policy = DaIngestAdmissionPolicyV1 {
        version: DaIngestAdmissionPolicyV1::VERSION,
        revision: 1,
        expected_previous_policy_hash: None,
        lanes: vec![DaIngestAdmissionLaneV1 {
            lane_id,
            lane_incarnation,
            producers: vec![owner],
            current_epoch: epoch,
            grace_epoch: None,
        }],
    };
    policy.validate()?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;
    use iroha_data_model::nexus::LaneCatalog;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use std::collections::BTreeMap;

    fn lifecycle() -> LaneLifecycleStatusV1 {
        LaneLifecycleStatusV1::new(
            &LaneCatalog::default(),
            &BTreeMap::from([(LaneId::SINGLE, Hash::new(b"native DA fixture incarnation"))]),
            None,
        )
        .expect("canonical fixture lifecycle")
    }

    #[test]
    fn producer_policy_binds_only_the_selected_native_scope() {
        let status = lifecycle();
        let policy = producer_policy(&status, ALICE_ID.clone(), LaneId::SINGLE, 7)
            .expect("initial producer policy");
        let incarnation = status.incarnations[0].incarnation;
        assert!(policy.authorizes(&ALICE_ID, LaneId::SINGLE, incarnation, 7));
        assert!(!policy.authorizes(&BOB_ID, LaneId::SINGLE, incarnation, 7));
        assert!(!policy.authorizes(&ALICE_ID, LaneId::SINGLE, incarnation, 6));
        assert!(!policy.authorizes(&ALICE_ID, LaneId::new(1), incarnation, 7));
        assert!(!policy.authorizes(
            &ALICE_ID,
            LaneId::SINGLE,
            Hash::new(b"other incarnation"),
            7
        ));
        assert_eq!(policy.revision, 1);
        assert_eq!(policy.expected_previous_policy_hash, None);
        assert_eq!(
            DaIngestAdmissionPolicyV1::from_custom_parameter(
                &policy.clone().into_custom_parameter()
            )
            .expect("canonical parameter"),
            Some(policy)
        );
    }

    #[test]
    fn producer_policy_refuses_missing_lane_forged_snapshot_and_terminal_epoch() {
        let status = lifecycle();
        assert!(producer_policy(&status, ALICE_ID.clone(), LaneId::new(1), 7).is_err());
        assert!(producer_policy(&status, ALICE_ID.clone(), LaneId::SINGLE, u64::MAX).is_err());
        let mut forged = status;
        forged.incarnations[0].incarnation = Hash::new(b"substituted incarnation");
        assert!(producer_policy(&forged, ALICE_ID.clone(), LaneId::SINGLE, 7).is_err());
    }
}
