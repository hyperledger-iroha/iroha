    #[test]
    fn fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence() {
        let validator = validator_peers(1).pop().expect("validator fixture");
        let ingress = super::FairV2Ingress::new(
            8,
            64 * 1024 * 1024,
            32 * 1024 * 1024,
            super::TIMEOUT_VOTE_RESERVE_BYTES,
            iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get()
                + super::BODY_ENVELOPE_HEADROOM_BYTES,
        );
        ingress
            .configure_roster([validator.clone()])
            .expect("validator and anonymous protected owners fit");
        ingress.open().expect("open configured roster");
        let message = v2_certified_body_response(7, validator.clone(), 64);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message.clone(),
                validator.clone(),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message.clone(),
                validator.clone(),
            )),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        let mut first = ingress
            .try_recv()
            .expect("checked dequeue owns the coalesced request");
        let first_owner = first
            .take_ingress_ownership()
            .expect("checked dequeue retains exact ownership");
        assert_eq!(first_owner.physical_admission_ordinal(), Some(1));
        assert_eq!(first_owner.runtime_physical_cut(), Some(2));
        let mut illegally_refreshed = first_owner.clone();
        assert!(
            !illegally_refreshed.freeze_runtime_physical_cut(3),
            "an admitted occurrence cannot refresh its frozen predecessor cut"
        );
        assert_eq!(illegally_refreshed.runtime_physical_cut(), Some(2));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                message, validator,
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        let mut retry = ingress
            .try_recv()
            .expect("post-drain transport retry owns a fresh physical occurrence");
        let retry_owner = retry
            .take_ingress_ownership()
            .expect("retry retains exact physical ownership");
        assert_eq!(retry_owner.physical_admission_ordinal(), Some(2));
        assert_eq!(retry_owner.runtime_physical_cut(), Some(3));
        assert_eq!(first_owner.runtime_physical_cut(), Some(2));
    }

    #[test]
    fn fair_v2_ingress_closed_drained_cut_rejects_each_stale_lane_account() {
        let corruptions = [
            "pending_wire",
            "progress_len",
            "certified_fence_escape_len",
            "timeout_vote_len",
            "transport_completion_len",
            "bytes",
            "certified_fence_escape_bytes",
            "timeout_vote_bytes",
            "transport_completion_bytes",
        ];
        for (index, label) in corruptions.into_iter().enumerate() {
            let validator = validator_peers(1).pop().expect("validator fixture");
            let ingress = super::FairV2Ingress::new(
                8,
                64 * 1024 * 1024,
                32 * 1024 * 1024,
                super::TIMEOUT_VOTE_RESERVE_BYTES,
                iroha_config::parameters::defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get()
                    + super::BODY_ENVELOPE_HEADROOM_BYTES,
            );
            ingress
                .configure_roster([validator.clone()])
                .expect("configure one empty validator lane");
            ingress
                .ensure_closed_drained_cut()
                .expect("fresh configured ingress is a closed empty cut");
            {
                let mut state = ingress.state.lock();
                let lane = state
                    .lanes
                    .get_mut(&super::FairV2IngressSource::Validator(validator.clone()))
                    .expect("configured validator lane");
                match index {
                    0 => {
                        lane.pending_wire.insert(super::FairV2IngressWireKey {
                            origin: validator,
                            hash: CryptoHash::new(b"stale closed-cut wire owner"),
                        });
                    }
                    1 => lane.progress_len = 1,
                    2 => lane.certified_fence_escape_len = 1,
                    3 => lane.timeout_vote_len = 1,
                    4 => lane.transport_completion_len = 1,
                    5 => lane.bytes = 1,
                    6 => lane.certified_fence_escape_bytes = 1,
                    7 => lane.timeout_vote_bytes = 1,
                    8 => lane.transport_completion_bytes = 1,
                    _ => unreachable!("the corruption table and mutation cases stay aligned"),
                }
            }
            assert!(
                matches!(
                    ingress.ensure_closed_drained_cut(),
                    Err(reason) if reason.contains("retained physical ownership")
                ),
                "closed drained cut accepted stale lane account `{label}`"
            );
        }
    }
