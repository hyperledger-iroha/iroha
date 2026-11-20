//! Step drafting, signing, context binding and artifact custody regression tests.

use super::*;
use clap::Parser as _;
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair};
use iroha_data_model::runtime::lifecycle::SelectionTransitionV1;
use tempfile::TempDir;

struct Fixture {
    directory: TempDir,
    context: ContextArgs,
    roster: Vec<PeerId>,
    keys: Vec<KeyPair>,
    now_ms: u64,
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])))
}

/// Reconstruct R's keyed selection from the release definitions independently of the pinned id.
fn keyed_selection() -> RootSelectionV1 {
    use iroha_core::runtime::{
        ironwood::{taira_repaired_chain, taira_time_ironwood_change_set},
        keyed_state::next_change_set,
    };
    let network = network(1);
    let (repaired, mut published) = taira_repaired_chain(network).unwrap();
    let activated = taira_time_ironwood_change_set(network, 2, [1; 32], &repaired, &published)
        .unwrap()
        .apply(&repaired, &published)
        .unwrap();
    published.extend(activated.published);
    let catalog = iroha_core::runtime::release_catalog().unwrap();
    let adoption = next_change_set(network, &catalog.baseline, &published, 3, [1; 32]).unwrap();
    let [SelectionTransitionV1::SetKernel(kernel)] = adoption.transitions.as_slice() else {
        panic!("expected R's kernel")
    };
    let mut predecessor = activated.selection;
    predecessor.kernel = *kernel;
    for descriptor in adoption.publish {
        published.insert(descriptor.id().unwrap(), descriptor);
    }
    next_change_set(network, &predecessor, &published, 4, [1; 32])
        .unwrap()
        .apply(&predecessor, &published)
        .unwrap()
        .selection
}

fn pinned(directory: &TempDir, name: &str, selection: &RootSelectionV1) -> (PathBuf, [u8; 32]) {
    let bytes = norito::json::to_vec(selection).expect("selection JSON");
    let path = directory.path().join(name);
    write_new(&path, &bytes).expect("selection file");
    (path, *blake3::hash(&bytes).as_bytes())
}

fn draft_change_set_args(
    directory: &TempDir,
    name: &str,
    selection: &RootSelectionV1,
) -> DraftChangeSetArgs {
    let (selection_json, selection_blake3_hex) = pinned(directory, name, selection);
    DraftChangeSetArgs {
        network_id: network(1),
        effective_height: NonZeroU64::new(100).unwrap(),
        selection_json,
        selection_blake3_hex,
        qualification_hex: [0x27; 32],
        output: directory.path().join(format!("{name}.change-set.json")),
    }
}

fn fixture() -> Fixture {
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
    let args = draft_change_set_args(&directory, "selection.json", &keyed_selection());
    let change_set_path = args.output.clone();
    let mut printed = Vec::new();
    Command::DraftChangeSet(args)
        .execute(0, &mut printed)
        .expect("exact change set");
    assert!(
        String::from_utf8(printed)
            .expect("text")
            .starts_with("predecessor=fdda49f6")
    );
    let change_set_bytes = fs_read(&change_set_path);
    let now_ms = 1_000_000;
    let context = ContextArgs {
        chain_id: ChainId::from("fc56984b-2be7-431d-840e-21514d1883f0"),
        network_id: network(1),
        operation_id_hex: [0x43; 32],
        notice_blocks: 64,
        notice_ms: 0,
        notice_approval_cushion_ms: 0,
        expires_at_unix_ms: now_ms + 3_600_000,
        authorization_digest_hex: [0x46; 32],
        release_digest_hex: [0x44; 32],
        source_closure_digest_hex: [0x45; 32],
        change_set_json: change_set_path,
        change_set_blake3_hex: *blake3::hash(&change_set_bytes).as_bytes(),
        roster: roster_path,
        roster_blake3_hex: *blake3::hash(&roster_bytes).as_bytes(),
    };
    Fixture {
        directory,
        context,
        roster,
        keys,
        now_ms,
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

fn shares(fixture: &Fixture, draft: &TairaReadScopePlanV1) -> Vec<ReadScopeSignatureShare> {
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
fn the_draft_publishes_and_activates_only_the_permission_after_cutover() {
    let directory = tempfile::tempdir().unwrap();
    let predecessor = keyed_selection();
    let drafted = draft_change_set_args(&directory, "keyed", &predecessor)
        .change_set()
        .unwrap();
    let descriptor = iroha_core::runtime::state_scope_permission::descriptor();
    let id = descriptor.id().unwrap();
    assert_eq!(drafted.publish, [descriptor]);
    assert_eq!(drafted.transitions, [SelectionTransitionV1::Activate(id)]);
    assert_eq!(
        drafted.predecessor,
        iroha_data_model::runtime::taira_read_scope::KEYED_PREDECESSOR
    );
    let mut successor = predecessor;
    successor.active.push(id);
    successor.active.sort_unstable();
    assert_eq!(drafted.successor, successor.id().unwrap());
    assert!(
        draft_change_set_args(&directory, "again", &successor)
            .change_set()
            .is_err()
    );
    let catalog = iroha_core::runtime::release_catalog().unwrap();
    assert!(
        draft_change_set_args(&directory, "world", &catalog.baseline)
            .change_set()
            .is_err()
    );
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
    changed.authorization_digest[0] ^= 1;
    assert!(assemble(changed, shares).is_err());
}

#[test]
fn prepare_refuses_an_expired_plan_missing_commitments_and_changed_pins() {
    let fixture = fixture();
    prepare(&fixture.context, &fixture.roster, fixture.now_ms).expect("the exact context");
    let mut unauthorized = fixture.context.clone();
    unauthorized.authorization_digest_hex = [0; 32];
    assert!(prepare(&unauthorized, &fixture.roster, fixture.now_ms).is_err());
    assert!(
        prepare(
            &fixture.context,
            &fixture.roster,
            fixture.context.expires_at_unix_ms,
        )
        .is_err()
    );
    let mut change_set_pin = fixture.context.clone();
    change_set_pin.change_set_blake3_hex[0] ^= 1;
    assert!(prepare(&change_set_pin, &fixture.roster, fixture.now_ms).is_err());
    let mut roster_pin = fixture.context.clone();
    roster_pin.roster_blake3_hex[0] ^= 1;
    assert!(roster_pin.load_roster().is_err());
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
        InstructionBox::from(ScheduleTairaReadScopeV1 {
            certificate: expected,
        }),
    );
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
    tool: ReadScope,
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
