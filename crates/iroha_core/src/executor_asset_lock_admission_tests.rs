/// Certified Initial execution preserves asset-lock custody and lifecycle authorization.
mod asset_lock_admission {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::{
        asset::AssetBalancePolicy,
        escrow::{AssetEscrowStatus, EscrowId},
        isi::escrow::{CancelAssetLock, DrawdownAssetLock, ExpireAssetLock, OpenAssetLock},
    };

    struct Fixture {
        chain: CertifiedTestChain,
        keys: [KeyPair; 3],
        accounts: [AccountId; 3],
        definition: AssetDefinitionId,
        lock: EscrowId,
    }

    impl Fixture {
        fn new() -> Self {
            let keys = [0xA1, 0xA2, 0xA3]
                .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519));
            let accounts = keys
                .each_ref()
                .map(|key| AccountId::new(key.public_key().clone()));
            let domain = DomainId::try_new("locks", "universal").unwrap();
            let definition = AssetDefinitionId::derive_from_components(
                domain.clone(),
                "locked_token".parse().unwrap(),
            );
            let world = World::with_assets(
                [Domain::new(domain.clone()).build(&accounts[0])],
                accounts.iter().map(|id| Account::new(id.clone()).build(id)),
                [AssetDefinition::numeric(
                    definition.clone(),
                    "Locked token",
                    AssetBalancePolicy::Global,
                    Some(domain),
                )
                .build(&accounts[0])],
                [Asset::new(
                    AssetId::new(definition.clone(), accounts[0].clone()),
                    Quantity::from(100_u32),
                )],
                [],
            );
            Self {
                chain: CertifiedTestChain::start(TestChainConfig::new(world, 1_000)).unwrap(),
                keys,
                accounts,
                definition,
                lock: EscrowId::new(iroha_crypto::Hash::new(b"initial_asset_lock")),
            }
        }

        fn submit(&mut self, signer: usize, instruction: InstructionBox, expected: Option<&str>) {
            self.submit_at(
                signer,
                instruction,
                expected,
                (self.chain.height() + 1) * 1_000,
            );
        }

        fn submit_at(
            &mut self,
            signer: usize,
            instruction: InstructionBox,
            expected: Option<&str>,
            time: u64,
        ) {
            let transaction = self.chain.sign(&self.keys[signer], [instruction], time);
            let outcome = self.chain.commit_at(time, vec![transaction]);
            let committed = self.chain.committed(self.chain.height());
            let error = committed.block().output_error(0);
            if let Some(message) = expected {
                assert_eq!(outcome, [false], "{error:?}");
                assert!(format!("{error:?}").contains(message), "{error:?}");
            } else {
                assert_eq!(outcome, [true], "{error:?}");
            }
        }

        fn open(&mut self, release_authority: Option<AccountId>, expiry: Option<u64>) {
            self.submit(
                0,
                OpenAssetLock::with_options(
                    self.lock,
                    self.definition.clone(),
                    self.accounts[1].clone(),
                    60_u32,
                    release_authority,
                    expiry,
                    Vec::new(),
                )
                .into(),
                None,
            );
            self.assert_custody(40, 0, 60, AssetEscrowStatus::Locked);
        }

        fn assert_custody(
            &self,
            opener: u32,
            recipient: u32,
            remaining: u32,
            status: AssetEscrowStatus,
        ) {
            let view = self.chain.state().view();
            let world = view.world();
            let record = world
                .asset_escrows()
                .get(&self.lock)
                .expect("committed lock");
            assert_eq!(record.status, status);
            assert_eq!(record.remaining_amount, Quantity::from(remaining));
            for (account, expected) in [
                (&self.accounts[0], opener),
                (&self.accounts[1], recipient),
                (&record.custody, remaining),
            ] {
                assert_eq!(
                    initial_batch_balance(
                        world,
                        &AssetId::new(self.definition.clone(), account.clone())
                    ),
                    Quantity::from(expected)
                );
            }
            assert_eq!(
                opener + recipient + remaining,
                100,
                "custody conserves funds"
            );
        }
    }

    #[test]
    fn initial_asset_lock_drawdown_preserves_authority_and_exact_remaining_amount() {
        let mut fixture = Fixture::new();
        fixture.open(None, None);
        fixture.submit(
            2,
            DrawdownAssetLock::new(fixture.lock, 20_u32, 60_u32).into(),
            Some("only lock destination"),
        );
        fixture.assert_custody(40, 0, 60, AssetEscrowStatus::Locked);
        fixture.submit(
            1,
            DrawdownAssetLock::new(fixture.lock, 20_u32, 60_u32).into(),
            None,
        );
        fixture.assert_custody(40, 20, 40, AssetEscrowStatus::Locked);
        fixture.submit(
            1,
            DrawdownAssetLock::new(fixture.lock, 20_u32, 60_u32).into(),
            Some("remaining amount changed"),
        );
        fixture.submit(
            1,
            DrawdownAssetLock::new(fixture.lock, 41_u32, 40_u32).into(),
            Some("exceeds remaining amount"),
        );
        fixture.assert_custody(40, 20, 40, AssetEscrowStatus::Locked);
        fixture.submit(
            1,
            DrawdownAssetLock::new(fixture.lock, 40_u32, 40_u32).into(),
            None,
        );
        fixture.assert_custody(40, 60, 0, AssetEscrowStatus::DrawnDown);
        fixture.submit(
            0,
            CancelAssetLock::new(fixture.lock, 40_u32).into(),
            Some("only locked asset locks"),
        );
        fixture.assert_custody(40, 60, 0, AssetEscrowStatus::DrawnDown);
    }

    #[test]
    fn initial_asset_lock_release_authority_and_cancellation_preserve_custody() {
        let mut fixture = Fixture::new();
        fixture.open(Some(fixture.accounts[2].clone()), None);
        fixture.submit(
            1,
            DrawdownAssetLock::new(fixture.lock, 20_u32, 60_u32).into(),
            Some("only release authority"),
        );
        fixture.assert_custody(40, 0, 60, AssetEscrowStatus::Locked);
        fixture.submit(
            2,
            DrawdownAssetLock::new(fixture.lock, 20_u32, 60_u32).into(),
            None,
        );
        fixture.assert_custody(40, 20, 40, AssetEscrowStatus::Locked);
        fixture.submit(
            2,
            CancelAssetLock::new(fixture.lock, 40_u32).into(),
            Some("only lock opener"),
        );
        fixture.submit(
            0,
            CancelAssetLock::new(fixture.lock, 60_u32).into(),
            Some("remaining amount changed"),
        );
        fixture.assert_custody(40, 20, 40, AssetEscrowStatus::Locked);
        fixture.submit(0, CancelAssetLock::new(fixture.lock, 40_u32).into(), None);
        fixture.assert_custody(80, 20, 0, AssetEscrowStatus::Cancelled);
    }

    #[test]
    fn initial_asset_lock_expiry_requires_committed_deadline_and_refunds_opener() {
        let mut fixture = Fixture::new();
        fixture.open(None, Some(100_000));
        fixture.submit(
            2,
            ExpireAssetLock::new(fixture.lock).into(),
            Some("expiry has not been reached"),
        );
        fixture.assert_custody(40, 0, 60, AssetEscrowStatus::Locked);
        fixture.submit_at(2, ExpireAssetLock::new(fixture.lock).into(), None, 100_000);
        fixture.assert_custody(100, 0, 0, AssetEscrowStatus::Expired);
        fixture.submit_at(
            2,
            ExpireAssetLock::new(fixture.lock).into(),
            Some("only locked asset locks"),
            101_000,
        );
        fixture.assert_custody(100, 0, 0, AssetEscrowStatus::Expired);
    }
}
