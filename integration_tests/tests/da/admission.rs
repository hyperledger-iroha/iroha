//! Governed producer admission for the real-network DA fixtures.

use eyre::{Result, eyre};
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

/// Install a single producer/epoch scope through an ordinary authorized transaction.
pub(crate) async fn install(network: &Network, epoch: u64) -> Result<()> {
    let client = network.client();
    // The synchronous SDK must run on a thread with no inherited Tokio runtime.
    let height = read_on_dedicated_thread(move || {
        let status = client.client().get_lane_lifecycle_status()?;
        let policy = policy_for(status, client.client().account().clone(), epoch)?;
        client.submit(
            SetParameter::new(Parameter::Custom(policy.into_custom_parameter())),
            FeePaymentIntent::authority(Vec::new(), None),
        )?;
        Ok(client.client().get_sumeragi_status()?.committed_height)
    })
    .await?;
    network.ensure_blocks(height).await?;
    Ok(())
}

fn policy_for(
    status: LaneLifecycleStatusV1,
    producer: AccountId,
    epoch: u64,
) -> Result<DaIngestAdmissionPolicyV1> {
    status.validate()?;
    let incarnation = status
        .incarnations
        .iter()
        .find(|entry| entry.lane_id == LaneId::SINGLE)
        .ok_or_else(|| eyre!("DA fixture requires the active primary lane incarnation"))?
        .incarnation;
    let policy = DaIngestAdmissionPolicyV1 {
        version: DaIngestAdmissionPolicyV1::VERSION,
        revision: 1,
        expected_previous_policy_hash: None,
        lanes: vec![DaIngestAdmissionLaneV1 {
            lane_id: LaneId::SINGLE,
            lane_incarnation: incarnation,
            producers: vec![producer],
            current_epoch: epoch,
            grace_epoch: None,
        }],
    };
    policy.validate_transition(None)?;
    Ok(policy)
}

#[test]
fn fixture_admission_is_bound_to_the_validated_incarnation_producer_and_epoch() {
    use std::collections::BTreeMap;

    use iroha_crypto::Hash;
    use iroha_data_model::nexus::LaneCatalog;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    let incarnation = Hash::new(b"DA fixture current incarnation");
    let status = LaneLifecycleStatusV1::new(
        &LaneCatalog::default(),
        &BTreeMap::from([(LaneId::SINGLE, incarnation)]),
        None,
    )
    .expect("canonical fixture catalog");
    let policy = policy_for(status.clone(), ALICE_ID.clone(), 7).expect("initial policy");
    assert!(policy.authorizes(&ALICE_ID, LaneId::SINGLE, incarnation, 7));
    assert!(!policy.authorizes(&BOB_ID, LaneId::SINGLE, incarnation, 7));
    assert!(!policy.authorizes(&ALICE_ID, LaneId::new(1), incarnation, 7));
    assert!(!policy.authorizes(&ALICE_ID, LaneId::SINGLE, incarnation, 6));
    assert!(!policy.authorizes(
        &ALICE_ID,
        LaneId::SINGLE,
        Hash::new(b"different incarnation"),
        7,
    ));
    let mut invalid = status;
    invalid.incarnation_root = Hash::new(b"forged catalog commitment");
    assert!(policy_for(invalid, ALICE_ID.clone(), 7).is_err());
}
