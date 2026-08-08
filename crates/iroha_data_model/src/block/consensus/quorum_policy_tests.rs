    #[test]
    fn quorum_policy_enforces_strict_supermajority_boundaries() {
        assert_eq!(QuorumPolicy::permissioned_threshold(1), Some(1));
        assert_eq!(QuorumPolicy::permissioned_threshold(2), Some(2));
        assert_eq!(QuorumPolicy::permissioned_threshold(3), Some(3));
        assert_eq!(QuorumPolicy::permissioned_threshold(4), Some(3));
        assert_eq!(QuorumPolicy::permissioned_threshold(5), Some(4));
        assert_eq!(QuorumPolicy::permissioned_threshold(6), Some(5));
        assert_eq!(QuorumPolicy::permissioned_threshold(7), Some(5));
        assert_eq!(QuorumPolicy::permissioned_threshold(8), Some(6));
        assert_eq!(QuorumPolicy::permissioned_threshold(9), Some(7));
        assert_eq!(
            QuorumPolicy::permissioned_threshold(u32::MAX),
            Some(2_863_311_531)
        );
        assert_eq!(QuorumPolicy::permissioned_threshold(0), None);
        assert!(!QuorumPolicy::PermissionedCount(0).is_satisfied_by_count(u32::MAX));

        let count = QuorumPolicy::PermissionedCount(5);
        assert!(!count.is_satisfied_by_count(3));
        assert!(count.is_satisfied_by_count(4));
        assert!(!count.is_satisfied_by_count(6));
        assert!(!count.is_satisfied_by_stake(Some(Quantity::from(4_u64))));
        for validators in 1..=3 {
            let policy = QuorumPolicy::PermissionedCount(validators);
            assert!(!policy.is_satisfied_by_count(validators - 1));
            assert!(policy.is_satisfied_by_count(validators));
            assert!(!policy.is_satisfied_by_count(validators + 1));
        }
        let max_count = QuorumPolicy::PermissionedCount(u32::MAX);
        assert!(!max_count.is_satisfied_by_count(2_863_311_530));
        assert!(max_count.is_satisfied_by_count(2_863_311_531));

        let stake = QuorumPolicy::NposStake(Quantity::from(3_u64));
        assert!(!stake.is_satisfied_by_count(3));
        assert!(!stake.is_satisfied_by_stake(None));
        assert!(!stake.is_satisfied_by_stake(Some(Quantity::from(2_u64))));
        assert!(!stake.is_satisfied_by_stake(Some(Quantity::from(4_u64))));
        assert!(stake.is_satisfied_by_stake(Some("2.01".parse().expect("quantity"))));

        let fractional_stake = QuorumPolicy::NposStake("1.5".parse().expect("quantity"));
        assert!(!fractional_stake.is_satisfied_by_stake(Some("1.0".parse().expect("quantity"))));
        assert!(fractional_stake.is_satisfied_by_stake(Some("1.01".parse().expect("quantity"))));

        let tiny_fractional_stake = QuorumPolicy::NposStake("0.03".parse().expect("quantity"));
        assert!(
            !tiny_fractional_stake.is_satisfied_by_stake(Some("0.02".parse().expect("quantity")))
        );
        assert!(tiny_fractional_stake.is_satisfied_by_stake(Some(
            "0.0200000000000000000000000001".parse().expect("quantity")
        )));

        let zero_total = QuorumPolicy::NposStake(Quantity::zero());
        assert!(!zero_total.is_satisfied_by_stake(Some(Quantity::from(1_u64))));

        let max_total = max_positive_quantity();
        let boundary_stake = QuorumPolicy::NposStake(max_total.clone());
        assert!(boundary_stake.is_satisfied_by_stake(Some(max_total)));
    }

    #[test]
    fn qc_vote_roundtrip_codec_and_decode_from_slice() {
        let vote = QcVote {
            phase: CertPhase::Commit,
            block_hash: dummy_hash(),
            parent_state_root: Hash::new(b"parent_root"),
            post_state_root: Hash::new(b"post_root"),
            height: 7,
            view: 2,
            epoch: 0,
            chain_order_hash: default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: vec![0x01, 0x02],
        };
        let bytes = vote.encode();
        let dec = QcVote::decode(&mut &bytes[..]).expect("decode qc vote");
        assert_eq!(vote, dec);
        let (slice_dec, used) =
            QcVote::decode_from_slice(&bytes).expect("decode_from_slice qc vote");
        assert_eq!(vote, slice_dec);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn vrf_commit_roundtrip_codec() {
        let commit = sample_vrf_commit();
        let bytes = commit.encode();
        let dec = VrfCommit::decode(&mut &bytes[..]).expect("decode vrf commit");
        assert_eq!(commit, dec);
    }

    #[test]
    fn vrf_reveal_roundtrip_codec() {
        let reveal = sample_vrf_reveal();
        let bytes = reveal.encode();
        let dec = VrfReveal::decode(&mut &bytes[..]).expect("decode vrf reveal");
        assert_eq!(reveal, dec);
    }

    #[test]
    fn reconfig_roundtrip_codec() {
        let reconfig = sample_reconfig();
        let bytes = reconfig.encode();
        let dec = Reconfig::decode(&mut &bytes[..]).expect("decode reconfig");
        assert_eq!(reconfig, dec);
    }
