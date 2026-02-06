//! Core host Halo2 verification tests covering the Goldilocks backend.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[cfg(feature = "goldilocks_backend")]
mod goldilocks {
    use std::{convert::TryFrom, sync::Arc};

    use iroha_config::parameters::defaults;
    use iroha_core::smartcontracts::ivm::host::CoreHost;
    use iroha_data_model::prelude::AccountId;
    use iroha_test_samples::ALICE_ID;
    use ivm::{IVMHost, syscalls as ivm_sys};

    fn make_goldilocks_envelope() -> iroha_zkp_halo2::OpenVerifyEnvelope {
        use iroha_zkp_halo2::{
            GoldilocksParams, GoldilocksPolynomial, GoldilocksScalar, Transcript,
            backend::goldilocks::GoldilocksBackend, norito_helpers as nh,
        };

        let params = GoldilocksParams::new(8).expect("params");
        let coeffs: Vec<GoldilocksScalar> =
            (0u64..8).map(|i| GoldilocksScalar::from(i + 1)).collect();
        let poly = GoldilocksPolynomial::from_coeffs(coeffs);
        let label = ivm::host::LABEL_VOTE_BALLOT;
        let mut tr = Transcript::new(label);
        let p_g = poly.commit(&params).expect("commit");
        let z = GoldilocksScalar::from(4u64);
        let (proof, t) = poly.open(&params, &mut tr, z, p_g).expect("open");
        iroha_zkp_halo2::OpenVerifyEnvelope {
            params: nh::params_to_wire(&params),
            public: nh::poly_open_public::<GoldilocksBackend>(params.n(), z, t, p_g),
            proof: nh::proof_to_wire(&proof),
            transcript_label: label.to_string(),
            vk_commitment: None,
            public_inputs_schema_hash: None,
            domain_tag: None,
        }
    }

    fn tlv_from_env(env: &iroha_zkp_halo2::OpenVerifyEnvelope) -> Vec<u8> {
        let payload = norito::to_bytes(env).expect("encode envelope");
        let mut tlv = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
        tlv.extend_from_slice(&u16::to_be_bytes(ivm::PointerType::NoritoBytes as u16));
        tlv.push(1);
        let payload_len = u32::try_from(payload.len()).expect("payload length fits in u32");
        tlv.extend_from_slice(&payload_len.to_be_bytes());
        tlv.extend_from_slice(&payload);
        let hash: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
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
            max_envelope_bytes: defaults::zk::halo2::MAX_ENVELOPE_BYTES,
            max_proof_bytes: defaults::zk::halo2::MAX_PROOF_BYTES,
            max_transcript_label_len: defaults::zk::halo2::MAX_TRANSCRIPT_LABEL_LEN,
            enforce_transcript_label_ascii: defaults::zk::halo2::ENFORCE_TRANSCRIPT_LABEL_ASCII,
        }
    }

    #[test]
    fn core_host_goldilocks_verification_succeeds() {
        let authority: AccountId = ALICE_ID.clone();
        let mut host =
            CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
        host.set_halo2_config(&base_config());

        let env = make_goldilocks_envelope();
        let tlv = tlv_from_env(&env);
        let mut vm = ivm::IVM::new(1_000_000);
        let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
        vm.set_register(10, ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_ZK_VOTE_VERIFY_BALLOT, &mut vm)
            .expect("syscall ok");
        assert!(gas > 0);
        assert_eq!(vm.register(10), 1);
        assert_eq!(vm.register(11), 0);
    }

    #[test]
    fn core_host_goldilocks_rejected_when_curve_disabled() {
        let authority: AccountId = ALICE_ID.clone();
        let mut host =
            CoreHost::with_accounts(authority.clone(), Arc::new(vec![authority.clone()]));
        let mut cfg = base_config();
        cfg.curve = iroha_config::parameters::actual::ZkCurve::Pallas;
        host.set_halo2_config(&cfg);

        let env = make_goldilocks_envelope();
        let tlv = tlv_from_env(&env);
        let mut vm = ivm::IVM::new(1_000_000);
        let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
        vm.set_register(10, ptr);

        let gas = host
            .syscall(ivm_sys::SYSCALL_ZK_VOTE_VERIFY_BALLOT, &mut vm)
            .expect("syscall ok");
        assert!(gas > 0);
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), 3);
    }
}
