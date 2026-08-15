use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use iroha_data_model::{
    account::AccountId,
    isi::sorafs::{
        CompleteReplicationOrder, ExpireReplicationOrder, IssueReplicationOrder,
        RegisterCapacityDeclaration,
    },
    metadata::Metadata,
    musubi::ArchiveId,
    prelude::{InstructionBox, Name},
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        pin_registry::{
            ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
            ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
        },
    },
};
use iroha_primitives::json::Json;
use norito::{
    decode_from_bytes,
    json::{self, Map, Value},
    to_bytes,
};
use sorafs_manifest::capacity::{CapacityDeclarationV1, ReplicationOrderV1};
use std::{env, fs, process, str::FromStr};
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };
    match command.as_str() {
        "capacity-declaration" => run_capacity_declaration(args),
        "replication-order" => run_replication_order(args),
        "complete-order" => run_complete_order(args),
        "expire-order" => run_expire_order(args),
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown subcommand `{other}`\n\n{}", usage())),
    }
}
fn usage() -> String {
    r#"usage: sorafs_tx_stdin_builder <subcommand> [options]

Subcommands:
  capacity-declaration  Convert a canonical declaration summary into `iroha ledger transaction stdin` JSON.
  replication-order     Convert a canonical replication-order summary into `iroha ledger transaction stdin` JSON.
  complete-order                Emit a completion instruction for an existing replication order.
  expire-order                  Emit a deadline-bound expiration instruction for a pending replication order.

Options:
  capacity-declaration --summary=<path>
  replication-order --summary=<path> --issued-epoch=<u64> --deadline-epoch=<u64> \
[--musubi-archive-id-hex=<64-lowercase-hex>]
  complete-order --order-id-hex=<64-hex> --provider-id-hex=<64-hex> --completion-epoch=<positive-u64> \
--expected-owner=<account-id> --assignment-revision=<positive-u64> \
--signer-policy-id-hex=<64-hex> --signer-policy-revision=<positive-u64> \
--signer-policy-predecessor-digest-hex=<64-hex; required after revision 1> \
--signer-policy-digest-hex=<64-hex> --finalized-height=<positive-u64> \
--finalized-block-hash-hex=<64-hex>
  expire-order --order-id-hex=<64-hex> --expiration-epoch=<positive-u64>
"#
    .to_owned()
}
fn run_capacity_declaration(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut summary_path = None;
    for arg in args {
        let (key, value) = split_option(&arg)?;
        match key {
            "--summary" => set_once(&mut summary_path, value.to_owned(), key)?,
            _ => return Err(format!("unknown option `{key}`")),
        }
    }
    let summary = read_json_map(summary_path.as_deref(), "declaration summary")?;
    let declaration_b64 = require_string(&summary, "declaration_b64")?;
    let declaration_bytes = BASE64_STD
        .decode(declaration_b64.as_bytes())
        .map_err(|err| format!("invalid base64 in `declaration_b64`: {err}"))?;
    let declaration: CapacityDeclarationV1 = decode_from_bytes(&declaration_bytes)
        .map_err(|err| format!("failed to decode `CapacityDeclarationV1`: {err}"))?;
    declaration
        .validate()
        .map_err(|err| format!("capacity declaration validation failed: {err}"))?;
    let canonical_bytes = to_bytes(&declaration)
        .map_err(|err| format!("failed to re-encode capacity declaration: {err}"))?;
    let metadata = metadata_from_summary(&summary)?;
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        canonical_bytes,
        declaration.committed_capacity_gib,
        require_u64(&summary, "registered_epoch")?,
        require_u64(&summary, "valid_from_epoch")?,
        require_u64(&summary, "valid_until_epoch")?,
        metadata,
    );
    print_instruction_json(InstructionBox::from(RegisterCapacityDeclaration::new(
        record,
    )))
}
fn run_replication_order(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut summary_path = None;
    let mut issued_epoch = None;
    let mut deadline_epoch = None;
    let mut musubi_archive = None;
    for arg in args {
        let (key, value) = split_option(&arg)?;
        match key {
            "--summary" => set_once(&mut summary_path, value.to_owned(), key)?,
            "--issued-epoch" => set_once(&mut issued_epoch, parse_u64(value, key)?, key)?,
            "--deadline-epoch" => set_once(&mut deadline_epoch, parse_u64(value, key)?, key)?,
            "--musubi-archive-id-hex" => set_once(
                &mut musubi_archive,
                ArchiveId::new(parse_hex_32(value, "musubi_archive_id_hex")?),
                key,
            )?,
            _ => return Err(format!("unknown option `{key}`")),
        }
    }
    let summary = read_json_map(summary_path.as_deref(), "replication order summary")?;
    let order_b64 = require_string(&summary, "replication_order_b64")?;
    let order_bytes = BASE64_STD
        .decode(order_b64.as_bytes())
        .map_err(|err| format!("invalid base64 in `order_b64`: {err}"))?;
    let order: ReplicationOrderV1 = decode_from_bytes(&order_bytes)
        .map_err(|err| format!("failed to decode `ReplicationOrderV1`: {err}"))?;
    order
        .validate()
        .map_err(|err| format!("replication order validation failed: {err}"))?;
    let issued_epoch = issued_epoch.ok_or_else(|| "missing `--issued-epoch=<u64>`".to_owned())?;
    let deadline_epoch =
        deadline_epoch.ok_or_else(|| "missing `--deadline-epoch=<u64>`".to_owned())?;
    let instruction = IssueReplicationOrder::new(
        ReplicationOrderId::new(order.order_id),
        to_bytes(&order).map_err(|err| format!("failed to re-encode replication order: {err}"))?,
        issued_epoch,
        deadline_epoch,
    );
    let instruction = match musubi_archive {
        Some(archive_id) => instruction.for_musubi_archive(archive_id),
        None => instruction,
    };
    print_instruction_json(InstructionBox::from(instruction))
}
fn run_complete_order(args: impl Iterator<Item = String>) -> Result<(), String> {
    print_instruction_json(complete_order_instruction(args)?)
}
fn complete_order_instruction(
    args: impl Iterator<Item = String>,
) -> Result<InstructionBox, String> {
    let mut order_id_hex = None;
    let mut provider_id_hex = None;
    let mut completion_epoch = None;
    let mut expected_owner = None;
    let mut assignment_revision = None;
    let mut signer_policy_id_hex = None;
    let mut signer_policy_revision = None;
    let mut signer_policy_predecessor_digest_hex = None;
    let mut signer_policy_digest_hex = None;
    let mut finalized_height = None;
    let mut finalized_block_hash_hex = None;
    for arg in args {
        let (key, value) = split_option(&arg)?;
        match key {
            "--order-id-hex" => set_once(&mut order_id_hex, value.to_owned(), key)?,
            "--provider-id-hex" => set_once(&mut provider_id_hex, value.to_owned(), key)?,
            "--completion-epoch" => set_once(&mut completion_epoch, parse_u64(value, key)?, key)?,
            "--expected-owner" => {
                let parsed = AccountId::parse_encoded(value)
                    .map_err(|error| format!("invalid `--expected-owner` account ID: {error}"))?;
                if parsed.canonical() != value {
                    return Err(
                        "`--expected-owner` must be an exact canonical I105 account ID".to_owned(),
                    );
                }
                let owner = parsed.into_account_id();
                set_once(&mut expected_owner, owner, key)?;
            }
            "--assignment-revision" => {
                set_once(&mut assignment_revision, parse_u64(value, key)?, key)?;
            }
            "--signer-policy-id-hex" => {
                set_once(&mut signer_policy_id_hex, value.to_owned(), key)?;
            }
            "--signer-policy-revision" => {
                set_once(&mut signer_policy_revision, parse_u64(value, key)?, key)?;
            }
            "--signer-policy-predecessor-digest-hex" => {
                set_once(
                    &mut signer_policy_predecessor_digest_hex,
                    value.to_owned(),
                    key,
                )?;
            }
            "--signer-policy-digest-hex" => {
                set_once(&mut signer_policy_digest_hex, value.to_owned(), key)?;
            }
            "--finalized-height" => {
                set_once(&mut finalized_height, parse_u64(value, key)?, key)?;
            }
            "--finalized-block-hash-hex" => {
                set_once(&mut finalized_block_hash_hex, value.to_owned(), key)?;
            }
            _ => return Err(format!("unknown option `{key}`")),
        }
    }
    let order_id_hex = order_id_hex.ok_or_else(|| "missing `--order-id-hex=<hex>`".to_owned())?;
    let order_id = parse_hex_32(&order_id_hex, "order_id_hex")?;
    let provider_id_hex =
        provider_id_hex.ok_or_else(|| "missing `--provider-id-hex=<hex>`".to_owned())?;
    let provider_id = parse_hex_32(&provider_id_hex, "provider_id_hex")?;
    let completion_epoch = require_positive(completion_epoch, "--completion-epoch")?;
    let expected_owner =
        expected_owner.ok_or_else(|| "missing `--expected-owner=<account-id>`".to_owned())?;
    let assignment_revision = require_positive(assignment_revision, "--assignment-revision")?;
    let signer_policy_id_hex = signer_policy_id_hex
        .ok_or_else(|| "missing `--signer-policy-id-hex=<64-hex>`".to_owned())?;
    let signer_policy_revision =
        require_positive(signer_policy_revision, "--signer-policy-revision")?;
    let signer_policy_digest_hex = signer_policy_digest_hex
        .ok_or_else(|| "missing `--signer-policy-digest-hex=<64-hex>`".to_owned())?;
    let finalized_height = require_positive(finalized_height, "--finalized-height")?;
    let finalized_block_hash_hex = finalized_block_hash_hex
        .ok_or_else(|| "missing `--finalized-block-hash-hex=<64-hex>`".to_owned())?;
    let signer_policy = ProviderIngestCompletionSignerPolicyV1 {
        policy_id: parse_hex_32(&signer_policy_id_hex, "signer_policy_id_hex")?,
        revision: signer_policy_revision,
        predecessor_digest: if signer_policy_revision == 1 {
            if signer_policy_predecessor_digest_hex.is_some() {
                return Err(
                    "`--signer-policy-predecessor-digest-hex` is forbidden at revision 1"
                        .to_owned(),
                );
            }
            None
        } else {
            let predecessor_hex = signer_policy_predecessor_digest_hex.ok_or_else(|| {
                "missing `--signer-policy-predecessor-digest-hex=<64-hex>`".to_owned()
            })?;
            Some(parse_hex_32(
                &predecessor_hex,
                "signer_policy_predecessor_digest_hex",
            )?)
        },
        policy_digest: parse_hex_32(&signer_policy_digest_hex, "signer_policy_digest_hex")?,
    };
    let expected_authority =
        ProviderIngestCompletionAuthorityV1::new(expected_owner, signer_policy);
    let finalized_anchor = ProviderIngestFinalizedAnchorV1 {
        height: finalized_height,
        block_hash: parse_hex_32(&finalized_block_hash_hex, "finalized_block_hash_hex")?,
    };
    if !expected_authority.is_valid() || !finalized_anchor.is_valid() {
        return Err("completion authority and finalized anchor must be canonical".to_owned());
    }
    Ok(InstructionBox::from(CompleteReplicationOrder::new(
        ReplicationOrderId::new(order_id),
        ProviderId::new(provider_id),
        completion_epoch,
        expected_authority,
        assignment_revision,
        finalized_anchor,
    )))
}
fn run_expire_order(args: impl Iterator<Item = String>) -> Result<(), String> {
    print_instruction_json(expire_order_instruction(args)?)
}
fn expire_order_instruction(args: impl Iterator<Item = String>) -> Result<InstructionBox, String> {
    let mut order_id_hex = None;
    let mut expiration_epoch = None;
    for arg in args {
        let (key, value) = split_option(&arg)?;
        match key {
            "--order-id-hex" => set_once(&mut order_id_hex, value.to_owned(), key)?,
            "--expiration-epoch" => {
                let epoch = parse_u64(value, key)?;
                if epoch == 0 {
                    return Err("`--expiration-epoch` must be greater than zero".to_owned());
                }
                set_once(&mut expiration_epoch, epoch, key)?;
            }
            _ => return Err(format!("unknown option `{key}`")),
        }
    }
    let order_id_hex = order_id_hex.ok_or_else(|| "missing `--order-id-hex=<hex>`".to_owned())?;
    let order_id = parse_hex_32(&order_id_hex, "order_id_hex")?;
    let expiration_epoch =
        expiration_epoch.ok_or_else(|| "missing `--expiration-epoch=<positive-u64>`".to_owned())?;
    Ok(InstructionBox::from(ExpireReplicationOrder::new(
        ReplicationOrderId::new(order_id),
        expiration_epoch,
    )))
}
fn split_option(arg: &str) -> Result<(&str, &str), String> {
    arg.split_once('=')
        .ok_or_else(|| format!("expected `--key=value`, got `{arg}`"))
}
fn set_once<T>(slot: &mut Option<T>, value: T, key: &str) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("duplicate `{key}` option"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}
fn read_json_map(path: Option<&str>, label: &str) -> Result<Map, String> {
    let path = path.ok_or_else(|| format!("missing `--summary=<path>` for {label}"))?;
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read `{path}` for {label}: {err}"))?;
    let value: Value = json::from_slice(&bytes)
        .map_err(|err| format!("failed to parse JSON `{path}` for {label}: {err}"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| format!("{label} `{path}` must be a JSON object"))
}
fn require_string<'a>(map: &'a Map, key: &str) -> Result<&'a str, String> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or invalid string field `{key}`"))
}
fn require_u64(map: &Map, key: &str) -> Result<u64, String> {
    map.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing or invalid integer field `{key}`"))
}
fn parse_u64(value: &str, label: &str) -> Result<u64, String> {
    require_canonical_unsigned_decimal(value, label)?;
    value
        .parse::<u64>()
        .map_err(|err| format!("invalid `{label}` value `{value}`: {err}"))
}
fn require_positive(value: Option<u64>, label: &str) -> Result<u64, String> {
    let value = value.ok_or_else(|| format!("missing `{label}=<positive-u64>`"))?;
    if value == 0 {
        return Err(format!("`{label}` must be greater than zero"));
    }
    Ok(value)
}
fn parse_hex_32(value: &str, label: &str) -> Result<[u8; 32], String> {
    require_lowercase_fixed_hex(value, label, 64)?;
    let decoded = hex::decode(value).map_err(|err| format!("invalid `{label}` hex: {err}"))?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| format!("`{label}` must be exactly 32 bytes (64 hex chars)"))?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(format!("`{label}` must not be all zero"));
    }
    Ok(bytes)
}
fn require_canonical_unsigned_decimal(value: &str, label: &str) -> Result<(), String> {
    if is_canonical_unsigned_decimal(value) {
        Ok(())
    } else {
        Err(format!(
            "`{label}` value must be a canonical unsigned decimal integer"
        ))
    }
}
fn is_canonical_unsigned_decimal(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.iter().all(u8::is_ascii_digit)
        && (bytes.len() == 1 || bytes[0] != b'0')
}
fn require_lowercase_fixed_hex(
    value: &str,
    label: &str,
    expected_len: usize,
) -> Result<(), String> {
    if value.len() != expected_len {
        return Err(format!(
            "`{label}` must be exactly {} bytes ({} hex chars)",
            expected_len / 2,
            expected_len
        ));
    }
    if value
        .as_bytes()
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        Ok(())
    } else {
        Err(format!(
            "`{label}` must be lowercase fixed-width hex without prefixes or whitespace"
        ))
    }
}
fn metadata_from_summary(summary: &Map) -> Result<Metadata, String> {
    let mut metadata = Metadata::default();
    let Some(entries) = summary.get("metadata") else {
        return Ok(metadata);
    };
    let object = entries
        .as_object()
        .ok_or_else(|| "`metadata` must be an object".to_owned())?;
    for (key, value) in object {
        let name =
            Name::from_str(key).map_err(|err| format!("metadata key `{key}` is invalid: {err}"))?;
        metadata.insert(name, Json::new(value.clone()));
    }
    Ok(metadata)
}
fn print_instruction_json(instruction: InstructionBox) -> Result<(), String> {
    let encoded = to_bytes(&instruction)
        .map_err(|err| format!("failed to encode instruction payload: {err}"))?;
    let payload = Value::Array(vec![Value::String(BASE64_STD.encode(encoded))]);
    let rendered = json::to_string(&payload)
        .map_err(|err| format!("failed to serialize tx-stdin JSON: {err}"))?;
    println!("{rendered}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    const OWNER_I105: &str = "sorauﾛ1Pｶt8ｵgｷﾗﾗｸ5ﾕﾆヰﾁｳヱﾜｦヱLLﾉVｾﾕXｹｼﾘnﾉﾊjｸ9eQL2MVG9T";
    #[test]
    fn parse_u64_rejects_noncanonical_epoch_tokens() {
        assert_eq!(parse_u64("0", "--issued-epoch").expect("zero"), 0);
        assert_eq!(parse_u64("580", "--issued-epoch").expect("epoch"), 580);
        for value in [
            "",
            "00",
            "0580",
            "+580",
            "580 ",
            " 580",
            "18446744073709551616",
        ] {
            let err = parse_u64(value, "--issued-epoch").expect_err("invalid epoch must fail");
            assert!(
                err.contains("--issued-epoch"),
                "unexpected error for {value:?}: {err}"
            );
        }
    }
    #[test]
    fn parse_hex_32_rejects_noncanonical_order_ids() {
        let canonical = "5555555555555555555555555555555555555555555555555555555555555555";
        assert_eq!(
            parse_hex_32(canonical, "order_id_hex").expect("canonical order id"),
            [0x55; 32]
        );
        for value in [
            "",
            "5555",
            "555555555555555555555555555555555555555555555555555555555555555",
            "0x5555555555555555555555555555555555555555555555555555555555555555",
            "555555555555555555555555555555555555555555555555555555555555555G",
            "555555555555555555555555555555555555555555555555555555555555555A",
            "555555555555555555555555555555555555555555555555555555555555555 ",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ] {
            let err = parse_hex_32(value, "order_id_hex").expect_err("invalid order id must fail");
            assert!(
                err.contains("order_id_hex"),
                "unexpected error for {value:?}: {err}"
            );
        }
    }
    #[test]
    fn set_once_rejects_duplicate_options() {
        let mut slot = None;
        set_once(&mut slot, 580_u64, "--issued-epoch").expect("first value");
        let err =
            set_once(&mut slot, 581_u64, "--issued-epoch").expect_err("duplicate option must fail");
        assert!(err.contains("duplicate `--issued-epoch` option"));
        assert_eq!(slot, Some(580));
    }
    #[test]
    fn expire_order_builds_canonical_instruction_and_rejects_bad_epochs() {
        let order_id = "5555555555555555555555555555555555555555555555555555555555555555";
        let actual = expire_order_instruction(
            [
                format!("--order-id-hex={order_id}"),
                "--expiration-epoch=91".to_owned(),
            ]
            .into_iter(),
        )
        .expect("build expiration instruction");
        let expected = InstructionBox::from(ExpireReplicationOrder::new(
            ReplicationOrderId::new([0x55; 32]),
            91,
        ));
        assert_eq!(
            to_bytes(&actual).expect("encode actual instruction"),
            to_bytes(&expected).expect("encode expected instruction")
        );
        for args in [
            vec![format!("--order-id-hex={order_id}")],
            vec![
                format!("--order-id-hex={order_id}"),
                "--expiration-epoch=0".to_owned(),
            ],
            vec![
                format!("--order-id-hex={order_id}"),
                "--expiration-epoch=1".to_owned(),
                "--expiration-epoch=2".to_owned(),
            ],
        ] {
            assert!(
                expire_order_instruction(args.into_iter()).is_err(),
                "invalid expiration arguments must fail"
            );
        }
    }
    #[test]
    fn complete_order_requires_and_encodes_exact_commit_context() {
        let args = [
            format!("--order-id-hex={}", "11".repeat(32)),
            format!("--provider-id-hex={}", "22".repeat(32)),
            "--completion-epoch=25".to_owned(),
            format!("--expected-owner={OWNER_I105}"),
            "--assignment-revision=3".to_owned(),
            format!("--signer-policy-id-hex={}", "33".repeat(32)),
            "--signer-policy-revision=2".to_owned(),
            format!("--signer-policy-predecessor-digest-hex={}", "44".repeat(32)),
            format!("--signer-policy-digest-hex={}", "55".repeat(32)),
            "--finalized-height=19".to_owned(),
            format!("--finalized-block-hash-hex={}", "66".repeat(32)),
        ];
        let actual =
            complete_order_instruction(args.clone().into_iter()).expect("build exact completion");
        let owner = AccountId::parse_encoded(OWNER_I105)
            .expect("fixture owner")
            .into_account_id();
        let expected = InstructionBox::from(CompleteReplicationOrder::new(
            ReplicationOrderId::new([0x11; 32]),
            ProviderId::new([0x22; 32]),
            25,
            ProviderIngestCompletionAuthorityV1::new(
                owner,
                ProviderIngestCompletionSignerPolicyV1 {
                    policy_id: [0x33; 32],
                    revision: 2,
                    predecessor_digest: Some([0x44; 32]),
                    policy_digest: [0x55; 32],
                },
            ),
            3,
            ProviderIngestFinalizedAnchorV1 {
                height: 19,
                block_hash: [0x66; 32],
            },
        ));
        assert_eq!(
            to_bytes(&actual).expect("encode actual"),
            to_bytes(&expected).expect("encode expected")
        );
        for noncanonical_owner in [
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C".to_owned(),
            format!(" {OWNER_I105}"),
            format!("{OWNER_I105} "),
        ] {
            let mut invalid = args.clone();
            invalid[3] = format!("--expected-owner={noncanonical_owner}");
            assert!(
                complete_order_instruction(invalid.into_iter()).is_err(),
                "noncanonical expected owner must fail"
            );
        }
    }
    #[test]
    fn complete_order_rejects_noncanonical_policy_predecessor_shape() {
        let base = vec![
            format!("--order-id-hex={}", "11".repeat(32)),
            format!("--provider-id-hex={}", "22".repeat(32)),
            "--completion-epoch=25".to_owned(),
            format!("--expected-owner={OWNER_I105}"),
            "--assignment-revision=3".to_owned(),
            format!("--signer-policy-id-hex={}", "33".repeat(32)),
            format!("--signer-policy-digest-hex={}", "55".repeat(32)),
            "--finalized-height=19".to_owned(),
            format!("--finalized-block-hash-hex={}", "66".repeat(32)),
        ];
        let mut missing_predecessor = base.clone();
        missing_predecessor.push("--signer-policy-revision=2".to_owned());
        assert!(complete_order_instruction(missing_predecessor.into_iter()).is_err());
        let mut forbidden_predecessor = base;
        forbidden_predecessor.push("--signer-policy-revision=1".to_owned());
        forbidden_predecessor.push(format!(
            "--signer-policy-predecessor-digest-hex={}",
            "44".repeat(32)
        ));
        assert!(complete_order_instruction(forbidden_predecessor.into_iter()).is_err());
    }
}
