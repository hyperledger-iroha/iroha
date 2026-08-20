fn transactions_without_consensus_handshake_metadata(
    transactions: &[iroha_data_model::transaction::SignedTransaction],
    genesis_key_pair: &KeyPair,
) -> Vec<iroha_data_model::transaction::SignedTransaction> {
    transactions
        .iter()
        .filter_map(|transaction| {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                return Some(transaction.clone());
            };
            let filtered = instructions
                .iter()
                .filter(|instruction| {
                    !instruction
                        .as_any()
                        .downcast_ref::<SetParameter>()
                        .is_some_and(|set_param| {
                            matches!(
                                set_param.inner(),
                                Parameter::Custom(custom)
                                    if custom.id() == &consensus_metadata::handshake_meta_id()
                            )
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
            if filtered.len() == instructions.len() {
                return Some(transaction.clone());
            }
            if filtered.is_empty() {
                return None;
            }
            assert_eq!(
                transaction.authority().try_signatory(),
                Some(genesis_key_pair.public_key()),
                "cannot normalize handshake metadata in a genesis transaction signed by another authority"
            );
            assert!(
                transaction.attachments().is_none(),
                "cannot normalize handshake metadata inside a proof-attached genesis transaction"
            );
            assert!(
                transaction.multisig_signatures().is_none(),
                "cannot normalize handshake metadata inside a multisig genesis transaction"
            );
            let canonical_payload = norito::codec::encode_adaptive(transaction.payload());
            let builder =
                iroha_data_model::transaction::TransactionBuilder::decode_genesis_payload(
                    &canonical_payload,
                )
                .expect("canonical genesis transaction payload must decode")
                .with_instructions(filtered);
            Some(
                builder
                    .try_sign(genesis_key_pair.private_key())
                    .expect("re-sign canonical genesis transaction after handshake normalization"),
            )
        })
        .collect()
}
