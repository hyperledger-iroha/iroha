fn should_run_wrappers() -> bool {
    std::env::var("IROHA_RUN_ZK_WRAPPERS").ok().as_deref() == Some("1")
}

use iroha_zkp_halo2::backend::pallas::PallasBackend;
use ivm::{
    IVM,
    host::{DefaultHost, ZkCurve, ZkHalo2Backend, ZkHalo2Config},
    kotodama::std as kstd,
};

fn build_env_bytes(k: u32) -> Vec<u8> {
    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;
    let params = h2::Params::new(k as usize).unwrap();
    // Build a small polynomial opening
    let coeffs: Vec<h2::PrimeField64> = vec![0u64.into(); params.n()];
    let poly = h2::Polynomial::from_coeffs(coeffs);
    let mut tr = h2::Transcript::new(ivm::host::LABEL_TRANSFER);
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(1u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    let mut params_wire = nh::params_to_wire(&params);
    params_wire.curve_id = h2::ZkCurveId::Pallas.as_u16();
    let env = h2::OpenVerifyEnvelope {
        params: params_wire,
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: ivm::host::LABEL_TRANSFER.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    norito::to_bytes(&env).expect("encode")
}

#[test]
fn zk_verify_transfer_wrapper_positive() {
    if !should_run_wrappers() {
        eprintln!("Skipping: set IROHA_RUN_ZK_WRAPPERS=1 to run kotodama wrapper tests.");
        return;
    }

    let mut vm = IVM::new(1_000_000);
    let env_bytes = build_env_bytes(8);
    let cfg = ZkHalo2Config {
        enabled: true,
        curve: ZkCurve::Pallas,
        backend: ZkHalo2Backend::Ipa,
        max_k: 18,
        verifier_budget_ms: 50,
        verifier_max_batch: 4,
        ..ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let r = kstd::zk_verify_transfer(&mut host, &mut vm, &env_bytes);
    assert_eq!(r, 1);
}

#[test]
fn zk_verify_batch_wrapper_reports_disabled_under_default_host() {
    if !should_run_wrappers() {
        eprintln!("Skipping: set IROHA_RUN_ZK_WRAPPERS=1 to run kotodama wrapper tests.");
        return;
    }

    use h2::norito_helpers as nh;
    use iroha_zkp_halo2 as h2;

    let params = h2::Params::new(8).unwrap();
    let coeffs: Vec<h2::PrimeField64> = (0u64..8).map(|i| h2::PrimeField64::from(i + 1)).collect();
    let poly = h2::Polynomial::from_coeffs(coeffs);

    let mut tr = h2::Transcript::new(ivm::host::LABEL_BATCH);
    let p_g = poly.commit(&params).unwrap();
    let z = h2::PrimeField64::from(5u64);
    let (proof, t) = poly.open(&params, &mut tr, z, p_g).unwrap();
    let mut params_wire = nh::params_to_wire(&params);
    params_wire.curve_id = h2::ZkCurveId::Pallas.as_u16();
    let env_ok = h2::OpenVerifyEnvelope {
        params: params_wire,
        public: nh::poly_open_public::<PallasBackend>(params.n(), z, t, p_g),
        proof: nh::proof_to_wire(&proof),
        transcript_label: ivm::host::LABEL_BATCH.to_string(),
        vk_commitment: None,
        public_inputs_schema_hash: None,
        domain_tag: None,
    };
    let mut bad_pub = env_ok.public.clone();
    bad_pub.t[0] = bad_pub.t[0].wrapping_add(1);
    let env_bad = h2::OpenVerifyEnvelope {
        public: bad_pub,
        ..env_ok.clone()
    };

    let mut vm = ivm::IVM::new(1_000_000);
    let cfg = ivm::host::ZkHalo2Config {
        enabled: true,
        curve: ivm::host::ZkCurve::Pallas,
        backend: ivm::host::ZkHalo2Backend::Ipa,
        max_k: 12,
        verifier_budget_ms: 50,
        verifier_max_batch: 8,
        ..ivm::host::ZkHalo2Config::default()
    };
    let mut host = DefaultHost::new().with_zk_halo2_config(cfg);
    let (status, out_ptr) =
        ivm::kotodama::std::zk_verify_batch_envs(&mut host, &mut vm, &[env_ok, env_bad]);
    assert_eq!(status, ivm::host::ERR_DISABLED);
    assert_eq!(out_ptr, 0);
}
