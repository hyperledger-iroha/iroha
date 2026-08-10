// Exact-network persistence and governed-key recovery tests for provider VRF state.

#[test]
fn vrf_restart_drops_revoked_provider_entries_but_keeps_replay_high_water() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempdir().expect("temp dir");
        let root = canonical_temp_root(&dir);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private state root");
        let path = root.join("vrf-state.to");
        let provider_id = [0x41; 32];
        let manifest_digest = [0x42; 32];
        let submission = ProviderVrfSubmissionV1 {
            version: POR_VRF_SUBMISSION_VERSION_V1,
            network_id: *test_network_id(0x61).as_bytes(),
            provider_id,
            manifest_digest,
            epoch_id: 7,
            drand_round: 9,
            output: [0x43; 32],
            proof: iroha_crypto::vrf::VrfProof::SigInG1([0x44; 48]),
            sequence: 11,
            issued_at: 1_800_000_000,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0x45; 32],
                signature: vec![0x46; 64],
            },
        };
        submission.validate().expect("structural submission");
        let key = VrfStateKeyV1 {
            epoch_id: submission.epoch_id,
            provider_id,
            manifest_digest,
        };
        let mut persisted = VrfState::default();
        persisted.entries.insert(key, submission);
        persisted.sequences.insert(provider_id, 11);
        let network_id = test_network_id(0x61);
        persist_vrf_state(&path, &network_id, &persisted).expect("persist admitted-era state");

        let restored = load_vrf_state(
            &path,
            16,
            &crate::sorafs::AdmissionRegistry::empty(),
            &network_id,
        )
        .expect("revoked-provider state must not brick restart");
        assert!(restored.entries.is_empty());
        assert_eq!(restored.sequences.get(&provider_id), Some(&11));
    }
}

#[test]
fn vrf_restart_rejects_state_from_another_exact_network() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempdir().expect("temp dir");
        let root = canonical_temp_root(&dir);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private state root");
        let path = root.join("vrf-foreign-network-state.to");
        let local = test_network_id(0x62);
        let foreign = test_network_id(0x63);
        persist_vrf_state(&path, &local, &VrfState::default())
            .expect("persist local-network state");

        assert!(matches!(
            load_vrf_state(
                &path,
                16,
                &crate::sorafs::AdmissionRegistry::empty(),
                &foreign,
            ),
            Err(VrfError::Persistence(_))
        ));
    }
}

#[test]
fn vrf_restart_rejects_foreign_network_entry_inside_local_snapshot() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempdir().expect("temp dir");
        let root = canonical_temp_root(&dir);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private state root");
        let path = root.join("vrf-misbound-entry-state.to");
        let local = test_network_id(0x64);
        let foreign = test_network_id(0x65);
        let submission = ProviderVrfSubmissionV1 {
            version: POR_VRF_SUBMISSION_VERSION_V1,
            network_id: *foreign.as_bytes(),
            provider_id: [0x57; 32],
            manifest_digest: [0x58; 32],
            epoch_id: 9,
            drand_round: 11,
            output: [0x59; 32],
            proof: iroha_crypto::vrf::VrfProof::SigInG1([0x5A; 48]),
            sequence: 13,
            issued_at: 1_800_000_000,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0x5B; 32],
                signature: vec![0x5C; 64],
            },
        };
        let key = VrfStateKeyV1 {
            epoch_id: submission.epoch_id,
            provider_id: submission.provider_id,
            manifest_digest: submission.manifest_digest,
        };
        let mut state = VrfState::default();
        state.entries.insert(key, submission);
        state.sequences.insert(key.provider_id, 13);
        persist_vrf_state(&path, &local, &state).expect("persist local snapshot envelope");

        assert!(matches!(
            load_vrf_state(
                &path,
                16,
                &crate::sorafs::AdmissionRegistry::empty(),
                &local,
            ),
            Err(VrfError::Persistence(_))
        ));
    }
}

#[test]
fn vrf_restart_drops_entries_invalidated_by_active_key_rotation() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempdir().expect("temp dir");
        let root = canonical_temp_root(&dir);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private state root");
        let path = root.join("vrf-rotated-state.to");
        let envelope: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(include_bytes!(
            "../../../../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
        ))
        .expect("decode provider admission fixture");
        let provider_id = envelope.proposal.provider_id;
        let trusted_signers = envelope
            .council_signatures
            .iter()
            .map(|signature| signature.signer)
            .collect::<HashSet<_>>();
        let policy = ProviderAdmissionCouncilPolicy::new(trusted_signers, 1)
            .expect("fixture council policy");
        let admission = crate::sorafs::AdmissionRegistry::from_envelopes(policy, [envelope])
            .expect("active admission fixture");
        let manifest_digest = [0x52; 32];
        let submission = ProviderVrfSubmissionV1 {
            version: POR_VRF_SUBMISSION_VERSION_V1,
            network_id: *test_network_id(0x64).as_bytes(),
            provider_id,
            manifest_digest,
            epoch_id: 8,
            drand_round: 10,
            output: [0x53; 32],
            proof: iroha_crypto::vrf::VrfProof::SigInG1([0x54; 48]),
            sequence: 12,
            issued_at: 1_800_000_000,
            // Structurally valid old-key material that cannot verify under the current
            // council-admitted provider key.
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0x55; 32],
                signature: vec![0x56; 64],
            },
        };
        submission
            .validate()
            .expect("structural old-key submission");
        let key = VrfStateKeyV1 {
            epoch_id: submission.epoch_id,
            provider_id,
            manifest_digest,
        };
        let mut persisted = VrfState::default();
        persisted.entries.insert(key, submission);
        persisted.sequences.insert(provider_id, 12);
        let network_id = test_network_id(0x64);
        persist_vrf_state(&path, &network_id, &persisted).expect("persist pre-rotation state");

        let restored = load_vrf_state(&path, 16, &admission, &network_id)
            .expect("governed key rotation must not brick restart");
        assert!(restored.entries.is_empty());
        assert_eq!(restored.sequences.get(&provider_id), Some(&12));
    }
}
