#[test]
fn transaction_height_expiry_is_exclusive_when_ttl_is_optional() {
    use iroha_data_model::{isi::Log, metadata::Metadata, transaction::TransactionBuilder};
    use iroha_logger::Level;
    use iroha_primitives::json::Json;
    use nonzero_ext::nonzero;
    use std::time::Duration;
    for (expires_at_height, should_accept) in [(1_u64, false), (2, true)] {
        let (mut world, authority_id, kp) = world_with_authority("wonderland");
        let mut params = iroha_data_model::parameter::system::Parameters::default();
        params.transaction = params.transaction.with_ingress_enforcement(false, false);
        world.parameters = mv::cell::Cell::new(params);
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query_handle = crate::query::store::LiveQueryStore::start_test();
        let chain: ChainId = "ttl-check-chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain);
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut metadata = Metadata::default();
        metadata.insert(
            iroha_data_model::name::Name::from_str("expires_at_height").unwrap(),
            Json::from(expires_at_height),
        );
        let tx = TransactionBuilder::new(
            test_network_id(),
            authority_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "ttl".into())])
        .with_metadata(metadata)
        .sign(kp.private_key());
        let default_limits = TransactionParameters::default();
        let limits = TransactionParameters::with_max_signatures(
            default_limits.max_signatures(),
            default_limits.max_instructions(),
            default_limits.ivm_bytecode_size(),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        )
        .with_ingress_enforcement(false, false);
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        let accepted = AcceptedTransaction::accept(
            tx,
            &test_network_id(),
            Duration::from_secs(0),
            limits,
            &crypto_cfg,
        )
        .expect("stateless checks accept optional but present height expiry");
        let mut ivm_cache = IvmCache::new();
        let (_hash, result) = block.validate_transaction(accepted, &mut ivm_cache);
        if should_accept {
            assert!(
                result.is_ok(),
                "expiry one height after current height must remain valid: {result:?}"
            );
        } else {
            match result {
                Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg))) => {
                    assert!(
                        msg.contains("expired at height 1; current height is 1"),
                        "expected equality-boundary expiry rejection, got {msg}"
                    );
                }
                other => panic!("expected expired Validation::NotPermitted, got {other:?}"),
            }
        }
    }
}
