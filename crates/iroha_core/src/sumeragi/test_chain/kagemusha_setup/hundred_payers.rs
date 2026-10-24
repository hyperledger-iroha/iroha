//! One hundred payers pay one merchant concurrently with genuine native proofs.
//!
//! One process, ignored (hours of proving). The certified component chain registers and funds
//! the payers, installs the engineering scheme and executes every Activate and IssueLoad.
//! Every wallet is a real native coordinator over host custody with its own simulated hardware
//! key and the engineering issuer, admitted from a signed engineering catalog compiled for this
//! exact genesis (`KAGEMUSHA_WALLET_SIGNED_CATALOG`). Payers Send while the merchant's fold
//! runs; every Receive passes native verification and Receive admission (design §3.2).
//! Asserted: each accepted Payment is credited exactly once and replays exactly, every credit
//! folds unburned, the folded total is the sum, value is conserved and the merchant can spend it
//! onward. Engineering evidence only: no physical device, attestation or network-round claim.

use super::*;
use crate::kagemusha_wallet_v1::CommittedLoadReceipts;
use iroha_core_zk::{
    kagemusha_wallet_advance_v1::{
        KagemushaWalletAnchorPolicyV1, KagemushaWalletProviderV1, KagemushaWalletStdFsV1,
        engineering_fixture::{self as engineering, Fixture},
        test_support::{FakePlatformV1, test_options},
    },
    kagemusha_wallet_artifacts_v1::{
        InstallationV1, InstalledVerifierPackV1,
        producer_inventory::{
            BlobV1, DirectoryOriginalsV1, PROVING_KEY_MAX_BYTES_V1, QualifiedWalletSourcesV1,
        },
    },
    kagemusha_wallet_state_v1::{
        self as state, Completion, FoldStatus, NativeWalletCoordinatorV1, NativeWalletRuntimeV1,
        OperationActionV1, OperationRequestV1,
    },
};
use iroha_data_model::{
    asset::AssetBalanceScope,
    isi::{
        Transfer,
        kagemusha_wallet::{KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1},
    },
    kagemusha::*,
    sumeragi_finality::{
        CertifiedTransactionInclusionV1, SumeragiCommitCertificateV1, VerifiedSumeragiBlock,
    },
};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::keys::pk::{CosetCachePolicy, artifact::ReadConfig};
use std::{
    collections::BTreeMap,
    num::{NonZeroU16, NonZeroU64},
    ops::ControlFlow,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

const DEFAULT_PAYERS: usize = 100;
const FIRST_PAYER_SEED: u8 = 100;
/// Account 42 is registered by the original genesis.
const MERCHANT_SEED: u8 = 42;
/// Simulated hardware key seeds: payers 1..=N, merchant 250.
const MERCHANT_KEY_SEED: u8 = 250;
/// Atomic units (scale 2): each payer is funded 5, loads 3 and pays the merchant 2.
const FUNDING: u128 = 5;
const LOAD: u128 = 3;
const PAY: u128 = 2;
/// Concurrent payer proving jobs (setup and Sends); the merchant proves its Receives and fold
/// as one device, outside these slots.
const PROVERS: usize = 6;
/// Ledger transactions per block for Activate and IssueLoad batches, and instructions per
/// transaction for registration and funding.
const BATCH: usize = 10;

type Wallet =
    NativeWalletCoordinatorV1<KagemushaWalletStdFsV1, FakePlatformV1, DirectoryOriginalsV1>;

fn payers() -> usize {
    std::env::var("KAGEMUSHA_HUNDRED_PAYERS_COUNT")
        .map(|count| count.parse().expect("payer count"))
        .unwrap_or(DEFAULT_PAYERS)
}

fn read_config() -> ReadConfig {
    ReadConfig {
        maximum_bytes: PROVING_KEY_MAX_BYTES_V1,
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: MemoryBudget::DEFAULT,
    }
}

fn private_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

/// The signed engineering catalog, admitted through the ordinary production constructor.
struct Sources {
    installed: Arc<InstalledVerifierPackV1>,
    qualified: Arc<QualifiedWalletSourcesV1>,
    genesis: Arc<SumeragiFinalityVerifier>,
    originals: PathBuf,
    installation: InstallationV1,
}

impl Sources {
    fn admit(setup: &ExecutedKagemushaSetup, output: &Path) -> Self {
        let catalog = PathBuf::from(
            std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG")
                .expect("signed engineering catalog compiled for this genesis"),
        );
        let pack = fs::read(catalog.join("engineering-verifier-pack.norito")).unwrap();
        let (scheme_id, manifest_digest): ([u8; 32], [u8; 32]) = norito::decode_from_bytes(
            &fs::read(catalog.join("engineering-installation.norito")).unwrap(),
        )
        .unwrap();
        let installation = InstallationV1 {
            scheme_id,
            manifest_digest,
        };
        let installed = InstalledVerifierPackV1::load(&pack, installation).unwrap();
        assert_eq!(
            &installed.verifier().scheme().network_id,
            setup.chain.network_id().as_bytes(),
            "the catalog must be compiled for this exact signed genesis"
        );
        let inventory = fs::read(catalog.join("producer-inventory.norito")).unwrap();
        let authenticated = installed
            .authenticate_producer_inventory(&inventory)
            .unwrap();
        // Copy every exact original into fresh owner-private sealed custody.
        let originals = output.join("originals");
        private_dir(&originals);
        let mut sealed =
            DirectoryOriginalsV1::open_existing(&originals, PROVING_KEY_MAX_BYTES_V1).unwrap();
        for file in fs::read_dir(catalog.join("originals")).unwrap() {
            let bytes = fs::read(file.unwrap().path()).unwrap();
            sealed.store_original(BlobV1::of(&bytes), &bytes).unwrap();
        }
        let genesis = native(setup);
        let qualified = authenticated
            .qualify_wallet(&installed, &genesis, &mut sealed, read_config())
            .unwrap();
        Self {
            installed: Arc::new(installed),
            qualified: Arc::new(qualified),
            genesis: Arc::new(genesis),
            originals,
            installation,
        }
    }

    fn fixture(&self, asset: &KagemushaWalletAssetScopeV1, seed: u8) -> Fixture {
        let mut fixture =
            engineering::installed_fixture(*self.installed.verifier().scheme(), self.installation);
        engineering::bind_identity(
            &mut fixture,
            AccountId::new(key(seed).public_key().clone()),
            asset.clone(),
            seed,
        );
        fixture
    }
}

/// One wallet: its custody root, its own simulated hardware and its enrollment fixture.
struct Device {
    root: PathBuf,
    platform: FakePlatformV1,
    fixture: Fixture,
    frames: [Vec<u8>; 4],
}

impl Device {
    fn provider(
        &self,
        scheme: [u8; 32],
    ) -> KagemushaWalletProviderV1<KagemushaWalletStdFsV1, FakePlatformV1> {
        KagemushaWalletProviderV1::open(
            KagemushaWalletStdFsV1::open(self.root.join("custody")).unwrap(),
            self.platform.clone(),
            scheme,
            test_options(),
        )
        .unwrap()
    }

    fn enroll(root: PathBuf, key_seed: u8, fixture: Fixture) -> Self {
        private_dir(&root);
        private_dir(&root.join("custody"));
        let mut device = Self {
            root,
            platform: FakePlatformV1::new(KagemushaWalletAnchorPolicyV1::NotRequired, key_seed),
            fixture,
            frames: Default::default(),
        };
        let scheme = device.fixture.config.scheme.scheme_id();
        device.frames = engineering::enroll(device.provider(scheme), &device.fixture);
        device
    }

    fn open(&self, sources: &Sources) -> Wallet {
        let runtime = NativeWalletRuntimeV1::new(
            self.provider(self.fixture.config.scheme.scheme_id()),
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
}

/// Counting semaphore bounding resident proving work.
struct Slots {
    free: Mutex<usize>,
    released: Condvar,
}

impl Slots {
    fn new(count: usize) -> Self {
        Self {
            free: Mutex::new(count),
            released: Condvar::new(),
        }
    }

    fn run<T>(&self, work: impl FnOnce() -> T) -> T {
        let mut free = self.free.lock().unwrap();
        while *free == 0 {
            free = self.released.wait(free).unwrap();
        }
        *free -= 1;
        drop(free);
        let result = work();
        *self.free.lock().unwrap() += 1;
        self.released.notify_one();
        result
    }
}

fn complete(result: Completion) -> Vec<u8> {
    let Completion::Complete(bytes) = result else {
        panic!("not a durable completion: {result:?}")
    };
    bytes
}

fn action(request_id: [u8; 32], action: OperationActionV1) -> OperationRequestV1 {
    OperationRequestV1 { request_id, action }
}

fn id(tag: u8, index: usize) -> [u8; 32] {
    let mut id = [tag; 32];
    id[..8].copy_from_slice(&(index as u64).to_le_bytes());
    id
}

/// Fold to the current head with the coordinator held for each whole sub-proof.
fn fold(wallet: &mut Wallet) {
    wallet.scheduler().set_activity(true, false);
    for _ in 0..1 << 16 {
        match wallet.fold_once().unwrap() {
            FoldStatus::CaughtUp => {
                let snapshot = wallet.snapshot().unwrap();
                assert_eq!(snapshot.fold_backlog, 0);
                assert_eq!(snapshot.folded_balance, Some(snapshot.owned_balance));
                return;
            }
            FoldStatus::Checkpoint { .. } | FoldStatus::Folded(_) => {}
            status @ FoldStatus::Idle => panic!("unexpected fold status {status:?}"),
        }
    }
    panic!("bounded native fold schedule did not complete");
}

/// The executed chain plus its genesis-rooted verified history.
struct Ledger {
    setup: ExecutedKagemushaSetup,
    verifier: SumeragiFinalityVerifier,
    verified: BTreeMap<u64, VerifiedSumeragiBlock>,
}

impl Ledger {
    fn new(setup: ExecutedKagemushaSetup) -> Self {
        let verifier = native(&setup);
        let mut ledger = Self {
            setup,
            verifier,
            verified: BTreeMap::new(),
        };
        ledger.verify_to(ledger.setup.chain.height());
        ledger
    }

    fn verify_to(&mut self, height: u64) {
        let mut next = self.verified.keys().next_back().map_or(1, |last| last + 1);
        while next <= height {
            let proof =
                crate::sumeragi::finality::build_proof(&self.setup.chain.state().view(), next)
                    .unwrap();
            self.verified
                .insert(next, self.verifier.verify(&proof).unwrap());
            next += 1;
        }
    }

    /// Sign with each key at the next height's time and require every transaction to execute.
    fn commit(&mut self, signed: Vec<SignedTransaction>) -> u64 {
        let height = self.setup.chain.height() + 1;
        let results = self.setup.chain.commit_at(height * 1_000, signed.clone());
        assert_eq!(
            results,
            vec![true; signed.len()],
            "every transaction must execute"
        );
        self.verify_to(height);
        height
    }

    fn sign(&self, seed: u8, instructions: Vec<InstructionBox>) -> SignedTransaction {
        let created = (self.setup.chain.height() + 1) * 1_000 - 1;
        self.setup.chain.sign(&key(seed), instructions, created)
    }

    /// Canonical commit certificates of every verified height after genesis, for wallets'
    /// epoch synchronization on worker threads (the chain itself stays on this thread).
    fn certificates(&self) -> BTreeMap<u64, Vec<u8>> {
        self.verified
            .iter()
            .filter(|(height, _)| **height > 1)
            .map(|(height, block)| {
                (
                    *height,
                    SumeragiCommitCertificateV1::from_verified(block)
                        .unwrap()
                        .to_canonical_bytes()
                        .unwrap(),
                )
            })
            .collect()
    }

    fn activation_finality(&self, height: u64, signed: &SignedTransaction) -> Vec<u8> {
        let block = &self.verified[&height];
        let entrypoint_hash = signed.hash_as_entrypoint();
        let certified = block.block();
        let index = certified
            .network_entrypoints()
            .position(|entry| entry.hash() == entrypoint_hash)
            .unwrap() as u32;
        let (output_index, _) = certified.network_output_at(index).unwrap();
        KagemushaWalletLedgerFinalityV1 {
            version: 1,
            entrypoint_hash,
            certificate: SumeragiCommitCertificateV1::from_verified(block).unwrap(),
            inclusion: CertifiedTransactionInclusionV1 {
                entry_proof: certified.network_input_proof(index).unwrap(),
                output: certified.execution_outputs()[output_index as usize].clone(),
                output_proof: certified.output_proof(output_index).unwrap(),
            },
        }
        .to_canonical_bytes()
        .unwrap()
    }

    /// Canonical receipt and BLS finality of one executed IssueLoad at `height`.
    fn load_evidence(
        &self,
        account: &AccountId,
        scheme: &[u8; 32],
        wallet: &[u8; 32],
        request: &[u8; 32],
        height: u64,
    ) -> (Vec<u8>, Vec<u8>) {
        let mut cursor =
            crate::sumeragi::finality::NativeFinalityCursorV1::new(self.setup.chain.root_scope());
        let budget = iroha_allocation::AllocationBudget::new(2 << 30);
        let view = self.setup.chain.state().view();
        let at = cursor
            .advance_to_height(
                &view,
                NonZeroU64::new(height).unwrap(),
                &budget,
                Instant::now() + Duration::from_secs(60),
                NonZeroU16::new(64).unwrap(),
            )
            .unwrap()
            .unwrap();
        let source = CommittedLoadReceipts::new(
            &view,
            2_000_000,
            norito::canonical_decode_limits(2_000_000),
        )
        .unwrap();
        let original = source
            .event_evidence_for(&at, account, scheme, wallet, request)
            .unwrap();
        let receipt = *original.verified().receipt();
        let evidence = KagemushaWalletLoadFinalityV1 {
            version: 1,
            receipt_digest: receipt.receipt_digest().unwrap(),
            certificate: SumeragiCommitCertificateV1::from_verified(at.block()).unwrap(),
            event_proof: original.path().proof().unwrap(),
        };
        evidence.verify(&native(&self.setup), &receipt).unwrap();
        (
            norito::encode_canonical(&receipt).unwrap(),
            norito::encode_canonical(&evidence).unwrap(),
        )
    }

    fn balance(&self, seed: u8) -> Quantity {
        self.setup
            .chain
            .state()
            .view()
            .world()
            .assets()
            .get(&AssetId::of(
                self.setup.asset.asset.clone(),
                AccountId::new(key(seed).public_key().clone()),
            ))
            .map_or_else(Quantity::zero, |value| value.as_ref().clone())
    }
}

/// Advance the wallet's genesis-rooted epoch cursor until it authorizes `height`.
fn sync_epochs(wallet: &mut Wallet, certificates: &BTreeMap<u64, Vec<u8>>, height: u64) {
    loop {
        let current = wallet.epoch_progress().unwrap();
        if height <= current.boundary_height {
            return;
        }
        let boundary = certificates
            .get(&current.boundary_height)
            .expect("verified epoch boundary certificate");
        let next = wallet
            .ingest_epoch_boundary(current.epoch, boundary)
            .unwrap();
        assert!(next.epoch > current.epoch);
    }
}

fn quantity(units: u128) -> Quantity {
    Quantity::from_canonical_numeric(Numeric::new(units, SCALE)).unwrap()
}

struct Payment {
    index: usize,
    payment: Vec<u8>,
    companion: Vec<u8>,
    received: Vec<u8>,
    credit_id: [u8; 32],
    digest: [u8; 32],
    send_ms: u128,
    receive_ms: u128,
}

fn receive(payment: &[u8], companion: &[u8], request_id: [u8; 32]) -> OperationRequestV1 {
    let companion = KagemushaWalletPayerCompanionV1::decode_canonical(companion).unwrap();
    action(
        request_id,
        OperationActionV1::Receive {
            payment: payment.to_vec(),
            payer_credential: companion.payer_credential.to_canonical_bytes().unwrap(),
            certificates: norito::encode_canonical(&companion.certificates).unwrap(),
        },
    )
}

#[test]
#[ignore = "requires KAGEMUSHA_WALLET_SIGNED_CATALOG compiled for this genesis; hours of genuine proving"]
#[allow(clippy::too_many_lines)]
fn hundred_payers_pay_one_merchant_and_every_accepted_payment_folds_unburned() {
    let count = payers();
    assert!((1..=150).contains(&count));
    let began = Instant::now();
    let output = PathBuf::from(
        std::env::var_os("KAGEMUSHA_HUNDRED_PAYERS_OUTPUT").unwrap_or_else(|| {
            std::env::temp_dir()
                .join(format!("kagemusha-hundred-payers-{}", std::process::id()))
                .into_os_string()
        }),
    );
    assert!(!output.exists(), "fresh exclusive output");
    private_dir(&output);
    let mut phases = Vec::new();
    let mut phase = |name: &str, since: Instant| {
        eprintln!(
            "HUNDRED_PAYERS_PHASE {name} ms={}",
            since.elapsed().as_millis()
        );
        phases.push(norito::json!({"phase":name,"ms":(since.elapsed().as_millis().to_string())}));
    };

    // Chain: payer accounts, funding from genesis account 41, scheme registration and pack.
    let at = Instant::now();
    let mut ledger = Ledger::new(start());
    let seeds: Vec<u8> = (0..count).map(|i| FIRST_PAYER_SEED + i as u8).collect();
    // This chain admits at most 16 instructions per transaction: register and fund the
    // payers `BATCH` at a time, one transaction per block.
    for chunk in seeds.chunks(BATCH) {
        let register = chunk
            .iter()
            .map(|seed| {
                Register::account(Account::new(AccountId::new(
                    key(*seed).public_key().clone(),
                )))
                .into()
            })
            .collect();
        let transaction = ledger.sign(0xCE, register);
        ledger.commit(vec![transaction]);
    }
    for chunk in seeds.chunks(BATCH) {
        let transfers = chunk
            .iter()
            .map(|seed| {
                Transfer::asset_quantity(
                    AssetId::of(
                        ledger.setup.asset.asset.clone(),
                        ledger.setup.account.clone(),
                    ),
                    quantity(FUNDING),
                    AccountId::new(key(*seed).public_key().clone()),
                )
                .into()
            })
            .collect();
        let transaction = ledger.sign(41, transfers);
        ledger.commit(vec![transaction]);
    }
    let sources = Sources::admit(&ledger.setup, &output);
    let scheme = *sources.installed.verifier().scheme();
    let scheme_id = scheme.scheme_id();
    let register = KagemushaWalletLedgerV1::new(
        scheme_id,
        Action::Register {
            scheme: scheme.to_canonical_bytes().unwrap(),
            asset: norito::encode_canonical(&ledger.setup.asset).unwrap(),
            reserve: ledger.setup.reserve.clone(),
            balance_scope: AssetBalanceScope::Global,
        },
    );
    let transaction = ledger.sign(95, vec![register.into()]);
    ledger.commit(vec![transaction]);
    let install = KagemushaWalletLedgerV1::new(
        scheme_id,
        Action::InstallVerifierPack {
            asset: ledger.setup.asset.asset_digest(),
            manifest_digest: sources.installation.manifest_digest,
            pack: fs::read(
                PathBuf::from(std::env::var_os("KAGEMUSHA_WALLET_SIGNED_CATALOG").unwrap())
                    .join("engineering-verifier-pack.norito"),
            )
            .unwrap(),
        },
    );
    let transaction = ledger.sign(95, vec![install.into()]);
    ledger.commit(vec![transaction]);
    phase("chain_setup_and_catalog_admission", at);

    // Enrollment, opening and Bootstrap of every wallet; the merchant is wallet 0.
    let at = Instant::now();
    let slots = Arc::new(Slots::new(PROVERS));
    let asset = ledger.setup.asset.clone();
    let enrolled = Mutex::new(BTreeMap::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..PROVERS {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index > count {
                        return;
                    }
                    let (seed, key_seed) = if index == 0 {
                        (MERCHANT_SEED, MERCHANT_KEY_SEED)
                    } else {
                        (seeds[index - 1], index as u8)
                    };
                    let device = Device::enroll(
                        output.join(format!("wallet-{index:03}")),
                        key_seed,
                        sources.fixture(&asset, seed),
                    );
                    enrolled.lock().unwrap().insert(index, device);
                }
            });
        }
    });
    let devices: Vec<Device> = enrolled.into_inner().unwrap().into_values().collect();
    assert_eq!(devices.len(), count + 1);
    let wallets: Vec<Mutex<Wallet>> = devices
        .iter()
        .map(|device| Mutex::new(device.open(&sources)))
        .collect();
    std::thread::scope(|scope| {
        for index in 0..wallets.len() {
            let (wallets, slots) = (&wallets, &slots);
            scope.spawn(move || {
                slots.run(|| {
                    let mut wallet = wallets[index].lock().unwrap();
                    complete(wallet.bootstrap().unwrap());
                    fold(&mut wallet);
                });
            });
        }
    });
    phase("enroll_open_bootstrap_fold", at);

    // Payers: Activate on chain, confirm natively, IssueLoad, Load and fold.
    let at = Instant::now();
    let mut activations = Vec::new();
    for index in 1..=count {
        let mut wallet = wallets[index].lock().unwrap();
        let activation = wallet.activation().unwrap();
        let instruction = KagemushaWalletLedgerV1::new(scheme_id, Action::Activate(activation));
        let signed = ledger.sign(seeds[index - 1], vec![instruction.into()]);
        let wire = signed.encode_wire_v1().unwrap();
        // Native retains the exact signed Activate before it is dispatched.
        assert!(matches!(
            wallet.retain_activation_attempt(&wire).unwrap(),
            state::LedgerAttemptOutcomeV1::Awaiting { .. }
        ));
        activations.push((index, signed, wire));
    }
    for batch in activations.chunks(BATCH) {
        let height = ledger.commit(batch.iter().map(|(_, signed, _)| signed.clone()).collect());
        let certificates = ledger.certificates();
        for (index, signed, wire) in batch {
            let mut wallet = wallets[*index].lock().unwrap();
            sync_epochs(&mut wallet, &certificates, height);
            let finality = ledger.activation_finality(height, signed);
            assert!(matches!(
                wallet.ingest_activation_finality(wire, &finality).unwrap(),
                state::LedgerAttemptIngestV1::Outcome(state::LedgerAttemptOutcomeV1::Confirmed(_))
            ));
        }
    }
    let mut loads = Vec::new();
    for index in 1..=count {
        let instruction = wallets[index]
            .lock()
            .unwrap()
            .prepare_ledger_load(id(201, index), LOAD)
            .unwrap();
        let instruction: KagemushaWalletLedgerV1 = norito::decode_canonical_with_limits(
            &instruction,
            norito::canonical_decode_limits(instruction.len()),
        )
        .unwrap();
        loads.push((
            index,
            ledger.sign(seeds[index - 1], vec![instruction.into()]),
        ));
    }
    let mut evidence = BTreeMap::new();
    for batch in loads.chunks(BATCH) {
        let height = ledger.commit(batch.iter().map(|(_, signed)| signed.clone()).collect());
        for (index, _) in batch {
            let wallet = wallets[*index]
                .lock()
                .unwrap()
                .snapshot()
                .unwrap()
                .wallet_id;
            let account = AccountId::new(key(seeds[index - 1]).public_key().clone());
            evidence.insert(
                *index,
                (
                    height,
                    ledger.load_evidence(&account, &scheme_id, &wallet, &id(201, *index), height),
                ),
            );
        }
    }
    for seed in &seeds {
        assert_eq!(ledger.balance(*seed), quantity(FUNDING - LOAD));
    }
    let certificates = ledger.certificates();
    std::thread::scope(|scope| {
        for index in 1..=count {
            let (wallets, slots, certificates, evidence) =
                (&wallets, &slots, &certificates, &evidence);
            scope.spawn(move || {
                let (height, (receipt, finality)) = evidence[&index].clone();
                slots.run(|| {
                    let mut wallet = wallets[index].lock().unwrap();
                    sync_epochs(&mut wallet, certificates, height);
                    let load = action(
                        id(202, index),
                        OperationActionV1::Load { receipt, finality },
                    );
                    complete(wallet.execute(load).unwrap());
                    assert_eq!(wallet.snapshot().unwrap().owned_balance, LOAD);
                    fold(&mut wallet);
                });
            });
        }
    });
    phase("activate_load_fold", at);

    // The merchant issues one open Request per sale, before any payer is known.
    let requests: Vec<Vec<u8>> = {
        let mut merchant = wallets[0].lock().unwrap();
        (1..=count)
            .map(|index| merchant.request(id(211, index), PAY, None).unwrap())
            .collect()
    };

    // Payments: payers Send concurrently; the merchant receives each one while its fold runs
    // in the background, proving outside the coordinator lock exactly as the apps do.
    let at = Instant::now();
    let delivered = AtomicBool::new(false);
    let (published, cancelled) = (AtomicU64::new(0), AtomicU64::new(0));
    let arrivals = Mutex::new(None);
    let next = std::sync::atomic::AtomicUsize::new(1);
    let collected = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let folder = scope.spawn(|| {
            loop {
                let prepared = {
                    let mut merchant = wallets[0].lock().unwrap();
                    merchant.scheduler().set_activity(true, false);
                    merchant.fold_prepare().unwrap()
                };
                match prepared {
                    ControlFlow::Continue(job) => {
                        // The merchant is one device: its fold proves at once, never behind a
                        // payer slot. A prepared job holds the scheduler until it runs, and a
                        // Receive waits for it to observe cancellation.
                        let proved = job.prove();
                        let outcome = proved
                            .and_then(|proved| wallets[0].lock().unwrap().fold_publish(proved));
                        match outcome {
                            Ok(_) => published.fetch_add(1, Ordering::Relaxed),
                            Err(state::Error::Cancelled) => {
                                cancelled.fetch_add(1, Ordering::Relaxed)
                            }
                            Err(error) => panic!("merchant fold failed: {error:?}"),
                        };
                    }
                    ControlFlow::Break(status) => {
                        if delivered.load(Ordering::Acquire)
                            && status.status == FoldStatus::CaughtUp
                        {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        });
        let workers: Vec<_> = (0..PROVERS)
            .map(|_| {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index > count {
                            return;
                        }
                        let send = Instant::now();
                        let (payment, companion) = slots.run(|| {
                            let mut payer = wallets[index].lock().unwrap();
                            let request = action(
                                id(212, index),
                                OperationActionV1::Send {
                                    request: requests[index - 1].clone(),
                                },
                            );
                            let payment = match payer.execute(request).unwrap() {
                                Completion::Complete(bytes) => bytes,
                                Completion::Pending => complete(
                                    payer.resume().unwrap().expect("selected Send pending"),
                                ),
                                other => panic!("Send did not complete: {other:?}"),
                            };
                            let companion = payer.payment_companion(&payment).unwrap();
                            (payment, companion)
                        });
                        let send_ms = send.elapsed().as_millis();
                        let receive_at = Instant::now();
                        let incoming = receive(&payment, &companion, id(213, index));
                        let received =
                            complete(wallets[0].lock().unwrap().execute(incoming).unwrap());
                        let digests =
                            KagemushaWalletPaymentV1::decode_canonical(&payment, &scheme_id)
                                .unwrap()
                                .digests()
                                .unwrap();
                        collected.lock().unwrap().push(Payment {
                            index,
                            payment,
                            companion,
                            received,
                            credit_id: digests.credit_id,
                            digest: digests.payment,
                            send_ms,
                            receive_ms: receive_at.elapsed().as_millis(),
                        });
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        // Every Payment is delivered: record what the fold achieved while they arrived.
        *arrivals.lock().unwrap() = Some((
            at.elapsed().as_millis(),
            published.load(Ordering::Relaxed),
            cancelled.load(Ordering::Relaxed),
        ));
        delivered.store(true, Ordering::Release);
        folder.join().unwrap();
    });
    let mut payments: Vec<Payment> = collected.into_inner().unwrap();
    payments.sort_by_key(|payment| payment.index);
    phase("concurrent_payments_then_merchant_fold_to_caught_up", at);
    assert_eq!(payments.len(), count);
    let (arrival_ms, arrival_publications, arrival_cancellations) =
        arrivals.into_inner().unwrap().unwrap();
    eprintln!(
        "HUNDRED_PAYERS_ARRIVALS ms={arrival_ms} fold_publications={arrival_publications} fold_cancellations={arrival_cancellations}"
    );

    // Merchant: every accepted Payment is credited once, folded unburned and replays exactly.
    let at = Instant::now();
    let total = PAY * count as u128;
    let mut merchant = wallets[0].lock().unwrap();
    fold(&mut merchant);
    let snapshot = merchant.snapshot().unwrap();
    assert_eq!(snapshot.fold_backlog, 0);
    assert_eq!(snapshot.owned_balance, total);
    assert_eq!(snapshot.folded_balance, Some(total));
    assert_eq!(snapshot.core_burned_total, 0);
    assert_eq!(snapshot.known_burned_total, 0);
    assert_eq!(
        snapshot
            .verified_fold
            .as_ref()
            .map(|fold| fold.burned_total),
        Some(0)
    );
    for payment in &payments {
        let status = merchant
            .credit_status(&payment.credit_id, &payment.digest)
            .unwrap();
        assert_eq!(status.opening.credit_id, payment.credit_id);
        assert_eq!(status.opening.payment_digest, payment.digest);
        assert!(
            !status.opening.burned,
            "accepted Payment {} burned",
            payment.index
        );
        // The exact Receive replays its retained completion without another operation.
        assert_eq!(
            complete(
                merchant
                    .execute(receive(
                        &payment.payment,
                        &payment.companion,
                        id(213, payment.index)
                    ))
                    .unwrap()
            ),
            payment.received
        );
    }
    // A fresh intent for an already credited Payment is refused and changes nothing.
    let before = merchant.snapshot().unwrap();
    let first = &payments[0];
    assert!(
        merchant
            .execute(receive(
                &first.payment,
                &first.companion,
                id(214, first.index)
            ))
            .is_err()
    );
    // A forged Payment proof is refused before any mutation.
    let mut forged =
        KagemushaWalletPaymentV1::decode_canonical(&first.payment, &scheme_id).unwrap();
    forged.send.step_proof.bytes[0] ^= 1;
    assert!(
        merchant
            .execute(receive(
                &norito::encode_canonical(&forged).unwrap(),
                &first.companion,
                id(215, first.index),
            ))
            .is_err()
    );
    assert_eq!(merchant.snapshot().unwrap(), before);
    drop(merchant);

    // Payers: each debited exactly once; value is conserved across all wallets.
    let mut offline = total;
    for index in 1..=count {
        let snapshot = wallets[index].lock().unwrap().snapshot().unwrap();
        assert_eq!(snapshot.owned_balance, LOAD - PAY);
        offline += snapshot.owned_balance;
    }
    assert_eq!(offline, LOAD * count as u128, "offline value is conserved");

    // Restart: drop the merchant's owner, reopen its custody, and replay exactly.
    let mut wallets: Vec<Option<Wallet>> = wallets
        .into_iter()
        .map(|wallet| Some(wallet.into_inner().unwrap()))
        .collect();
    drop(wallets[0].take());
    let mut merchant = devices[0].open(&sources);
    let reopened = merchant.snapshot().unwrap();
    assert_eq!(reopened.owned_balance, total);
    assert_eq!(reopened.folded_balance, Some(total));
    assert_eq!(
        complete(
            merchant
                .execute(receive(
                    &first.payment,
                    &first.companion,
                    id(213, first.index)
                ))
                .unwrap()
        ),
        first.received
    );

    // Onward spend: the merchant pays payer 1 from the received value, which folds unburned.
    let payer = wallets[1].as_mut().unwrap();
    fold(payer);
    let onward_request = payer.request(id(221, 1), PAY, None).unwrap();
    let onward = complete(
        merchant
            .execute(action(
                id(222, 0),
                OperationActionV1::Send {
                    request: onward_request,
                },
            ))
            .unwrap(),
    );
    let onward_companion = merchant.payment_companion(&onward).unwrap();
    assert_eq!(merchant.snapshot().unwrap().owned_balance, total - PAY);
    complete(
        payer
            .execute(receive(&onward, &onward_companion, id(223, 1)))
            .unwrap(),
    );
    fold(payer);
    let snapshot = payer.snapshot().unwrap();
    assert_eq!(snapshot.owned_balance, LOAD);
    assert_eq!(snapshot.folded_balance, Some(LOAD));
    assert_eq!(snapshot.known_burned_total, 0);
    phase("merchant_fold_replay_restart_onward", at);

    let mut send = payments.iter().map(|p| p.send_ms).collect::<Vec<_>>();
    let mut receive_ms = payments.iter().map(|p| p.receive_ms).collect::<Vec<_>>();
    send.sort_unstable();
    receive_ms.sort_unstable();
    let percentile = |values: &[u128], p: usize| values[(values.len() - 1) * p / 100].to_string();
    let result = norito::json!({
        "schema":"iroha.kagemusha.hundred-payers-merchant.v1",
        "payers":count, "provers":PROVERS, "pay_atomic":(PAY.to_string()),
        "merchant_total_atomic":(total.to_string()),
        "send_ms_p50":(percentile(&send, 50)), "send_ms_p95":(percentile(&send, 95)), "send_ms_max":(percentile(&send, 100)),
        "receive_ms_p50":(percentile(&receive_ms, 50)), "receive_ms_p95":(percentile(&receive_ms, 95)), "receive_ms_max":(percentile(&receive_ms, 100)),
        "merchant_fold_publications_during_arrivals":(arrival_publications.to_string()),
        "merchant_fold_cancellations_during_arrivals":(arrival_cancellations.to_string()),
        "phases":(norito::json::Value::Array(phases)),
        "total_ms":(began.elapsed().as_millis().to_string()),
        "scope":"Genuine native proofs, real host custody, simulated hardware keys and engineering issuer on the certified component chain; no physical device, attestation or network-round claim.",
    });
    let bytes = norito::json::to_vec(&result).unwrap();
    fs::write(output.join("result.json"), &bytes).unwrap();
    eprintln!("HUNDRED_PAYERS_RESULT {}", String::from_utf8_lossy(&bytes));
}
