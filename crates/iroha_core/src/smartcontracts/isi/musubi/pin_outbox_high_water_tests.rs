// Actual signed native pin-outbox execution over a genuine certified four-validator chain.

mod pin_outbox_native {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        IntoKeyValue, Registrable,
        account::Account,
        transaction::{ExecutableBatchItem, SignedTransaction, TransactionBuilder},
    };

    fn key(seed: u8) -> KeyPair {
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
    }
    fn owner() -> AccountId {
        AccountId::new(key(0xa1).public_key().clone())
    }

    struct Fixture {
        chain: CertifiedTestChain,
    }
    impl Fixture {
        fn new() -> Self {
            let mut world = World::new();
            for seed in [0xa1, 0xa2] {
                let id = AccountId::new(key(seed).public_key().clone());
                let (id, account) = Account::new(id).build(&owner()).into_key_value();
                world.accounts.insert(id, account);
            }
            Self {
                chain: CertifiedTestChain::start(TestChainConfig::new(world, 1_000)).unwrap(),
            }
        }
        fn advance(
            &self,
            revision: u64,
            before: [u8; 32],
            after: [u8; 32],
        ) -> AdvanceMusubiPinOutboxV1 {
            AdvanceMusubiPinOutboxV1 {
                network_id: self.chain.network_id(),
                pin_authority: owner(),
                session_id: [0xb1; 32],
                expected_revision: revision,
                expected_inventory_digest: before,
                inventory_digest: after,
            }
        }
        fn check(&self, expected: MusubiPinOutboxCheckExpectationV1) -> CheckMusubiPinOutboxV1 {
            let floor = self.chain.committed(self.chain.height());
            CheckMusubiPinOutboxV1 {
                network_id: self.chain.network_id(),
                pin_authority: owner(),
                session_id: [0xb1; 32],
                inventory_digest: [0xc1; 32],
                challenge: [0xc7; 32],
                floor: MusubiPinOutboxCheckFloorV1 {
                    height: floor.height(),
                    block_hash: *floor.block_hash().as_ref(),
                    context_id: floor.id(),
                },
                expected,
            }
        }
        fn sign(&self, seed: u8, instructions: Vec<InstructionBox>) -> SignedTransaction {
            self.chain.sign(
                &key(seed),
                instructions,
                self.chain.committed(self.chain.height()).block_time_ms() + 1,
            )
        }
        fn commit(&mut self, instruction: impl Into<InstructionBox>) -> bool {
            let signed = self.sign(0xa1, vec![instruction.into()]);
            self.chain.commit(vec![signed])[0]
        }
        fn row(&self) -> Option<MusubiPinOutboxHighWaterV1> {
            self.chain
                .state()
                .view()
                .world
                .musubi_pin_outbox_high_waters
                .get(&owner())
                .cloned()
        }
    }

    #[test]
    fn pin_outbox_high_water_requires_signed_owner_network_and_contiguous_predecessor() {
        let mut fixture = Fixture::new();
        let first = fixture.advance(0, [0; 32], [0xc1; 32]);
        let stranger = fixture.sign(0xa2, vec![first.clone().into()]);
        assert!(!fixture.chain.commit(vec![stranger])[0]);
        let mut wrong_network = first.clone();
        wrong_network.network_id = iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::from_untyped_unchecked(Hash::new([0xe1; 32])),
        );
        assert!(!fixture.commit(wrong_network));
        assert!(fixture.row().is_none());
        let original = fixture.sign(0xa1, vec![first.clone().into()]);
        let signed_hash = original.hash();
        assert!(fixture.chain.commit(vec![original])[0]);
        let row = fixture.row().unwrap();
        assert_eq!(row.revision, 1);
        assert_eq!(row.recorded_at_height, fixture.chain.height());
        assert_eq!(row.transaction_hash, *signed_hash.as_ref());
        assert!(!fixture.commit(first));
        assert!(!fixture.commit(fixture.advance(1, [0xee; 32], [0xc2; 32])));
        let mut switched = fixture.advance(1, [0xc1; 32], [0xc2; 32]);
        switched.session_id = [0xb2; 32];
        assert!(!fixture.commit(switched));
        assert_eq!(fixture.row(), Some(row));
        assert!(fixture.commit(fixture.advance(1, [0xc1; 32], [0xc2; 32])));
        let successor = fixture.row().unwrap();
        assert_eq!(successor.revision, 2);
        assert_eq!(successor.inventory_digest, [0xc2; 32]);
    }

    #[test]
    fn pin_outbox_check_certifies_absence_then_entire_present_row_without_mutation() {
        let mut fixture = Fixture::new();
        let absence = fixture.check(MusubiPinOutboxCheckExpectationV1::Absent);
        assert!(fixture.commit(absence));
        assert!(fixture.row().is_none());
        assert!(fixture.commit(fixture.advance(0, [0; 32], [0xc1; 32])));
        let row = fixture.row().unwrap();
        // A recent successful Check need not reread the advance's old frame after it has
        // compared the complete committed row. Its current native floor authenticates the cut.
        for _ in 0..5 {
            fixture.chain.commit(Vec::new());
        }
        let present = fixture.check(MusubiPinOutboxCheckExpectationV1::Present(row.clone()));
        let original = fixture.sign(0xa1, vec![present.into()]);
        assert!(fixture.chain.commit(vec![original.clone()])[0]);
        let applied = fixture.chain.committed(fixture.chain.height());
        assert_eq!(
            applied.block().network_entrypoint_at(0).unwrap(),
            &iroha_data_model::transaction::TransactionEntrypoint::External(original)
        );
        assert!(
            applied
                .block()
                .network_output_at(0)
                .unwrap()
                .1
                .result
                .is_ok()
        );
        assert_eq!(fixture.row(), Some(row));
        let mut wrong_session_absence = fixture.check(MusubiPinOutboxCheckExpectationV1::Absent);
        wrong_session_absence.session_id = [0x81; 32];
        wrong_session_absence.inventory_digest = [0x82; 32];
        assert!(!fixture.commit(wrong_session_absence));
    }

    #[test]
    fn pin_outbox_check_rejects_every_full_row_and_floor_substitution() {
        let mut fixture = Fixture::new();
        assert!(fixture.commit(fixture.advance(0, [0; 32], [0xc1; 32])));
        let original = fixture.row().unwrap();
        for field in 0..8 {
            let mut row = original.clone();
            match field {
                0 => row.version = 0,
                1 => {
                    row.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                        HashOf::from_untyped_unchecked(Hash::new([0xe2; 32])),
                    )
                }
                2 => row.pin_authority = AccountId::new(key(0xa2).public_key().clone()),
                3 => row.session_id = [0xe3; 32],
                4 => row.revision += 1,
                5 => row.inventory_digest = [0xe5; 32],
                6 => row.recorded_at_height += 1,
                7 => row.transaction_hash = [0xe7; 32],
                _ => unreachable!(),
            }
            let mut check = fixture.check(MusubiPinOutboxCheckExpectationV1::Present(row.clone()));
            check.session_id = row.session_id;
            check.inventory_digest = row.inventory_digest;
            assert!(!fixture.commit(check), "substituted row field {field}");
            assert_eq!(fixture.row(), Some(original.clone()));
        }
        for field in 0..4 {
            let mut check =
                fixture.check(MusubiPinOutboxCheckExpectationV1::Present(original.clone()));
            match field {
                0 => check.floor.height = fixture.chain.height() + 1,
                1 => check.floor.block_hash = [0xe1; 32],
                2 => check.floor.context_id = fixture.chain.committed(1).id(),
                3 => check.challenge = [0; 32],
                _ => unreachable!(),
            }
            assert!(!fixture.commit(check), "substituted floor field {field}");
            assert_eq!(fixture.row(), Some(original.clone()));
        }
    }

    #[test]
    fn pin_outbox_native_operations_reject_mixed_and_wrapped_batches() {
        let mut fixture = Fixture::new();
        for instruction in [
            InstructionBox::from(fixture.advance(0, [0; 32], [0xc1; 32])),
            fixture
                .check(MusubiPinOutboxCheckExpectationV1::Absent)
                .into(),
        ] {
            let mixed = fixture.sign(
                0xa1,
                vec![
                    instruction.clone(),
                    iroha_data_model::isi::Log::new(
                        iroha_logger::Level::INFO,
                        "mixed native outbox".to_owned(),
                    )
                    .into(),
                ],
            );
            assert!(!fixture.chain.commit(vec![mixed])[0]);
            let ordinary = fixture.sign(0xa1, vec![instruction.clone()]);
            let wrapped = TransactionBuilder::from_payload(ordinary.payload().clone())
                .unwrap()
                .with_executable_batch([ExecutableBatchItem::Instruction(instruction)])
                .sign(key(0xa1).private_key());
            assert!(!fixture.chain.commit(vec![wrapped])[0]);
            assert!(fixture.row().is_none());
        }
    }
}
