    #[test]
    fn block_payload_detects_transaction_blocks() {
        let block = block_with_transactions(2);
        assert!(block_counts_as_non_empty(&block));
    }

    #[test]
    fn block_payload_flags_genesis_without_transactions() {
        let block = empty_block(1);
        assert!(block_counts_as_non_empty(&block));
    }

    #[test]
    fn block_payload_rejects_non_genesis_empty_block() {
        let block = empty_block(2);
        assert!(!block_counts_as_non_empty(&block));
    }

    fn checked_block_signature(
        private_key: &PrivateKey,
        header: &iroha_data_model::block::BlockHeader,
    ) -> SignatureOf<iroha_data_model::block::BlockHeader> {
        SignatureOf::try_new(private_key, header).expect("test block signing should succeed")
    }

    #[test]
    fn block_payload_detects_da_commitment_blocks() {
        let block = block_with_da_commitments(2);
        assert!(block_counts_as_non_empty(&block));
    }

    fn empty_block(height: u64) -> iroha_data_model::block::SignedBlock {
        use std::num::NonZeroU64;

        use iroha_data_model::block::{BlockHeader, BlockSignature};

        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height must be > 0"),
            None,
            None,
            None,
            0,
            0,
        );
        let signer = checked_keypair();
        let signature =
            BlockSignature::new(0, checked_block_signature(signer.private_key(), &header));

        iroha_data_model::block::SignedBlock::presigned(signature, header, Vec::new())
    }

    fn block_with_da_commitments(height: u64) -> iroha_data_model::block::SignedBlock {
        use std::num::NonZeroU64;

        use iroha_crypto::{Hash, Signature};
        use iroha_data_model::{
            block::{BlockHeader, BlockSignature},
            da::{
                commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme},
                types::{BlobDigest, RetentionPolicy, StorageTicketId},
            },
            nexus::LaneId,
            sorafs::pin_registry::ManifestDigest,
        };

        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height must be > 0"),
            None,
            None,
            None,
            0,
            0,
        );
        let signer = checked_keypair();
        let signature =
            BlockSignature::new(0, checked_block_signature(signer.private_key(), &header));
        let mut block =
            iroha_data_model::block::SignedBlock::presigned(signature, header, Vec::new());

        let record = DaCommitmentRecord::new(
            LaneId::new(0),
            1,
            1,
            BlobDigest::new([0x11; 32]),
            ManifestDigest::new([0x22; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0x33; 32]),
            Some(Hash::prehashed([0x55; 32])),
            RetentionPolicy::default(),
            StorageTicketId::new([0x66; 32]),
            Signature::try_from_bytes(&[0x77; 64])
                .expect("checked telemetry DA acknowledgement signature fixture"),
        );
        let bundle = DaCommitmentBundle::new(vec![record]);
        block.set_da_commitments(Some(bundle));

        block
    }

    fn block_with_transactions(height: u64) -> iroha_data_model::block::SignedBlock {
        use std::num::NonZeroU64;

        use iroha_data_model::{
            ChainId,
            block::{BlockHeader, BlockSignature},
            transaction::signed::SignedTransaction,
        };

        fn dummy_transaction() -> SignedTransaction {
            let chain_id: ChainId = "test-chain".parse().expect("chain id");
            let key_pair = checked_keypair();
            let authority = AccountId::new(key_pair.public_key().clone());
            TransactionBuilder::new(
                chain_id,
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .sign(key_pair.private_key())
        }

        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height must be > 0"),
            None,
            None,
            None,
            0,
            0,
        );
        let signer = checked_keypair();
        let signature =
            BlockSignature::new(0, checked_block_signature(signer.private_key(), &header));
        let tx = dummy_transaction();

        iroha_data_model::block::SignedBlock::presigned(signature, header, vec![tx])
    }
