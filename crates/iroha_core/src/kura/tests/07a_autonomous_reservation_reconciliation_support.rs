    fn autonomous_reservation_reconciliation_group(
        ordered_keys: Vec<LaneQueueReservationKeyV2>,
    ) -> LaneQueueReservationReconciliationGroupV1 {
        let first = ordered_keys
            .first()
            .copied()
            .expect("reconciliation group has one reservation");
        LaneQueueReservationReconciliationGroupV1 {
            identity: LaneQueueReservationGroupIdentityV1 {
                lane_id: first.lane_id,
                dataspace_id: first.dataspace_id,
                lane_incarnation: first.lane_incarnation,
                proposal_height: first.proposal_height,
                lane_block_height: first.lane_block_height,
                lane_block_view: first.lane_block_view,
                reservation_owner_hash: first.reservation_owner_hash,
                proposal_identity_hash: first.proposal_identity_hash,
            },
            ordered_keys,
        }
    }

    fn two_reservation_autonomous_lane_payload_for_kura(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_block_height: u64,
        signer: &KeyPair,
    ) -> (Hash, u64, LaneExecutablePayloadV1) {
        let (chain_id_hash, epoch, source) =
            autonomous_lane_payload_for_kura(lane_id, dataspace_id, lane_block_height, signer);
        let chain: ChainId = "kura-autonomous-view-checkpoint".parse().expect("chain id");
        let second = TransactionBuilder::new(
            chain,
            (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "second strict reservation payload".to_owned(),
        )])
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let entrypoints = vec![
            source.entrypoints[0].clone(),
            TransactionEntrypoint::External(second),
        ];
        let entrypoint_hashes = entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect::<Vec<_>>();
        let mut proposal = source.origin_proposal.clone();
        proposal.descriptor.accepted_candidate_indices = vec![0, 1];
        proposal.descriptor.accepted_transaction_hashes = entrypoint_hashes;
        proposal.descriptor.subject_hash = Hash::new(b"two-reservation-strict-subject");
        proposal.descriptor.payload_ownership_hash = Hash::new(b"two-reservation-strict-ownership");
        proposal.descriptor.rbc_instance_hash = Hash::new(b"two-reservation-strict-rbc");
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let routing_plans = entrypoints
            .iter()
            .map(|_| RoutingPlan::single(crate::queue::RoutingDecision::new(lane_id, dataspace_id)))
            .collect::<Vec<_>>();
        let reservation_owner_hash = Hash::new(b"two-reservation-strict-owner");
        let reservation_keys = entrypoints
            .iter()
            .zip(&routing_plans)
            .enumerate()
            .map(|(index, (entrypoint, routing_plan))| {
                let accepted =
                    AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
                LaneQueueReservationKeyV2 {
                    version: LaneQueueReservationKeyV2::VERSION,
                    signed_transaction_hash: accepted.hash(),
                    entrypoint_hash: entrypoint.hash(),
                    queue_plan_admission_binding_hash: Hash::new_from_chunks(&[
                        b"two-reservation-strict-admission",
                        &u64::try_from(index)
                            .expect("reservation index")
                            .to_le_bytes(),
                    ]),
                    routing_plan_digest: routing_plan.digest(),
                    coordinator_leg: routing_plan.coordinator_leg(),
                    lane_id,
                    dataspace_id,
                    lane_incarnation: proposal.descriptor.lane_incarnation,
                    proposal_height: proposal.descriptor.proposal_height,
                    lane_block_height,
                    lane_block_view: proposal.descriptor.lane_block_view,
                    reservation_owner_hash,
                    proposal_identity_hash: proposal.proposal_hash,
                }
            })
            .collect::<Vec<_>>();
        let validator = PeerId::new(signer.public_key().clone());
        let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            chain_id_hash,
            epoch,
            proposal,
            entrypoints,
            reservation_keys,
            routing_plans,
            vec![None, None],
            validator,
            signer.private_key(),
        )
        .expect("two-reservation autonomous payload");
        (chain_id_hash, epoch, payload)
    }
