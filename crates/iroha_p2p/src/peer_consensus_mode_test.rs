        #[test]
        fn peer_admission_requires_an_exact_typed_consensus_mode() {
            let caps = ConsensusHandshakeCaps {
                mode: ConsensusMode::Permissioned,
                proto_version: 2,
                consensus_fingerprint: [0xA5; 32],
                config: consensus_caps([0x5A; 32]),
            };
            let matching = build_consensus_meta(Some(&caps));
            enforce_consensus_caps(Some(&caps), &matching)
                .expect("identical typed consensus mode must be admitted");

            let mut mismatched = matching;
            mismatched.mode = Some(ConsensusMode::Npos);
            let error = enforce_consensus_caps(Some(&caps), &mismatched)
                .expect_err("a different typed consensus mode must be rejected");
            let crate::Error::HandshakeConsensusMismatch { reason } = error else {
                panic!("unexpected consensus-mode mismatch error: {error:?}");
            };
            assert!(reason.contains(ConsensusMode::Permissioned.tag()));
            assert!(reason.contains(ConsensusMode::Npos.tag()));

            let mut missing = matching;
            missing.mode = None;
            assert!(matches!(
                enforce_consensus_caps(Some(&caps), &missing),
                Err(crate::Error::HandshakeConsensusMismatch { reason })
                    if reason == "missing consensus mode"
            ));
        }
