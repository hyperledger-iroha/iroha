//! Genuine engineering wallet using the public simulator's exact release sync fault.

use super::{release_fault::ReleaseFaultFs, *};
use iroha_core_zk::kagemusha_wallet_advance_v1::{
    KagemushaWalletSimPowerLossV1 as PowerLoss, KagemushaWalletSimStepV1 as Step,
    KagemushaWalletSlotStatusV1 as SlotStatus,
};

pub(super) struct PowerLossDevice {
    root: PathBuf,
    fs: ReleaseFaultFs,
    platform: FakePlatformV1,
    fixture: Fixture,
    frames: [Vec<u8>; 4],
}

impl PowerLossDevice {
    fn provider(&self) -> state::NativeProviderV1<ReleaseFaultFs, FakePlatformV1> {
        state::NativeProviderV1::open(
            self.fs.clone(),
            self.platform.clone(),
            self.fixture.config.scheme.scheme_id(),
            test_options(),
        )
        .unwrap()
    }

    fn require_same_release(&self) {
        let (_, released) = self.fs.fired().expect("actual release crash fired");
        let mut provider = self.provider();
        let SlotStatus::Released(current) = provider.status(released.slot()).unwrap() else {
            panic!("durable release was lost or replaced");
        };
        assert_eq!(current.head(), released.head());
        assert_eq!(current.completion_digest(), released.completion_digest());
        assert_eq!(
            current.selected_generation(),
            released.selected_generation()
        );
    }
}

impl EngineDevice for PowerLossDevice {
    type Fs = ReleaseFaultFs;

    fn enroll(root: PathBuf, key_seed: u8, fixture: Fixture) -> Self {
        private_dir(&root);
        let mut device = Self {
            root,
            fs: ReleaseFaultFs::default(),
            platform: FakePlatformV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, key_seed),
            fixture,
            frames: Default::default(),
        };
        device.frames = engineering::enroll(device.provider(), &device.fixture);
        device
    }

    fn fixture(&self) -> &Fixture {
        &self.fixture
    }

    fn open(&self, sources: &Sources) -> EngineWallet<Self::Fs> {
        let runtime = NativeWalletRuntimeV1::new(
            self.provider(),
            Arc::clone(&sources.installed),
            Arc::clone(&sources.qualified),
            Arc::clone(&sources.genesis),
            DirectoryOriginalsV1::open_existing(&sources.originals, PROVING_KEY_MAX_BYTES_V1)
                .unwrap(),
            read_config(),
            MemoryBudget::DEFAULT,
        );
        let pending = runtime
            .begin(
                &self.frames[0],
                &self.frames[1],
                &self.frames[2],
                &self.frames[3],
            )
            .unwrap();
        let challenge = pending.challenge().to_vec();
        pending
            .finish(&engineering::sign(&self.fixture, &challenge))
            .unwrap()
    }

    fn execute_then_lose(
        &self,
        wallet: &mut EngineWallet<Self::Fs>,
        review: state::ReviewedSuffixUnloadV1,
    ) -> Option<Vec<u8>> {
        let expected_sequence = wallet.snapshot().unwrap().sequence + 1;
        self.fs
            .arm(self.fixture.config.scheme.scheme_id(), expected_sequence);
        match wallet.execute_reviewed_suffix_unload(review) {
            Err(failure) => {
                let (_, retained) = failure.into_parts();
                assert!(
                    retained.is_none(),
                    "released debit cannot return a new review"
                );
            }
            Ok(state::SuffixUnloadExecutionV1::Pending) => {}
            Ok(state::SuffixUnloadExecutionV1::Released(_)) => {
                panic!("release publication escaped the injected process crash");
            }
        }
        let (step, released) = self
            .fs
            .fired()
            .expect("exact release retirement sync reached");
        assert!(self.fs.simulation().crashed());
        assert_eq!(released.head().unwrap().0, expected_sequence);
        assert_eq!(self.fs.simulation().trace_since(step), vec![Step::SyncDir]);
        // Preserve the observed real marker and public fault location for diagnosis. These
        // are outputs only; the fixture never installs them as provider authority.
        fs::write(
            self.root.join("released-at-fault.norito"),
            released.file_bytes(),
        )
        .unwrap();
        fs::write(self.root.join("release-fault-step.txt"), step.to_string()).unwrap();
        None
    }

    fn restart(&self) {
        assert!(self.fs.simulation().crashed());
        self.fs.simulation().power_loss(PowerLoss::DropUnsynced);
        self.require_same_release();
    }

    fn assert_recovered_release(&self) {
        // Coordinator StepEntry/suffix publication may advance archive metadata generations,
        // but neither operation, capsule, original Selected generation nor completion changes.
        self.require_same_release();
    }
}
