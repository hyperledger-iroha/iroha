fn circuit_id_matches(backend: &str, record_id: &str, env_id: &str) -> bool {
    if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
        match (
            crate::zk::normalize_halo2_ipa_circuit_id(record_id),
            crate::zk::normalize_halo2_ipa_circuit_id(env_id),
        ) {
            (Some(rec), Some(env)) => rec == env,
            _ => false,
        }
    } else if crate::zk::is_stark_fri_v1_backend(backend) {
        match (
            crate::zk::normalize_stark_fri_circuit_id_for_backend(backend, record_id),
            crate::zk::normalize_stark_fri_circuit_id_for_backend(backend, env_id),
        ) {
            (Some(rec), Some(env)) => rec == env,
            _ => false,
        }
    } else {
        record_id == env_id
    }
}

#[cfg(test)]
mod circuit_id_match_tests {
    use super::circuit_id_matches;
    #[test]
    fn circuit_id_matching_uses_the_verifier_canonicalizers_and_fails_closed() {
        assert!(circuit_id_matches(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            "halo2/ipa:tiny-add",
            "halo2/pasta/ipa/tiny-add",
        ));
        assert!(!circuit_id_matches(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            crate::zk::ZK_BACKEND_HALO2_IPA,
            crate::zk::ZK_BACKEND_HALO2_IPA,
        ));
        assert!(!circuit_id_matches(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            "INVALID",
            "INVALID",
        ));
        let stark_backend = "stark/fri/sha256-goldilocks";
        assert!(circuit_id_matches(
            stark_backend,
            "ivm-execution-v1",
            "stark/fri/sha256-goldilocks:ivm-execution-v1",
        ));
        assert!(!circuit_id_matches(
            stark_backend,
            stark_backend,
            stark_backend,
        ));
    }
}
