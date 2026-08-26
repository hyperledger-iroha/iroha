fn circuit_id_matches(backend: &str, record_id: &str, env_id: &str) -> bool {
    if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
        record_id == crate::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID
            && env_id == crate::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID
    } else if crate::zk::is_stark_fri_v1_backend(backend) {
        record_id == env_id
            && record_id
                .strip_prefix(backend)
                .and_then(|suffix| suffix.strip_prefix(':'))
                == Some(crate::zk::IVM_EXECUTION_V1_CIRCUIT_ID)
    } else {
        false
    }
}

#[cfg(test)]
mod circuit_id_match_tests {
    use super::circuit_id_matches;
    #[test]
    fn circuit_id_matching_accepts_only_exact_backend_canonical_syntax() {
        let halo2_canonical = crate::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID;
        assert!(circuit_id_matches(
            crate::zk::ZK_BACKEND_HALO2_IPA,
            halo2_canonical,
            halo2_canonical,
        ));
        for alias in [
            crate::zk::IVM_EXECUTION_V1_CIRCUIT_ID,
            "halo2/ipa:ivm-execution-v1",
            "halo2/ipa::ivm-execution-v1",
            "halo2/ipa/ivm-execution-v1",
            "halo2/pasta/ivm-execution-v1",
            " halo2/pasta/ipa/ivm-execution-v1 ",
        ] {
            assert!(!circuit_id_matches(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                alias,
                alias,
            ));
        }
        let stark_backend = "stark/fri/sha256-goldilocks";
        let stark_canonical = "stark/fri/sha256-goldilocks:ivm-execution-v1";
        assert!(circuit_id_matches(
            stark_backend,
            stark_canonical,
            stark_canonical,
        ));
        for alias in [
            crate::zk::IVM_EXECUTION_V1_CIRCUIT_ID,
            "stark/fri/sha256-goldilocks/ivm-execution-v1",
            " stark/fri/sha256-goldilocks:ivm-execution-v1 ",
        ] {
            assert!(!circuit_id_matches(stark_backend, alias, alias));
        }
        assert!(!circuit_id_matches("unknown", "same", "same"));
    }
}
