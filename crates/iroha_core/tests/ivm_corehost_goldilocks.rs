//! Core host rejection of polynomial-opening payloads and registered IPA curve policy.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
mod goldilocks {
    use iroha_config::parameters::defaults;
    use iroha_core::smartcontracts::ivm::host::CoreHost;
    use iroha_data_model::prelude::AccountId;
    use iroha_test_samples::ALICE_ID;
    use ivm::{IVMHost, syscalls as ivm_sys};
    use std::{convert::TryFrom, sync::Arc};
    fn make_goldilocks_envelope() -> iroha_zkp_halo2::OpenVerifyEnvelope {
        use iroha_zkp_halo2::{
            Params, Polynomial, PrimeField64, Transcript, ZkCurveId,
            backend::pallas::PallasBackend, norito_helpers as nh,
        };
        let params = Params::new(8).expect("params");
        let coeffs: Vec<PrimeField64> = (0u64..8).map(|i| PrimeField64::from(i + 1)).collect();
        let poly = Polynomial::from_coeffs(coeffs);
        let label = ivm::host::LABEL_VOTE_BALLOT;
        let mut tr = Transcript::new(label);
        let p_g = poly.commit(&params).expect("commit");
        let z = PrimeField64::from(4u64);
        let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
        let mut envelope = iroha_zkp_halo2::OpenVerifyEnvelope {
            params: nh::params_to_wire(&params),
            public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
            proof: nh::proof_to_wire(&proof),
            transcript_label: label.to_string(),
            vk_commitment: None,
            public_inputs_schema_hash: None,
            domain_tag: None,
        };
        // The unsupported field identity must be rejected before group decoding.
        envelope.params.curve_id = ZkCurveId::Goldilocks.as_u16();
        envelope.public.curve_id = ZkCurveId::Goldilocks.as_u16();
        envelope
    }
    fn envelope_tlv(payload: &[u8]) -> Vec<u8> {
        let mut tlv = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
        tlv.extend_from_slice(&u16::to_be_bytes(ivm::PointerType::NoritoBytes as u16));
        tlv.push(1);
        let payload_len = u32::try_from(payload.len()).expect("payload length fits in u32");
        tlv.extend_from_slice(&payload_len.to_be_bytes());
        tlv.extend_from_slice(payload);
        let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
        tlv.extend_from_slice(&hash);
        tlv
    }
    fn base_config() -> iroha_config::parameters::actual::Halo2 {
        iroha_config::parameters::actual::Halo2 {
            enabled: true,
            curve: iroha_config::parameters::actual::ZkCurve::Goldilocks,
            backend: iroha_config::parameters::actual::Halo2Backend::Ipa,
            max_k: 18,
            verifier_budget_ms: 200,
            verifier_max_batch: 8,
            verifier_worker_threads: defaults::zk::halo2::VERIFIER_WORKER_THREADS,
            verifier_queue_cap: defaults::zk::halo2::VERIFIER_QUEUE_CAP,
            verifier_enqueue_wait_ms: defaults::zk::halo2::VERIFIER_ENQUEUE_WAIT_MS,
            verifier_retry_ring_cap: defaults::zk::halo2::VERIFIER_RETRY_RING_CAP,
            verifier_retry_max_attempts: defaults::zk::halo2::VERIFIER_RETRY_MAX_ATTEMPTS,
            verifier_retry_tick_ms: defaults::zk::halo2::VERIFIER_RETRY_TICK_MS,
            max_envelope_bytes: defaults::zk::halo2::MAX_ENVELOPE_BYTES,
            max_proof_bytes: defaults::zk::halo2::MAX_PROOF_BYTES,
            max_transcript_label_len: defaults::zk::halo2::MAX_TRANSCRIPT_LABEL_LEN,
            enforce_transcript_label_ascii: defaults::zk::halo2::ENFORCE_TRANSCRIPT_LABEL_ASCII,
        }
    }
    #[test]
    fn core_host_rejects_non_binding_goldilocks_commitments() {
        let env = make_goldilocks_envelope();
        let raw = norito::to_bytes(&env).expect("encode Goldilocks envelope");
        assert!(matches!(
            ivm::zk_verify::verify_open_envelope(&raw),
            Err(iroha_zkp_halo2::Error::UnsupportedBackend {
                backend: iroha_zkp_halo2::ZkCurveId::Goldilocks
            })
        ));
        let tlv = envelope_tlv(&raw);
        for curve in [
            iroha_config::parameters::actual::ZkCurve::Pallas,
            iroha_config::parameters::actual::ZkCurve::Goldilocks,
        ] {
            let authority: AccountId = ALICE_ID.clone();
            let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority]));
            let mut cfg = base_config();
            cfg.curve = curve;
            host.set_halo2_config(&cfg);
            let mut vm = ivm::IVM::new(1_000_000);
            let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
            vm.set_register(10, ptr);
            let gas = host
                .syscall(ivm_sys::SYSCALL_ZK_VOTE_VERIFY_BALLOT, &mut vm)
                .expect("syscall ok");
            assert!(gas > 0);
            assert_eq!(vm.register(10), 0);
            // Production accepts only the registry-bound data-model envelope;
            // a polynomial opening cannot reach curve policy or arm a ballot latch.
            assert_eq!(vm.register(11), ivm::host::ERR_DECODE);
        }
    }
    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn core_host_enforces_registered_ipa_curve_policy() {
        use iroha_core::zk;
        use iroha_data_model::proof::VerifyingKeyId;
        use std::collections::BTreeMap;

        let authority: AccountId = ALICE_ID.clone();
        let mut host = CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority]));
        let record = zk::halo2_ipa_ivm_execution_vk_record("ballot", 1)
            .expect("canonical registered Pallas key");
        let id = VerifyingKeyId::new(zk::ZK_BACKEND_HALO2_IPA, "curve_policy");
        let proof = zk::prove_halo2_ipa_ivm_execution_envelope(
            zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
            record.key.as_ref().expect("inline verifier key"),
            iroha_crypto::Hash::new(b"curve-policy-code"),
            iroha_crypto::Hash::new(b"curve-policy-overlay"),
            iroha_crypto::Hash::new(b"curve-policy-events"),
            iroha_crypto::Hash::new(b"curve-policy-gas"),
            None,
        )
        .expect("valid registered Pallas proof");
        let tlv = envelope_tlv(&proof.bytes);

        let mut unsupported_record = record.clone();
        unsupported_record.curve = "goldilocks".to_owned();
        assert_eq!(
            host.set_verifying_keys(BTreeMap::from([(id.clone(), unsupported_record)])),
            Err(ivm::VMError::NoritoInvalid),
            "Goldilocks is not an admissible IPA group in the verifier registry"
        );
        host.set_verifying_keys(BTreeMap::from([(id, record)]))
            .expect("install canonical Pallas key");

        for (curve, result, status) in [
            (iroha_config::parameters::actual::ZkCurve::Pallas, 1, 0),
            (
                iroha_config::parameters::actual::ZkCurve::Goldilocks,
                0,
                ivm::host::ERR_CURVE,
            ),
            (
                iroha_config::parameters::actual::ZkCurve::Bn254,
                0,
                ivm::host::ERR_CURVE,
            ),
        ] {
            let mut cfg = base_config();
            cfg.curve = curve;
            host.set_halo2_config(&cfg);
            let mut vm = ivm::IVM::new(1_000_000);
            let ptr = vm
                .alloc_input_tlv(&tlv)
                .expect("alloc canonical envelope tlv");
            vm.set_register(10, ptr);
            let gas = host
                .syscall(ivm_sys::SYSCALL_ZK_VOTE_VERIFY_BALLOT, &mut vm)
                .expect("curve-policy syscall");
            assert!(gas > 0);
            assert_eq!(vm.register(10), result);
            assert_eq!(vm.register(11), status);
        }
    }
}
