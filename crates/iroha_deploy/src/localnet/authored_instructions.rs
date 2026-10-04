//! Bounded public authored instructions for a fresh localnet genesis.

use color_eyre::eyre::{Result, WrapErr as _, ensure, eyre};
use iroha_data_model::isi::{InstructionBox, SetParameter};
use iroha_genesis::{
    GENESIS_MANIFEST_JSON_MAX_BYTES_V1, genesis_instructions_json, validate_genesis_manifest_json,
};
use std::path::PathBuf;

/// Explicit public-file selection for optional fresh-genesis authoring.
///
/// The file must contain the canonical maintained genesis instruction JSON array.
/// Rows may use either the maintained structured projection or the canonical
/// `InstructionBox` base64 JSON string, including a mixture of both forms.
/// Its SHA-256 is over the exact selected file bytes, including whitespace.
#[derive(Clone, Debug)]
pub struct LocalnetAuthoredInstructions {
    /// Bounded stable regular single-link public instruction file.
    pub path: PathBuf,
    /// Expected raw SHA-256 bytes; this is not an Iroha Blake2b `Hash`.
    pub expected_sha256: [u8; 32],
}

pub(super) fn load_optional(
    selected: Option<&LocalnetAuthoredInstructions>,
) -> Result<Option<Vec<InstructionBox>>> {
    selected.map(load).transpose()
}

fn load(selected: &LocalnetAuthoredInstructions) -> Result<Vec<InstructionBox>> {
    let bytes = iroha_fs::read_regular(&selected.path, GENESIS_MANIFEST_JSON_MAX_BYTES_V1)
        .wrap_err("read bounded stable public genesis instruction file")?;
    ensure!(
        iroha_crypto::sha256(&*bytes) == selected.expected_sha256,
        "public genesis instruction file SHA-256 differs from the selected digest"
    );
    let source = validate_genesis_manifest_json(&bytes)
        .wrap_err("validate authored genesis instruction JSON bounds")?;
    let value: norito::json::Value =
        norito::json::from_str(source).wrap_err("parse authored genesis instruction JSON")?;
    let instructions = genesis_instructions_json::from_value(&value)
        .wrap_err("decode authored instructions with the maintained genesis parser")?;
    ensure!(
        !instructions.is_empty(),
        "authored instruction file must not be empty"
    );
    ensure!(
        instructions.iter().all(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<SetParameter>()
                .is_none()
        }),
        "authored instruction file cannot contain SetParameter; use the authoritative parameter snapshot"
    );
    let rows = value
        .as_array()
        .ok_or_else(|| eyre!("authored instruction file must contain a JSON array"))?;
    ensure!(
        rows.len() == instructions.len(),
        "authored instruction count differs after maintained genesis decoding"
    );
    for (row, instruction) in rows.iter().zip(&instructions) {
        let encoded = norito::encode_canonical(instruction)
            .wrap_err("canonical-encode authored InstructionBox")?;
        let decoded = norito::decode_canonical::<InstructionBox>(&encoded)
            .wrap_err("canonical-decode authored InstructionBox")?;
        ensure!(
            decoded == *instruction,
            "authored instruction canonical roundtrip differs"
        );
        let canonical_json = norito::json::value::to_value(instruction)
            .wrap_err("encode canonical authored InstructionBox JSON")?;
        ensure!(
            row == &genesis_instructions_json::instruction_value(instruction)
                || row == &canonical_json,
            "authored instructions must use an exact maintained structured projection or canonical InstructionBox JSON string without unknown fields"
        );
    }
    Ok(instructions)
}

#[cfg(test)]
mod tests {
    //! Generic current-model input admission and fail-before-output checks.

    use super::*;
    use crate::localnet::{
        DEFAULT_CHAIN_ID, LocalnetOptions, LocalnetServiceProfile, TairaParentCatalog,
        generate_localnet_with_authored_instructions, localnet_test_helpers,
    };
    use iroha_data_model::{
        account::Account,
        isi::{Grant, Log, Register},
        level::Level,
        parameter::{Parameter, system::TransactionParameter},
        permission::Permission,
    };
    use std::{
        fs,
        io::BufWriter,
        num::{NonZeroU16, NonZeroU64},
        path::Path,
    };

    fn source_bytes() -> Vec<u8> {
        iroha_genesis::init_instruction_registry();
        let instructions = [
            InstructionBox::from(Log::new(Level::INFO, "first generic instruction".into())),
            InstructionBox::from(Log::new(Level::INFO, "second generic instruction".into())),
        ];
        norito::json::to_vec(&genesis_instructions_json::instructions_to_value(
            &instructions,
        ))
        .unwrap()
    }

    fn selection(parent: &Path, bytes: &[u8]) -> LocalnetAuthoredInstructions {
        let path = parent.join("authored.json");
        iroha_fs::PrivateDirectory::open(parent)
            .unwrap()
            .write_atomic("authored.json", bytes, iroha_fs::PublishMode::CreateNew)
            .unwrap();
        LocalnetAuthoredInstructions {
            path,
            expected_sha256: iroha_crypto::sha256(bytes),
        }
    }

    fn assert_preflight_refuses_without_output(selected: &LocalnetAuthoredInstructions) {
        let output = selected.path.parent().unwrap().join("must-remain-absent");
        let opts = LocalnetOptions {
            service_profile: LocalnetServiceProfile::Standard,
            sora_profile: None,
            perf_profile: None,
            peers: NonZeroU16::new(4).unwrap(),
            seed: Some("unreached-authored-input-fixture".into()),
            bind_host: "127.0.0.1".into(),
            public_host: "127.0.0.1".into(),
            base_api_port: 8_080,
            base_p2p_port: 13_337,
            out_dir: output.clone(),
            extra_accounts: 0,
            assets: Vec::new(),
            consensus_mode:
                iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned,
            block_cadence_ms: None,
        };
        let mut writer = BufWriter::new(Vec::new());
        let result = generate_localnet_with_authored_instructions(
            &opts,
            &mut writer,
            Some(DEFAULT_CHAIN_ID),
            None,
            TairaParentCatalog::WithIs,
            Some(selected),
        );
        assert!(
            result.is_err(),
            "invalid authored input must be a normal error"
        );
        assert!(
            !output.exists(),
            "preflight must precede output and key creation"
        );
        assert!(writer.into_inner().unwrap().is_empty());
    }

    #[test]
    fn authored_instructions_absent_preserves_optional_default() {
        assert!(load_optional(None).unwrap().is_none());
    }

    #[test]
    fn authored_instructions_accept_exact_digest_and_generic_order() {
        let temporary = localnet_test_helpers::private_tempdir().unwrap();
        let bytes = source_bytes();
        let selected = selection(temporary.path(), &bytes);
        let decoded = load_optional(Some(&selected)).unwrap().unwrap();
        let expected = genesis_instructions_json::from_value(
            &norito::json::from_slice::<norito::json::Value>(&bytes).unwrap(),
        )
        .unwrap();
        assert_eq!(decoded, expected);
        assert_eq!(decoded.len(), 2);
    }

    #[test]
    fn authored_instructions_accept_generic_register_and_grant_in_both_canonical_forms() {
        let _chain = iroha_data_model::account::address::ChainDiscriminantGuard::enter(42);
        iroha_genesis::init_instruction_registry();
        let account = iroha_test_samples::ALICE_ID.clone();
        let instructions = [
            InstructionBox::from(Register::account(Account::new(account.clone()))),
            InstructionBox::from(Grant::account_permission(
                Permission::new(
                    "CanSetParameters".into(),
                    iroha_primitives::json::Json::new(()),
                ),
                account,
            )),
        ];
        let structured = instructions
            .iter()
            .map(genesis_instructions_json::instruction_value)
            .collect::<Vec<_>>();
        let canonical = instructions
            .iter()
            .map(|instruction| norito::json::value::to_value(instruction).unwrap())
            .collect::<Vec<_>>();
        assert!(structured.iter().all(norito::json::Value::is_object));
        assert!(canonical.iter().all(norito::json::Value::is_string));
        for forms in 0..4 {
            let rows = (0..instructions.len())
                .map(|index| {
                    if forms & (1 << index) == 0 {
                        structured[index].clone()
                    } else {
                        canonical[index].clone()
                    }
                })
                .collect();
            let bytes = norito::json::to_vec(&norito::json::Value::Array(rows)).unwrap();
            let temporary = localnet_test_helpers::private_tempdir().unwrap();
            let selected = selection(temporary.path(), &bytes);
            assert_eq!(load(&selected).unwrap(), instructions);
        }
    }

    #[test]
    fn authored_instructions_digest_mismatch_precedes_outputs() {
        let temporary = localnet_test_helpers::private_tempdir().unwrap();
        let mut selected = selection(temporary.path(), &source_bytes());
        selected.expected_sha256[0] ^= 1;
        assert_preflight_refuses_without_output(&selected);
    }

    #[test]
    fn authored_instructions_reject_malformed_unknown_and_noncanonical_before_outputs() {
        for bytes in [
            b"[".as_slice(),
            br#"[{"UnknownInstruction":{}}]"#.as_slice(),
            b"[]".as_slice(),
        ] {
            let temporary = localnet_test_helpers::private_tempdir().unwrap();
            assert_preflight_refuses_without_output(&selection(temporary.path(), bytes));
        }
        let domain = iroha_data_model::domain::Domain::new(
            iroha_model_base::domain::DomainId::try_new("app", "authored-fixture").unwrap(),
        );
        let mut value = genesis_instructions_json::instructions_to_value(&[InstructionBox::from(
            iroha_data_model::isi::Register::domain(domain),
        )]);
        let norito::json::Value::Array(rows) = &mut value else {
            panic!("canonical instruction array");
        };
        let norito::json::Value::Object(object) = &mut rows[0] else {
            panic!("canonical generic instruction object");
        };
        let norito::json::Value::Object(register) = object.get_mut("Register").unwrap() else {
            panic!("structured registration");
        };
        let norito::json::Value::Object(domain) = register.get_mut("Domain").unwrap() else {
            panic!("structured domain");
        };
        domain.insert(
            "unrecognized_authored_field".into(),
            norito::json::Value::Bool(true),
        );
        let temporary = localnet_test_helpers::private_tempdir().unwrap();
        let bytes = norito::json::to_vec(&value).unwrap();
        assert_preflight_refuses_without_output(&selection(temporary.path(), &bytes));
    }

    #[test]
    fn authored_instructions_reject_canonical_set_parameter_before_outputs() {
        iroha_genesis::init_instruction_registry();
        let instruction = InstructionBox::from(SetParameter::new(Parameter::Transaction(
            TransactionParameter::MaxInstructions(NonZeroU64::new(7).unwrap()),
        )));
        let bytes = norito::json::to_vec(&genesis_instructions_json::instructions_to_value(&[
            instruction,
        ]))
        .unwrap();
        let temporary = localnet_test_helpers::private_tempdir().unwrap();
        assert_preflight_refuses_without_output(&selection(temporary.path(), &bytes));
    }

    #[test]
    fn authored_instructions_reject_native_byte_and_depth_bounds_before_outputs() {
        let temporary = localnet_test_helpers::private_tempdir().unwrap();
        let mut selected = selection(temporary.path(), &source_bytes());
        fs::OpenOptions::new()
            .write(true)
            .open(&selected.path)
            .unwrap()
            .set_len(u64::try_from(GENESIS_MANIFEST_JSON_MAX_BYTES_V1).unwrap() + 1)
            .unwrap();
        selected.expected_sha256 = [0; 32];
        assert!(
            load(&selected)
                .unwrap_err()
                .chain()
                .any(|cause| cause.to_string().contains("exceeds the read bound"))
        );
        assert_preflight_refuses_without_output(&selected);
        let temporary = localnet_test_helpers::private_tempdir().unwrap();
        let depth = iroha_genesis::GENESIS_MANIFEST_JSON_MAX_DEPTH_V1 + 1;
        let bytes = format!("{}0{}", "[".repeat(depth), "]".repeat(depth)).into_bytes();
        let selected = selection(temporary.path(), &bytes);
        assert!(
            load(&selected)
                .unwrap_err()
                .chain()
                .any(|cause| cause.to_string().contains("nesting depth"))
        );
        assert_preflight_refuses_without_output(&selected);
    }

    #[cfg(unix)]
    #[test]
    fn authored_instructions_reject_symlink_hardlink_and_shared_write_custody() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let temporary = localnet_test_helpers::private_tempdir().unwrap();
        let selected = selection(temporary.path(), &source_bytes());
        let linked = temporary.path().join("linked.json");
        symlink(&selected.path, &linked).unwrap();
        assert_preflight_refuses_without_output(&LocalnetAuthoredInstructions {
            path: linked.clone(),
            expected_sha256: selected.expected_sha256,
        });
        fs::remove_file(&linked).unwrap();
        fs::hard_link(&selected.path, &linked).unwrap();
        assert_preflight_refuses_without_output(&selected);
        fs::remove_file(&linked).unwrap();
        fs::set_permissions(&selected.path, fs::Permissions::from_mode(0o666)).unwrap();
        assert_preflight_refuses_without_output(&selected);
    }
}
