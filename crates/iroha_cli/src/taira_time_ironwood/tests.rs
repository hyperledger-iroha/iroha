//! Activation signing, context binding and artifact custody regression tests.

use super::*;
use clap::Parser as _;
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair};
use tempfile::TempDir;

/// Chain discriminant the fixture's account JSON is written under.
const DISCRIMINANT: u16 = 369;

struct Fixture {
    directory: TempDir,
    context: ContextArgs,
    roster: Vec<PeerId>,
    keys: Vec<KeyPair>,
    now_ms: u64,
    _account_context: ChainDiscriminantGuard,
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])))
}

fn pool() -> PrivacyIronwoodPoolBootstrapV1 {
    let reserve = KeyPair::from_seed(vec![0x62; 32], Algorithm::Ed25519);
    PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new([0x41; 32]),
        "7TdbS7ACHkfy1TSW4VwbnMXouos4"
            .parse::<AssetDefinitionId>()
            .expect("asset definition"),
        AssetBalanceScope::Global,
        AccountId::new(reserve.public_key().clone()),
    )
    .expect("pool")
}

/// The exact repaired selection in a pinned file.
fn pinned_selection(directory: &TempDir) -> (PathBuf, [u8; 32]) {
    let (selection, _) = taira_repaired_chain(network(1)).expect("repaired chain");
    let selection = norito::json::to_vec(&selection).expect("selection JSON");
    let selection_path = directory.path().join("selection.json");
    write_new(&selection_path, &selection).expect("selection file");
    (selection_path, *blake3::hash(&selection).as_bytes())
}

fn draft_change_set_args(directory: &TempDir, output: PathBuf) -> DraftChangeSetArgs {
    let (selection_json, selection_blake3_hex) = pinned_selection(directory);
    DraftChangeSetArgs {
        network_id: network(1),
        effective_height: NonZeroU64::new(100).unwrap(),
        selection_json,
        selection_blake3_hex,
        qualification_hex: [0x27; 32],
        output,
    }
}

fn fixture() -> Fixture {
    let account_context = ChainDiscriminantGuard::enter(DISCRIMINANT);
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut keys = (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| {
        iroha_core::sumeragi::schedule::consensus_key(&PeerId::new(key.public_key().clone()))
            .expect("native consensus key")
    });
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let roster_bytes = norito::json::to_vec(&roster).expect("roster JSON");
    let roster_path = directory.path().join("roster.json");
    write_new(&roster_path, &roster_bytes).expect("roster file");
    let change_set_path = directory.path().join("change-set.json");
    let mut buffer = Vec::new();
    Command::DraftChangeSet(draft_change_set_args(&directory, change_set_path.clone()))
        .execute(0, &mut buffer)
        .expect("exact change set");
    let change_set_bytes = fs_read(&change_set_path);
    let pool_bytes = norito::json::to_vec(&pool()).expect("pool JSON");
    let pool_path = directory.path().join("pool.json");
    write_new(&pool_path, &pool_bytes).expect("pool file");
    let now_ms = 1_000_000;
    let context = ContextArgs {
        chain_id: ChainId::from("fc56984b-2be7-431d-840e-21514d1883f0"),
        network_id: network(1),
        operation_id_hex: [0x43; 32],
        notice_blocks: 64,
        notice_ms: 0,
        notice_approval_cushion_ms: 0,
        expires_at_unix_ms: now_ms + 3_600_000,
        authorization_digest_hex: AUTHORIZATION_DIGEST,
        release_digest_hex: [0x44; 32],
        source_closure_digest_hex: [0x45; 32],
        change_set_json: change_set_path,
        change_set_blake3_hex: *blake3::hash(&change_set_bytes).as_bytes(),
        pool_json: pool_path,
        pool_blake3_hex: *blake3::hash(&pool_bytes).as_bytes(),
        roster: roster_path,
        roster_blake3_hex: *blake3::hash(&roster_bytes).as_bytes(),
    };
    Fixture {
        directory,
        context,
        roster,
        keys,
        now_ms,
        _account_context: account_context,
    }
}

fn fs_read(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).expect("fixture file")
}

fn draft_args(fixture: &Fixture) -> DraftArgs {
    let draft = prepare(&fixture.context, &fixture.roster, fixture.now_ms).expect("draft");
    let bytes = draft.to_bytes().expect("encode draft");
    let path = fixture.directory.path().join("draft.to");
    write_new(&path, &bytes).expect("draft file");
    DraftArgs {
        context: fixture.context.clone(),
        draft: path,
        draft_blake3_hex: *blake3::hash(&bytes).as_bytes(),
    }
}

fn shares(fixture: &Fixture, draft: &TairaActivationPlanV1) -> Vec<ActivationSignatureShare> {
    (0..4)
        .map(|index| {
            sign(
                draft,
                &fixture.roster,
                index,
                fixture.keys[usize::from(index)].private_key(),
            )
            .expect("share")
        })
        .collect()
}

#[test]
fn the_certificate_requires_all_four_and_rejects_duplicates_supersets_and_changes() {
    let fixture = fixture();
    let draft = prepare(&fixture.context, &fixture.roster, fixture.now_ms).expect("draft");
    let shares = shares(&fixture, &draft);
    let certificate = assemble(
        draft.clone(),
        vec![
            shares[2].clone(),
            shares[0].clone(),
            shares[3].clone(),
            shares[1].clone(),
        ],
    )
    .expect("all four");
    certificate.verify().expect("native verifier");
    for invalid in [
        shares[..3].to_vec(),
        [shares.clone(), vec![shares[0].clone()]].concat(),
        vec![
            shares[0].clone(),
            shares[0].clone(),
            shares[2].clone(),
            shares[3].clone(),
        ],
    ] {
        assert!(assemble(draft.clone(), invalid).is_err());
    }
    let mut changed = draft;
    changed.release_digest[0] ^= 1;
    assert!(assemble(changed, shares).is_err());
}

#[test]
fn prepare_refuses_another_authorization_an_expired_plan_and_changed_pins() {
    let fixture = fixture();
    prepare(&fixture.context, &fixture.roster, fixture.now_ms).expect("the exact context");
    let mut other = fixture.context.clone();
    other.authorization_digest_hex = [0x99; 32];
    assert!(prepare(&other, &fixture.roster, fixture.now_ms).is_err());
    assert!(
        prepare(
            &fixture.context,
            &fixture.roster,
            fixture.context.expires_at_unix_ms,
        )
        .is_err()
    );
    let mut pool_pin = fixture.context.clone();
    pool_pin.pool_blake3_hex[0] ^= 1;
    assert!(prepare(&pool_pin, &fixture.roster, fixture.now_ms).is_err());
    let mut change_set_pin = fixture.context.clone();
    change_set_pin.change_set_blake3_hex[0] ^= 1;
    assert!(prepare(&change_set_pin, &fixture.roster, fixture.now_ms).is_err());
}

#[test]
fn only_the_exact_repaired_selection_drafts_a_change_set() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let args = draft_change_set_args(&directory, directory.path().join("unused.json"));
    let change_set = args.change_set().expect("exact change set");
    assert_eq!(change_set.effective_height, 100);
    let catalog = iroha_core::runtime::release_catalog().expect("release catalog");
    let baseline = norito::json::to_vec(&catalog.baseline).expect("baseline JSON");
    let baseline_path = directory.path().join("baseline.json");
    write_new(&baseline_path, &baseline).expect("baseline file");
    let refused = DraftChangeSetArgs {
        selection_json: baseline_path,
        selection_blake3_hex: *blake3::hash(&baseline).as_bytes(),
        ..args
    }
    .change_set()
    .expect_err("the baseline is not the repaired selection");
    assert!(format!("{refused:#}").contains("repaired selection"));
}

#[test]
fn a_pinned_draft_rejects_a_foreign_context_and_modified_bytes() {
    let fixture = fixture();
    let args = draft_args(&fixture);
    args.load(fixture.now_ms).expect("reviewed draft");
    let foreign = DraftArgs {
        context: ContextArgs {
            operation_id_hex: [0x99; 32],
            ..fixture.context.clone()
        },
        draft: args.draft.clone(),
        draft_blake3_hex: args.draft_blake3_hex,
    };
    assert!(foreign.load(fixture.now_ms).is_err());
    let modified = DraftArgs {
        context: fixture.context.clone(),
        draft: args.draft.clone(),
        draft_blake3_hex: [0x99; 32],
    };
    assert!(modified.load(fixture.now_ms).is_err());
}

#[test]
fn the_private_key_must_match_its_exact_seat() {
    let fixture = fixture();
    let draft = prepare(&fixture.context, &fixture.roster, fixture.now_ms).expect("draft");
    assert!(sign(&draft, &fixture.roster, 0, fixture.keys[1].private_key()).is_err());
    assert!(sign(&draft, &fixture.roster, 4, fixture.keys[0].private_key()).is_err());
}

#[test]
fn signing_uses_owner_private_custody_and_assembly_writes_only_the_verified_instruction() {
    let fixture = fixture();
    let args = draft_args(&fixture);
    let (draft, _) = args.load(fixture.now_ms).expect("reviewed draft");
    let mut paths = Vec::new();
    let mut buffer = Vec::new();
    for index in 0..4 {
        let key_path = fixture.directory.path().join(format!("{index}.key"));
        let secret = ExposedPrivateKey(fixture.keys[usize::from(index)].private_key().clone())
            .try_to_multihash_string()
            .expect("test key text");
        write_new(&key_path, secret.as_bytes()).expect("private test key file");
        let share_path = fixture.directory.path().join(format!("{index}.to"));
        Command::Sign(SignArgs {
            draft: DraftArgs {
                context: args.context.clone(),
                draft: args.draft.clone(),
                draft_blake3_hex: args.draft_blake3_hex,
            },
            signer_index: index,
            private_key_file: key_path,
            output: share_path.clone(),
        })
        .execute(fixture.now_ms, &mut buffer)
        .expect("sign command");
        paths.push(share_path);
    }
    let output = fixture.directory.path().join("instruction.json");
    Command::Assemble(AssembleArgs {
        draft: args,
        signature: paths,
        output: output.clone(),
    })
    .execute(fixture.now_ms, &mut buffer)
    .expect("assemble command");
    let instructions: Vec<InstructionBox> =
        norito::json::from_slice(&fs_read(&output)).expect("transaction stdin JSON");
    assert_eq!(instructions.len(), 1);
    let expected = assemble(draft.clone(), shares(&fixture, &draft)).expect("verified certificate");
    assert_eq!(
        instructions[0],
        InstructionBox::from(ScheduleTairaTimeIronwoodV1 {
            certificate: expected,
        }),
    );
}

#[test]
fn the_pool_step_writes_what_prepare_reads() {
    let fixture = fixture();
    let output = fixture.directory.path().join("written-pool.json");
    let mut printed = Vec::new();
    Command::Pool(PoolArgs {
        pool_id_hex: [0x41; 32],
        asset_definition_id: "7TdbS7ACHkfy1TSW4VwbnMXouos4".parse().unwrap(),
        reserve_account: pool().reserve_account.to_string(),
        output: output.clone(),
    })
    .execute(fixture.now_ms, &mut printed)
    .expect("pool step");
    let bytes = fs_read(&output);
    let written: PrivacyIronwoodPoolBootstrapV1 =
        norito::json::from_slice(&bytes).expect("pool JSON");
    assert_eq!(written, pool());
    let printed = String::from_utf8(printed).expect("text");
    assert_eq!(
        printed.trim(),
        format!("pool_blake3_hex={}", blake3::hash(&bytes).to_hex()),
    );
    let refused = Command::Pool(PoolArgs {
        pool_id_hex: [0; 32],
        asset_definition_id: "7TdbS7ACHkfy1TSW4VwbnMXouos4".parse().unwrap(),
        reserve_account: pool().reserve_account.to_string(),
        output: fixture.directory.path().join("zero-pool.json"),
    })
    .execute(fixture.now_ms, &mut Vec::new());
    assert!(refused.is_err(), "the zero pool identity is refused");
}

#[test]
fn outputs_are_never_overwritten() {
    let fixture = fixture();
    let args = draft_args(&fixture);
    let output = fixture.directory.path().join("existing");
    write_new(&output, b"kept").expect("existing file");
    let result = Command::Prepare(PrepareArgs {
        context: args.context,
        output: output.clone(),
    })
    .execute(fixture.now_ms, &mut Vec::new());
    assert!(result.is_err());
    assert_eq!(fs_read(&output), b"kept");
}

/// Clap structure for the tool, parsed the way the CLI parses it.
#[derive(clap::Parser, Debug)]
struct TestCli {
    #[command(flatten)]
    tool: TimeIronwood,
}

#[test]
fn the_group_is_credential_free_and_has_no_inline_secret_flag() {
    use clap::CommandFactory as _;
    let command = TestCli::command();
    let sign = command.find_subcommand("sign").expect("sign command");
    let flags: Vec<_> = sign
        .get_arguments()
        .filter_map(|argument| argument.get_long())
        .collect();
    assert!(flags.contains(&"private-key-file"));
    assert!(
        !flags
            .iter()
            .any(|flag| *flag == "private-key" || flag.ends_with("secret")),
        "{flags:?}",
    );
    let error = TestCli::try_parse_from(["tool", "sign"]).expect_err("needs its context");
    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument
    );
}
