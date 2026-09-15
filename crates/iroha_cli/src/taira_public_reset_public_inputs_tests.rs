//! Public preparation tests use native signed genesis fixtures and temporary output custody.

use super::*;
use iroha_crypto::{HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    block::{BlockSignature, SignedBlock, consensus_v2::SumeragiV2GenesisContextParameters},
    isi::{
        SetParameter,
        kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterTemplateV1,
            KagemushaMintFinalityGenesisParametersV1,
        },
    },
    parameter::{
        Parameter,
        custom::CustomParameter,
        system::{
            ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode,
            consensus_metadata,
        },
    },
    transaction::{FeePaymentIntent, TransactionBuilder, TransactionResultInner},
    trigger::DataTriggerSequence,
};
use std::num::NonZeroU64;

fn key(seed: u8, algorithm: Algorithm) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], algorithm).expect("valid deterministic fixture seed")
}

fn line(value: impl std::fmt::Display) -> Vec<u8> {
    format!("{value}\n").into_bytes()
}

struct Fixture {
    block: SignedBlock,
    genesis: KeyPair,
    canary: KeyPair,
}

impl Fixture {
    fn new() -> Self {
        let genesis = key(101, Algorithm::Ed25519);
        let canary = key(102, Algorithm::Ed25519);
        let mut validators: Vec<_> = (110..114)
            .map(|seed| {
                let peer = PeerId::new(key(seed, Algorithm::BlsNormal).public_key().clone());
                iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                    &[seed; 32], 0, peer,
                ).expect("native public mint-finality fixture keys")
            })
            .collect();
        validators.sort_by(|a, b| a.validator.cmp(&b.validator));
        let metadata = ConsensusHandshakeMetadata {
            mode: SumeragiConsensusMode::Npos,
            block_cadence_ms: NonZeroU64::new(1000).unwrap(),
            wire_protocol_version: u32::from(
                iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            ),
            consensus_fingerprint: ConsensusFingerprint::new([0xab; 32]),
            kagemusha_mint_finality: KagemushaMintFinalityGenesisParametersV1 {
                epoch_roster: KagemushaMintFinalityEpochRosterTemplateV1 {
                    version: KAGEMUSHA_CHAIN_VERSION_V1,
                    epoch: 0,
                    validators,
                },
                next_epoch_roster: None,
            },
            sumeragi_v2: SumeragiV2GenesisContextParameters::recommended(),
        };
        metadata
            .validate()
            .expect("valid native consensus envelope");
        let parameter = Parameter::Custom(CustomParameter::new(
            consensus_metadata::handshake_meta_id(),
            iroha_primitives::json::Json::new(metadata),
        ));
        let mut instructions: Vec<iroha_data_model::isi::InstructionBox> =
            vec![SetParameter::new(parameter).into()];
        for seed in 110..114 {
            let validator = key(seed, Algorithm::BlsNormal);
            instructions.push(
                iroha_data_model::isi::register::RegisterPeerWithPop::new(
                    PeerId::new(validator.public_key().clone()),
                    iroha_crypto::bls_normal_pop_prove(validator.private_key()).unwrap(),
                )
                .into(),
            );
        }
        let mut builder = TransactionBuilder::new_genesis(
            AccountId::new(genesis.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions);
        builder.set_creation_time(std::time::Duration::from_millis(42));
        let transaction = builder.try_sign(genesis.private_key()).unwrap();
        let mut block =
            SignedBlock::try_genesis(vec![transaction], genesis.private_key(), None, None).unwrap();
        let entrypoints = block
            .external_entrypoints_cloned()
            .map(|entry| entry.hash())
            .collect::<Vec<_>>();
        block
            .set_transaction_results(
                Vec::new(),
                &entrypoints,
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .unwrap();
        block
            .replace_signatures(BTreeSet::from([BlockSignature::new(
                0,
                SignatureOf::try_from_hash(genesis.private_key(), block.hash()).unwrap(),
            )]))
            .unwrap();
        // The same production validator called by assembly must accept this fixture.
        iroha_core::release_identity::genesis_identity(
            &block.encode_wire().unwrap(),
            genesis.public_key(),
        )
        .unwrap();
        Self {
            block,
            genesis,
            canary,
        }
    }

    fn network(&self) -> NetworkId {
        NetworkId::from_genesis_hash(self.block.hash())
    }

    fn derive(&self) -> Result<PublicInputsV1> {
        derive(
            &self.block.encode_wire().unwrap(),
            &line(self.network()),
            &line(self.genesis.public_key()),
            &line(self.canary.public_key()),
        )
    }

    #[cfg(unix)]
    fn write(&self, root: &Path) -> PreparePublicInputs {
        let localnet = root.join("localnet");
        fs::create_dir(&localnet).unwrap();
        fs::set_permissions(&localnet, fs::Permissions::from_mode(0o700)).unwrap();
        for (name, bytes) in [
            ("genesis.signed.nrt", self.block.encode_wire().unwrap()),
            ("genesis.expected_hash", line(self.network())),
            ("genesis.public_key", line(self.genesis.public_key())),
        ] {
            fs::write(localnet.join(name), bytes).unwrap();
        }
        let canary_public_key = root.join("canary.public_key");
        fs::write(&canary_public_key, line(self.canary.public_key())).unwrap();
        PreparePublicInputs {
            localnet_dir: localnet,
            canary_public_key,
            output_dir: root.join("public-inputs"),
        }
    }
}

pub(crate) fn deployment_genesis_fixture() -> (SignedBlock, KeyPair) {
    let fixture = Fixture::new();
    (fixture.block, fixture.genesis)
}

#[test]
fn derives_native_genesis_identity_and_exact_canary_request() {
    let fixture = Fixture::new();
    let report = fixture.derive().unwrap();
    assert_eq!(report.genesis_hash, fixture.block.hash().to_string());
    assert_ne!(report.genesis_hash, report.signed_genesis_sha256);
    assert_eq!(report.network_id, fixture.network());
    assert_eq!(
        report.canary_onboarding_request.alias,
        crate::taira::canary_alias(fixture.canary.public_key())
    );
    assert!(report.canary_onboarding_request.permissions.is_empty());
    let _guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    assert_eq!(
        report.canary_onboarding_request.account_id,
        AccountId::new(fixture.canary.public_key().clone()).to_string()
    );
    let decoded: PublicInputsV1 = json::from_slice(&json_line(&report).unwrap()).unwrap();
    assert_eq!(decoded, report);
}

#[test]
fn rejects_wrong_network_key_and_resultless_genesis() {
    let fixture = Fixture::new();
    let wire = fixture.block.encode_wire().unwrap();
    let wrong =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"wrong network")));
    assert!(
        derive(
            &wire,
            &line(wrong),
            &line(fixture.genesis.public_key()),
            &line(fixture.canary.public_key())
        )
        .unwrap_err()
        .to_string()
        .contains("differs from checked network")
    );
    assert!(
        derive(
            &wire,
            &line(fixture.network()),
            &line(fixture.canary.public_key()),
            &line(fixture.canary.public_key())
        )
        .is_err()
    );
    let resultless = fixture
        .block
        .canonical_resultless_proposal()
        .encode_wire()
        .unwrap();
    assert!(
        derive(
            &resultless,
            &line(fixture.network()),
            &line(fixture.genesis.public_key()),
            &line(fixture.canary.public_key())
        )
        .is_err()
    );
}

#[test]
fn rejects_noncanonical_identity_and_non_ed25519_canary() {
    let fixture = Fixture::new();
    for text in [
        Vec::new(),
        b"invalid\n".to_vec(),
        line(fixture.network()).repeat(2),
        fixture.network().to_string().into_bytes(),
    ] {
        assert!(
            derive(
                &fixture.block.encode_wire().unwrap(),
                &text,
                &line(fixture.genesis.public_key()),
                &line(fixture.canary.public_key())
            )
            .is_err()
        );
    }
    let unsupported = key(104, Algorithm::Secp256k1);
    assert!(
        derive(
            &fixture.block.encode_wire().unwrap(),
            &line(fixture.network()),
            &line(fixture.genesis.public_key()),
            &line(unsupported.public_key())
        )
        .unwrap_err()
        .to_string()
        .contains("must use Ed25519")
    );
    assert!(canonical_public_key(b"not-a-key\n").is_err());
    assert!(canonical_line(&vec![b'a'; 1025]).is_err());
}

#[cfg(unix)]
#[test]
fn publishes_complete_public_bundle_and_reuses_identical_request() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let fixture = Fixture::new();
    let args = fixture.write(&root);
    let original = fs::read(args.localnet_dir.join("genesis.signed.nrt")).unwrap();
    let mut first = Vec::new();
    prepare(&args, &mut first).unwrap();
    let before = fs::metadata(args.output_dir.join("public-inputs.json"))
        .unwrap()
        .ino();
    let mut second = Vec::new();
    prepare(&args, &mut second).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        before,
        fs::metadata(args.output_dir.join("public-inputs.json"))
            .unwrap()
            .ino()
    );
    assert_eq!(load(&args.output_dir).unwrap(), fixture.derive().unwrap());
    assert_eq!(
        fs::read(args.localnet_dir.join("genesis.signed.nrt")).unwrap(),
        original
    );
    for name in OUTPUT_FILES {
        assert_eq!(
            fs::metadata(args.output_dir.join(name)).unwrap().mode() & 0o7777,
            0o644
        );
    }
}

#[cfg(unix)]
#[test]
fn refuses_changed_bundle_and_never_overwrites_existing_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let args = Fixture::new().write(&root);
    prepare(&args, &mut Vec::new()).unwrap();
    let original = fs::read(args.output_dir.join("public-inputs.json")).unwrap();
    fs::write(
        &args.canary_public_key,
        line(key(103, Algorithm::Ed25519).public_key()),
    )
    .unwrap();
    assert!(prepare(&args, &mut Vec::new()).is_err());
    assert_eq!(
        fs::read(args.output_dir.join("public-inputs.json")).unwrap(),
        original
    );
    fs::write(args.output_dir.join("genesis.hash"), b"wrong\n").unwrap();
    assert!(load(&args.output_dir).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_input_and_partial_or_surplus_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let args = Fixture::new().write(&root);
    let key = fs::read(&args.canary_public_key).unwrap();
    fs::remove_file(&args.canary_public_key).unwrap();
    std::os::unix::fs::symlink(
        args.localnet_dir.join("genesis.public_key"),
        &args.canary_public_key,
    )
    .unwrap();
    assert!(prepare(&args, &mut Vec::new()).is_err());
    assert!(!args.output_dir.exists());
    fs::remove_file(&args.canary_public_key).unwrap();
    fs::write(&args.canary_public_key, key).unwrap();
    prepare(&args, &mut Vec::new()).unwrap();
    fs::write(args.output_dir.join("unexpected"), b"extra").unwrap();
    assert!(load(&args.output_dir).is_err());
    fs::remove_file(args.output_dir.join("unexpected")).unwrap();
    fs::remove_file(args.output_dir.join("genesis.hash")).unwrap();
    assert!(load(&args.output_dir).is_err());
}

#[test]
fn cli_public_input_preparation_never_accepts_private_credentials() {
    use clap::Parser as _;
    let args = [
        "iroha",
        "taira",
        "public-reset",
        "prepare-public-inputs",
        "--localnet-dir",
        "/localnet",
        "--canary-public-key",
        "/canary.pub",
        "--output-dir",
        "/output",
    ];
    assert!(crate::Args::try_parse_from(args).is_ok());
    let mut private = args.to_vec();
    private.extend(["--private-key", "forbidden"]);
    assert!(crate::Args::try_parse_from(private).is_err());
}
