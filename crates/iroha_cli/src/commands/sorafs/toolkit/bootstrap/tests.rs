//! Native repair signing, context binding and artifact custody regression tests.

use super::*;
use clap::Parser as _;
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair};
use iroha_data_model::{account::AccountId, sorafs::bootstrap::SorafsBootstrapProviderV1};
use sorafs_manifest::{
    CouncilSignature, provider_admission::compute_envelope_authorization_digest,
};
use tempfile::TempDir;

struct Fixture {
    directory: TempDir,
    context: ContextArgs,
    roster: Vec<PeerId>,
    keys: Vec<KeyPair>,
    payload: SorafsBootstrapPayloadV1,
    now: u64,
}

fn network(seed: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])))
}

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let catalog = iroha_core::runtime::release_catalog().expect("release catalog");
    let selection_bytes = norito::json::to_vec(&catalog.baseline).expect("selection JSON");
    let definitions = catalog
        .baseline_definitions
        .iter()
        .map(|id| catalog.definitions[id].clone())
        .collect::<Vec<_>>();
    let definitions_bytes = norito::json::to_vec(&definitions).expect("definitions JSON");
    let selection_path = directory.path().join("selection.json");
    let definitions_path = directory.path().join("definitions.json");
    write_new(&selection_path, &selection_bytes).expect("selection file");
    write_new(&definitions_path, &definitions_bytes).expect("definitions file");
    let change_set_path = directory.path().join("change-set.json");
    DraftChangeSetArgs {
        network_id: network(11),
        parent_height: NonZeroU64::new(21).expect("parent"),
        selection_json: selection_path,
        selection_blake3_hex: *blake3::hash(&selection_bytes).as_bytes(),
        definitions_json: definitions_path,
        definitions_blake3_hex: *blake3::hash(&definitions_bytes).as_bytes(),
        qualification_hex: [31; 32],
        output: change_set_path.clone(),
    }
    .execute()
    .expect("release-owned repair transition");
    let change_set_bytes = fs::read(&change_set_path).expect("change set bytes");
    let context = ContextArgs {
        chain_id: ChainId::from("repair-fixture"),
        parent_height: NonZeroU64::new(21).expect("parent"),
        parent_core_hash_hex: [21; 32],
        parent_result_hash_hex: [22; 32],
        operation_id_hex: [23; 32],
        expires_at_unix_ms: (now + 1800) * 1000,
        authorization_digest_hex: [24; 32],
        release_digest_hex: [25; 32],
        source_closure_digest_hex: [26; 32],
        change_set_json: change_set_path,
        change_set_blake3_hex: *blake3::hash(&change_set_bytes).as_bytes(),
        network_id: network(11),
        roster: roster_path,
        roster_blake3_hex: *blake3::hash(&roster_bytes).as_bytes(),
    };
    let council_key = key(80);
    let council = ProviderAdmissionCouncilPolicyV1 {
        network_id: *context.network_id.as_bytes(),
        policy_id: [81; 32],
        version: 1,
        revision: 1,
        predecessor_policy_digest: None,
        trusted_signers: vec![
            council_key
                .public_key()
                .try_to_bytes()
                .expect("public key")
                .1
                .try_into()
                .expect("Ed25519 length"),
        ],
        signature_threshold: 1,
        paused: false,
    };
    let mut envelope: ProviderAdmissionEnvelopeV1 =
        norito::decode_from_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
        )))
        .expect("native admission fixture");
    envelope.network_id = council.network_id;
    envelope.policy_id = council.policy_id;
    envelope.policy_digest = council.canonical_digest().expect("council digest");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    envelope.issued_at = now - 60;
    envelope.retention_epoch = now + 3600;
    let digest = compute_envelope_authorization_digest(&envelope).expect("admission preimage");
    envelope.council_signatures = vec![CouncilSignature {
        signer: council.trusted_signers[0],
        signature: Signature::try_new(council_key.private_key(), &digest)
            .expect("test council signature")
            .payload()
            .to_vec(),
    }];
    let payload = SorafsBootstrapPayloadV1 {
        council: norito::encode_canonical(&council).expect("canonical council"),
        providers: vec![SorafsBootstrapProviderV1 {
            owner: AccountId::new(key(82).public_key().clone()),
            envelope: norito::encode_canonical(&envelope).expect("canonical envelope"),
        }],
        grants: Vec::new(),
        compliance_operators: Vec::new(),
    };
    Fixture {
        directory,
        context,
        roster,
        keys,
        payload,
        now,
    }
}

fn draft_args(fixture: &Fixture) -> DraftArgs {
    let draft = prepare(&fixture.payload, &fixture.context, &fixture.roster).expect("draft");
    let bytes = norito::encode_canonical(&draft).expect("encode draft");
    let path = fixture.directory.path().join("draft.to");
    write_new(&path, &bytes).expect("draft file");
    DraftArgs {
        context: fixture.context.clone(),
        draft: path,
        draft_blake3_hex: *blake3::hash(&bytes).as_bytes(),
    }
}

#[test]
fn native_certificate_requires_all_four_and_rejects_duplicates_supersets_and_changes() {
    let fixture = fixture();
    validate_payload(&fixture.payload, &fixture.context.network_id, fixture.now)
        .expect("valid payload");
    let draft = prepare(&fixture.payload, &fixture.context, &fixture.roster).expect("draft");
    let shares = (0..4)
        .map(|index| {
            sign(
                &draft,
                &fixture.roster,
                index,
                fixture.keys[usize::from(index)].private_key(),
            )
            .expect("share")
        })
        .collect::<Vec<_>>();
    let certificate = assemble(
        draft.clone(),
        vec![
            shares[2].clone(),
            shares[0].clone(),
            shares[3].clone(),
            shares[1].clone(),
        ],
        &fixture.context,
        &fixture.roster,
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
        assert!(assemble(draft.clone(), invalid, &fixture.context, &fixture.roster).is_err());
    }
    let mut changed = draft;
    changed.release_digest[0] ^= 1;
    assert!(assemble(changed, shares.clone(), &fixture.context, &fixture.roster).is_err());
}

#[test]
fn pinned_draft_rejects_foreign_context_and_modified_bytes() {
    let fixture = fixture();
    let mut args = draft_args(&fixture);
    args.load(fixture.now).expect("pinned draft");
    args.context.network_id = network(12);
    assert!(args.load(fixture.now).is_err());
    args.context.network_id = fixture.context.network_id;
    args.context.parent_height = NonZeroU64::new(22).expect("height");
    assert!(args.load(fixture.now).is_err());
    args.context.parent_height = fixture.context.parent_height;
    args.context.parent_core_hash_hex[0] ^= 1;
    assert!(args.load(fixture.now).is_err());
    args.context.parent_core_hash_hex = fixture.context.parent_core_hash_hex;
    args.context.release_digest_hex[0] ^= 1;
    assert!(args.load(fixture.now).is_err());
    args.context.release_digest_hex = fixture.context.release_digest_hex;
    args.context.change_set_blake3_hex[0] ^= 1;
    assert!(args.load(fixture.now).is_err());
    args.context.change_set_blake3_hex = fixture.context.change_set_blake3_hex;
    assert!(args.load(fixture.now + 1800).is_err());
    args.context.roster_blake3_hex[0] ^= 1;
    assert!(args.load(fixture.now).is_err());
    args.context.roster_blake3_hex = fixture.context.roster_blake3_hex;
    let mut bytes = fs::read(&args.draft).expect("draft bytes");
    bytes.push(0);
    fs::write(&args.draft, &bytes).expect("alter draft");
    assert!(args.load(fixture.now).is_err());
    args.draft_blake3_hex = *blake3::hash(&bytes).as_bytes();
    assert!(
        args.load(fixture.now).is_err(),
        "canonical decoding rejects trailing bytes even when pinned"
    );
}

#[test]
fn payload_rejects_foreign_network_expiration_bad_admission_and_duplicate_provider() {
    let fixture = fixture();
    assert!(validate_payload(&fixture.payload, &network(12), fixture.now).is_err());
    assert!(
        validate_payload(
            &fixture.payload,
            &fixture.context.network_id,
            fixture.now + 3600
        )
        .is_err()
    );
    let mut invalid = fixture.payload.clone();
    invalid.providers.push(invalid.providers[0].clone());
    assert!(validate_payload(&invalid, &fixture.context.network_id, fixture.now).is_err());
    let mut envelope: ProviderAdmissionEnvelopeV1 =
        decode_frame(&fixture.payload.providers[0].envelope).expect("envelope");
    envelope.council_signatures[0].signature[0] ^= 1;
    invalid = fixture.payload;
    invalid.providers[0].envelope =
        norito::encode_canonical(&envelope).expect("encode invalid envelope");
    assert!(validate_payload(&invalid, &fixture.context.network_id, fixture.now).is_err());
}

#[test]
fn roster_and_private_key_must_match_exact_seat() {
    let fixture = fixture();
    let draft = prepare(&fixture.payload, &fixture.context, &fixture.roster).expect("draft");
    assert!(sign(&draft, &fixture.roster, 0, fixture.keys[1].private_key()).is_err());
    assert!(sign(&draft, &fixture.roster, 4, fixture.keys[0].private_key()).is_err());
    assert!(validate_roster(&fixture.roster[..1]).is_err());
    assert!(validate_roster(&fixture.roster[..3]).is_err());
    let mut reversed = fixture.roster.clone();
    reversed.reverse();
    assert!(validate_roster(&reversed).is_err());
    let mut duplicate = fixture.roster.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(validate_roster(&duplicate).is_err());
}

#[test]
fn bounded_pinned_artifacts_and_fresh_outputs_are_enforced() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("artifact");
    write_new(&path, b"public").expect("new file");
    assert!(write_new(&path, b"replacement").is_err());
    assert_eq!(fs::read(&path).expect("read"), b"public");
    assert!(read_bounded(&path, 5).is_err());
    assert!(read_pinned(&path, 6, [0; 32]).is_err());
    assert_eq!(
        read_pinned(&path, 6, *blake3::hash(b"public").as_bytes()).expect("pinned bytes"),
        b"public"
    );
    assert!(parse_digest(&"A".repeat(64)).is_err());
    assert!(parse_digest("00").is_err());
    assert_eq!(parse_digest(&"ab".repeat(32)).expect("digest"), [0xab; 32]);
}

#[cfg(unix)]
#[test]
fn command_signing_uses_owner_private_custody_and_does_not_expose_parse_input() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let fixture = fixture();
    let args = draft_args(&fixture);
    let key_path = fixture.directory.path().join("runtime.key");
    let secret = ExposedPrivateKey(fixture.keys[0].private_key().clone())
        .try_to_multihash_string()
        .expect("test key text");
    write_new(&key_path, secret.as_bytes()).expect("private test key file");
    let share_path = fixture.directory.path().join("share.to");
    Command::Sign(SignArgs {
        draft: args,
        signer_index: 0,
        private_key_file: key_path.clone(),
        output: share_path.clone(),
    })
    .execute()
    .expect("sign command");
    let share: RepairSignatureShare =
        decode_canonical(&fs::read(share_path).expect("share bytes")).expect("canonical share");
    assert_eq!(share.signer_index, 0);
    fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644))
        .expect("unsafe test permissions");
    assert!(
        super::super::super::read_owner_private_handshake_file(
            &key_path,
            16384,
            None,
            "test credential"
        )
        .is_err()
    );
    fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
        .expect("restore private mode");
    let link = fixture.directory.path().join("link.key");
    symlink(&key_path, &link).expect("test symlink");
    assert!(
        super::super::super::read_owner_private_handshake_file(
            &link,
            16384,
            None,
            "test credential"
        )
        .is_err()
    );
    let invalid_secret = "synthetic-invalid-private-material-must-not-appear-in-errors";
    fs::write(&key_path, invalid_secret).expect("invalid private test input");
    let error = read_private_key(&key_path).expect_err("invalid credential");
    assert!(!format!("{error:#}").contains(invalid_secret));
}

#[test]
fn assembled_transaction_stdin_contains_only_the_native_verified_instruction() {
    let fixture = fixture();
    let args = draft_args(&fixture);
    let (draft, roster) = args.load(fixture.now).expect("reviewed draft");
    let mut paths = Vec::new();
    for index in 0..4 {
        let share = sign(
            &draft,
            &roster,
            index,
            fixture.keys[usize::from(index)].private_key(),
        )
        .expect("native share");
        let path = fixture.directory.path().join(format!("share-{index}.to"));
        write_new(
            &path,
            &norito::encode_canonical(&share).expect("canonical share"),
        )
        .expect("share file");
        paths.push(path);
    }
    let output = fixture.directory.path().join("instruction.json");
    Command::Assemble(AssembleArgs {
        draft: args,
        signature: paths,
        output: output.clone(),
    })
    .execute()
    .expect("assemble command");
    let instructions: Vec<InstructionBox> =
        norito::json::from_slice(&fs::read(output).expect("output"))
            .expect("transaction stdin JSON");
    assert_eq!(instructions.len(), 1);
    let expected = assemble(
        draft.clone(),
        (0..4)
            .map(|index| {
                sign(
                    &draft,
                    &roster,
                    index,
                    fixture.keys[usize::from(index)].private_key(),
                )
                .expect("share")
            })
            .collect(),
        &fixture.context,
        &roster,
    )
    .expect("verified certificate");
    assert_eq!(
        instructions[0],
        InstructionBox::from(ApplySorafsRepairV1 {
            certificate: expected
        })
    );
}

#[test]
fn repair_group_is_credential_free_and_has_no_inline_secret_flag() {
    let network = network(11).to_string();
    let digest = "ab".repeat(32);
    let args = [
        "iroha",
        "app",
        "sorafs",
        "toolkit",
        "repair",
        "sign",
        "--network-id",
        &network,
        "--chain-id",
        "repair-fixture",
        "--parent-height",
        "21",
        "--parent-core-hash-hex",
        &digest,
        "--parent-result-hash-hex",
        &digest,
        "--operation-id-hex",
        &digest,
        "--expires-at-unix-ms",
        "2000000000000",
        "--authorization-digest-hex",
        &digest,
        "--release-digest-hex",
        &digest,
        "--source-closure-digest-hex",
        &digest,
        "--change-set-json",
        "change-set.json",
        "--change-set-blake3-hex",
        &digest,
        "--roster",
        "roster.json",
        "--roster-blake3-hex",
        &digest,
        "--draft",
        "draft.to",
        "--draft-blake3-hex",
        &digest,
        "--signer-index",
        "0",
        "--private-key-file",
        "runtime.key",
        "--output",
        "share.to",
    ];
    let parsed = crate::Args::try_parse_from(args).expect("parse offline signing");
    let crate::Command::App(crate::app::Command::Sorafs(super::super::super::Command::Toolkit(
        command,
    ))) = parsed.command
    else {
        panic!("expected toolkit command")
    };
    assert!(command.is_artifact_tool());
    let mut inline = args.to_vec();
    let key_flag = inline
        .iter()
        .position(|value| *value == "--private-key-file")
        .expect("key flag");
    inline[key_flag] = "--private-key";
    assert!(crate::Args::try_parse_from(inline).is_err());
}

#[test]
fn transition_draft_rejects_duplicate_definitions_changed_pin_and_zero_qualification() {
    let fixture = fixture();
    let selection = fixture.directory.path().join("selection.json");
    let definitions = fixture.directory.path().join("definitions.json");
    let selection_bytes = fs::read(&selection).expect("selection");
    let definitions_bytes = fs::read(&definitions).expect("definitions");
    let args = || DraftChangeSetArgs {
        network_id: fixture.context.network_id,
        parent_height: fixture.context.parent_height,
        selection_json: selection.clone(),
        selection_blake3_hex: *blake3::hash(&selection_bytes).as_bytes(),
        definitions_json: definitions.clone(),
        definitions_blake3_hex: *blake3::hash(&definitions_bytes).as_bytes(),
        qualification_hex: [31; 32],
        output: fixture.directory.path().join("must-not-exist.json"),
    };
    let mut wrong_pin = args();
    wrong_pin.selection_blake3_hex[0] ^= 1;
    assert!(wrong_pin.execute().is_err());
    let mut no_qualification = args();
    no_qualification.qualification_hex = [0; 32];
    assert!(no_qualification.execute().is_err());
    let mut overflowing = args();
    overflowing.parent_height = NonZeroU64::new(u64::MAX).expect("nonzero");
    assert!(overflowing.execute().is_err());
    let mut descriptors: Vec<DefinitionDescriptorV1> =
        norito::json::from_slice(&definitions_bytes).expect("definition array");
    descriptors.push(descriptors[0].clone());
    let duplicate_bytes = norito::json::to_vec(&descriptors).expect("duplicate fixture");
    fs::write(&definitions, &duplicate_bytes).expect("replace test input");
    let mut duplicated = args();
    duplicated.definitions_blake3_hex = *blake3::hash(&duplicate_bytes).as_bytes();
    assert!(duplicated.execute().is_err());
    assert!(
        !fixture
            .directory
            .path()
            .join("must-not-exist.json")
            .exists()
    );
}
