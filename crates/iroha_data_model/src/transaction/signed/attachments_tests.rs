#[cfg(test)]
mod attachments_tests {
    use super::*;
    #[test]
    fn signed_tx_with_attachments_roundtrip() {
        let network_id = test_network_id(0x2E);
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let authority = AccountId::new(iroha_crypto::PublicKey::from(private_key.clone()));
        let attachments = crate::proof::ProofAttachmentList::try_from(vec![
            crate::proof::ProofAttachment::new_ref(
                "halo2/ipa".into(),
                crate::proof::ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                crate::proof::VerifyingKeyId::new("halo2/ipa", "vk_1"),
            ),
        ])
        .expect("one attachment is a valid bounded proof list");
        let tx: SignedTransaction = TransactionBuilder::new(
            network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(Vec::new().into()))
        .with_attachments(attachments)
        .sign(&private_key);
        let bytes = norito::to_bytes(&tx).expect("encode");
        let archived = norito::from_bytes::<SignedTransaction>(&bytes).expect("archived");
        let decoded: SignedTransaction = norito::core::NoritoDeserialize::deserialize(archived);
        assert!(decoded.attachments().is_some());
        decoded
            .verify_signature()
            .expect("round-tripped attachment remains signature-bound");
        let original_hash = decoded.hash();
        let mut tampered = decoded;
        tampered.payload.attachments = Some(
            crate::proof::ProofAttachmentList::try_from(vec![
                crate::proof::ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    crate::proof::ProofBox::new("halo2/ipa".into(), vec![9, 9, 9]),
                    crate::proof::VerifyingKeyId::new("halo2/ipa", "vk_1"),
                ),
            ])
            .expect("one attachment is a valid bounded proof list"),
        );
        assert_ne!(
            tampered.hash(),
            original_hash,
            "execution-affecting attachments must change transaction identity"
        );
        tampered
            .verify_signature()
            .expect_err("an attachment mutation must invalidate authorization");
    }
}
