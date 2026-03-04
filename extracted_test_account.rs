        fn test_account_id(domain: &str, seed: u8) -> AccountId {
            let pair = iroha_crypto::KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            let raw = format!("{}@{domain}", pair.public_key());
            AccountId::from_str(&raw).expect("test account id")
        }