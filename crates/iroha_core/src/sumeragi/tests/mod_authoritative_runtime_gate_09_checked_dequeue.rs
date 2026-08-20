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
        let message = v2_certified_body_response(7, 0, 64);
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message.clone(),
                Some(validator.clone()),
            )),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message.clone(),
                Some(validator.clone()),
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
            ingress.try_push(InboundBlockMessage::new(message, Some(validator))),
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
