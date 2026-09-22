//! Pre-receipt input checks against the recovered layout and actual transport owners.

use iroha_data_model::{
    NetworkId, block::MAX_QUEUE_PLAN_ADMISSION_BYTES, transaction::TransactionEntrypoint,
};

use super::{AdmissionCapacityUnavailableV1, SumeragiHandle};
use crate::torii_proxy::{
    QueuePlanAdmissionBindingV1, maximum_lane_admitted_input_encoded_len_v1,
    maximum_lane_admitted_input_envelope_sizes_v1, validate_queue_plan_binding_for_request,
};

/// A QueuePlan input cannot yet acquire a durable admission receipt.
#[derive(Debug, thiserror::Error)]
pub enum QueuePlanInputCapacityErrorV1 {
    /// The process has no authenticated, usable capacity owner.
    #[error(transparent)]
    Unavailable(#[from] AdmissionCapacityUnavailableV1),
    /// Recovery capacity exists, but the live process admission owner is closed.
    #[error("Sumeragi admission owner is not active")]
    Inactive,
    /// The exact transaction, network, routing plan or binding is inconsistent.
    #[error("invalid QueuePlan input: {0}")]
    Invalid(String),
    /// A canonical envelope exceeds an immutable byte capacity.
    #[error("QueuePlan {envelope} requires {required} bytes, exceeding {capacity}")]
    Oversized {
        /// Encoding owner whose envelope cannot fit.
        envelope: &'static str,
        /// Worst-quorum canonical envelope size.
        required: usize,
        /// Actual service or protocol byte capacity.
        capacity: usize,
    },
    /// The native body cannot fit the authenticated RS16 stripes.
    #[error("QueuePlan native payload cannot fit authenticated availability: {0}")]
    Availability(String),
}

impl SumeragiHandle {
    /// Check immutable input and publication capacities before issuing a receipt.
    ///
    /// Uses the recovered network layout and this handle's actual transport
    /// owner, never request-supplied capacities or default geometry. Counts the
    /// largest valid durability quorum, native route descriptor, both direct
    /// publication variants, encryption and stream framing.
    ///
    /// Success is a necessary capacity check, not a reservation or a promise of
    /// a future global carrier. Live authority and journal ownership remain
    /// separate. TODO: enforce a guaranteed global admission opportunity with
    /// bounded mandatory metadata and owner-preserving optional-work deferral.
    ///
    /// # Errors
    /// Rejects unavailable recovery, inconsistent bindings, or any body/frame
    /// that cannot fit its immutable owner even when that owner's queue is empty.
    pub fn check_queue_plan_input_capacity(
        &self,
        network_id: &NetworkId,
        entrypoint: &TransactionEntrypoint,
        binding: &QueuePlanAdmissionBindingV1,
    ) -> Result<(), QueuePlanInputCapacityErrorV1> {
        let capacity = self.authenticated_admission_capacity()?;
        if !self.admission_ready() {
            return Err(QueuePlanInputCapacityErrorV1::Inactive);
        }
        if capacity.network_id() != *network_id {
            return Err(QueuePlanInputCapacityErrorV1::Invalid(
                "capacity belongs to another network".to_owned(),
            ));
        }
        let plan = binding
            .admission_context
            .routing_plan()
            .map_err(QueuePlanInputCapacityErrorV1::Invalid)?;
        validate_queue_plan_binding_for_request(binding, network_id, entrypoint, &plan)
            .map_err(QueuePlanInputCapacityErrorV1::Invalid)?;
        let input_bytes = maximum_lane_admitted_input_encoded_len_v1(entrypoint, binding)
            .map_err(QueuePlanInputCapacityErrorV1::Invalid)?;
        require_capacity(
            "complete input",
            input_bytes,
            MAX_QUEUE_PLAN_ADMISSION_BYTES,
        )?;
        let sizes = maximum_lane_admitted_input_envelope_sizes_v1(entrypoint, binding)
            .map_err(QueuePlanInputCapacityErrorV1::Invalid)?;
        let native_bytes = u64::try_from(sizes.native_payload_bytes).map_err(|_| {
            QueuePlanInputCapacityErrorV1::Availability("payload length overflows u64".to_owned())
        })?;
        capacity
            .check_payload_size(network_id, native_bytes)
            .map_err(QueuePlanInputCapacityErrorV1::Availability)?;
        for (envelope, required, capacity) in [
            (
                "publication control frame",
                sizes.publication_plaintext_bytes,
                self.block.control_frame_byte_capacity,
            ),
            (
                "republication consensus frame",
                sizes.republication_plaintext_bytes,
                self.block.consensus_frame_byte_capacity,
            ),
            (
                "publication outbound frame",
                sizes.publication_queue_bytes,
                self.block.outbound_high_frame_byte_capacity,
            ),
            (
                "republication outbound frame",
                sizes.republication_queue_bytes,
                self.block.outbound_high_frame_byte_capacity,
            ),
        ] {
            require_capacity(envelope, required, capacity)?;
        }
        Ok(())
    }
}

fn require_capacity(
    envelope: &'static str,
    required: usize,
    capacity: usize,
) -> Result<(), QueuePlanInputCapacityErrorV1> {
    if required > capacity {
        return Err(QueuePlanInputCapacityErrorV1::Oversized {
            envelope,
            required,
            capacity,
        });
    }
    Ok(())
}

#[cfg(all(test, feature = "bls"))]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        block::consensus_v2 as wire,
        transaction::{FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder},
    };
    use iroha_model_base::peer::PeerId;
    use std::sync::Arc;

    fn fixture(
        layout: wire::DataAvailabilityLayout,
    ) -> (
        SumeragiHandle,
        NetworkId,
        TransactionEntrypoint,
        QueuePlanAdmissionBindingV1,
    ) {
        let mut keys = (1_u8..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"capacity input fixture",
        )));
        let validators = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let roster = validators
            .iter()
            .cloned()
            .map(|validator| wire::ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(network_id, 100, &roster);
        let context = wire::HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).unwrap(),
            roster,
            kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority,
            nexus_amx_context_hash: Hash::new(b"nexus"),
            execution_policy_hash: Hash::new(b"policy"),
            da_layout: layout,
            leader_seed: [9; 32],
        };
        let harness = super::super::SumeragiIngressTestHarness::new(4);
        harness
            .authenticate_admission_capacity(
                context,
                keys.iter()
                    .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
                    .collect(),
                &iroha_config::parameters::actual::Sumeragi::default(),
            )
            .unwrap();
        let signer = KeyPair::from_seed(vec![17; 32], Algorithm::Ed25519);
        let entrypoint = TransactionEntrypoint::External(
            TransactionBuilder::new(
                network_id,
                AccountId::new(signer.public_key().clone()),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
            .sign(signer.private_key()),
        );
        let route = crate::queue::RoutingDecision::new(
            iroha_model_base::topology::LaneId::SINGLE,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        );
        let plan = crate::queue::RoutingPlan::single(route);
        let admission_context = crate::queue::QueuePlanAdmissionContextV1 {
            version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
            authority_height: 0,
            proposal_height: 1,
            predecessor_block_hash: None,
            routing_plan_digest: plan.digest(),
            route_incarnations: vec![crate::queue::QueuePlanRouteIncarnationV1 {
                leg: crate::queue::RouteLeg::new(route, crate::queue::RouteLegRole::Coordinator),
                lane_incarnation: Hash::new(b"capacity incarnation"),
                validator_set_hash_version:
                    iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validators),
                validator_set: validators,
                validator_count: 4,
                durability_threshold: 2,
            }],
        };
        let binding = crate::torii_proxy::new_queue_plan_admission_binding(
            &network_id,
            &entrypoint,
            &plan,
            admission_context,
            73,
        )
        .unwrap();
        (harness.handle(), network_id, entrypoint, binding)
    }

    #[test]
    fn input_capacity_requires_authenticated_owner_and_exact_network() {
        let (handle, network, input, binding) =
            fixture(wire::recommended_data_availability_layout());
        handle
            .check_queue_plan_input_capacity(&network, &input, &binding)
            .unwrap();
        let (pending, _, _) = super::super::test_sumeragi_handle(4);
        assert!(matches!(
            pending.check_queue_plan_input_capacity(&network, &input, &binding),
            Err(QueuePlanInputCapacityErrorV1::Unavailable(
                AdmissionCapacityUnavailableV1::Pending
            ))
        ));
        let disabled = SumeragiHandle::emergency_fast_disabled();
        assert!(matches!(
            disabled.check_queue_plan_input_capacity(&network, &input, &binding),
            Err(QueuePlanInputCapacityErrorV1::Unavailable(
                AdmissionCapacityUnavailableV1::Disabled
            ))
        ));
        let foreign =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
        assert!(matches!(
            handle.check_queue_plan_input_capacity(&foreign, &input, &binding),
            Err(QueuePlanInputCapacityErrorV1::Invalid(_))
        ));
        handle
            .ingress_ready
            .store(false, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            handle.check_queue_plan_input_capacity(&network, &input, &binding),
            Err(QueuePlanInputCapacityErrorV1::Inactive)
        ));
        handle
            .ingress_ready
            .store(true, std::sync::atomic::Ordering::Release);
        handle.output_guard.activate_restart_required();
        assert!(matches!(
            handle.check_queue_plan_input_capacity(&network, &input, &binding),
            Err(QueuePlanInputCapacityErrorV1::Unavailable(
                AdmissionCapacityUnavailableV1::RestartRequired
            ))
        ));
    }

    #[test]
    fn input_capacity_counts_native_descriptor_before_signed_rs16_check() {
        let (_, _, input, binding) = fixture(wire::recommended_data_availability_layout());
        let sizes = maximum_lane_admitted_input_envelope_sizes_v1(&input, &binding).unwrap();
        assert!(sizes.native_payload_bytes > sizes.complete_input_bytes);
        for (limit, fits) in [
            (sizes.native_payload_bytes, true),
            (sizes.native_payload_bytes - 1, false),
            (sizes.complete_input_bytes, false),
        ] {
            let mut layout = wire::recommended_data_availability_layout();
            layout.max_payload_size_bytes = limit as u64;
            let (handle, network, input, binding) = fixture(layout);
            let result = handle.check_queue_plan_input_capacity(&network, &input, &binding);
            if fits {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(QueuePlanInputCapacityErrorV1::Availability(_))
                ));
            }
        }
    }

    #[test]
    fn input_capacity_checks_actual_topic_and_encrypted_queue_boundaries() {
        for owner in 0..3 {
            let (mut handle, network, input, binding) =
                fixture(wire::recommended_data_availability_layout());
            let sizes = maximum_lane_admitted_input_envelope_sizes_v1(&input, &binding).unwrap();
            let expected = match owner {
                0 => sizes.publication_plaintext_bytes,
                1 => sizes.republication_plaintext_bytes,
                _ => sizes
                    .publication_queue_bytes
                    .max(sizes.republication_queue_bytes),
            };
            for capacity in [expected, expected - 1] {
                let block = Arc::get_mut(&mut handle.block)
                    .expect("fixture dropped every other physical owner");
                match owner {
                    0 => block.control_frame_byte_capacity = capacity,
                    1 => block.consensus_frame_byte_capacity = capacity,
                    _ => block.outbound_high_frame_byte_capacity = capacity,
                }
                let result = handle.check_queue_plan_input_capacity(&network, &input, &binding);
                if capacity == expected {
                    result.unwrap();
                } else {
                    assert!(
                        matches!(result, Err(QueuePlanInputCapacityErrorV1::Oversized {required, capacity: actual, ..}) if required == expected && actual == capacity)
                    );
                }
            }
        }
    }
}
