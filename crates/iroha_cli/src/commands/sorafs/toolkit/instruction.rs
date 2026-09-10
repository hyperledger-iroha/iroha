//! Offline SoraFS instruction preparation for `iroha ledger transaction stdin`.
//!
//! Typed command options feed the canonical instruction builders. This capability
//! writes one instruction array without client configuration, signing, network
//! access or transaction submission.

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
use std::{
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

/// Prepare one canonical SoraFS instruction as transaction-stdin JSON.
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Convert a canonical capacity-declaration summary into an instruction.
    CapacityDeclaration(CapacityDeclarationArgs),
    /// Convert a canonical replication-order summary into an instruction.
    ReplicationOrder(ReplicationOrderArgs),
    /// Bind a completion to its authority, assignment and finalized anchor.
    CompleteOrder(CompleteOrderArgs),
    /// Expire a pending replication order at an explicit epoch.
    ExpireOrder(ExpireOrderArgs),
}

/// A canonical capacity declaration and its registration metadata.
#[derive(Debug, clap::Args)]
pub struct CapacityDeclarationArgs {
    /// JSON summary containing declaration_b64 and registered_epoch.
    #[arg(long, value_name = "PATH")]
    summary: PathBuf,
}

/// A canonical replication order and optional Musubi archive binding.
#[derive(Debug, clap::Args)]
pub struct ReplicationOrderArgs {
    /// JSON summary containing replication_order_b64.
    #[arg(long, value_name = "PATH")]
    summary: PathBuf,
    /// Nonzero archive ID in exactly 64 lowercase hex characters.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    musubi_archive_id_hex: Option<[u8; 32]>,
}

/// The complete authority and finalized context for a replication completion.
#[derive(Debug, clap::Args)]
pub struct CompleteOrderArgs {
    /// Nonzero order ID in exactly 64 lowercase hex characters.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    order_id_hex: [u8; 32],
    /// Nonzero provider ID in exactly 64 lowercase hex characters.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    provider_id_hex: [u8; 32],
    /// Completion epoch as a positive canonical unsigned decimal integer.
    #[arg(long, value_parser = parse_positive_u64)]
    completion_epoch: NonZeroU64,
    /// Exact canonical I105 account ID of the expected owner.
    #[arg(long, value_name = "ACCOUNT_ID", value_parser = parse_canonical_owner)]
    expected_owner: AccountId,
    /// Positive canonical assignment revision.
    #[arg(long, value_parser = parse_positive_u64)]
    assignment_revision: NonZeroU64,
    /// Nonzero signer policy ID in exactly 64 lowercase hex characters.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    signer_policy_id_hex: [u8; 32],
    /// Positive canonical signer policy revision.
    #[arg(long, value_parser = parse_positive_u64)]
    signer_policy_revision: NonZeroU64,
    /// Nonzero predecessor digest; required after revision 1 and forbidden at 1.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    signer_policy_predecessor_digest_hex: Option<[u8; 32]>,
    /// Nonzero signer policy digest in exactly 64 lowercase hex characters.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    signer_policy_digest_hex: [u8; 32],
    /// Positive canonical finalized block height.
    #[arg(long, value_parser = parse_positive_u64)]
    finalized_height: NonZeroU64,
    /// Nonzero finalized block hash in exactly 64 lowercase hex characters.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    finalized_block_hash_hex: [u8; 32],
}

/// A pending replication order and its explicit expiration epoch.
#[derive(Debug, clap::Args)]
pub struct ExpireOrderArgs {
    /// Nonzero order ID in exactly 64 lowercase hex characters.
    #[arg(long, value_name = "HEX", value_parser = parse_nonzero_digest)]
    order_id_hex: [u8; 32],
    /// Expiration epoch as a positive canonical unsigned decimal integer.
    #[arg(long, value_parser = parse_positive_u64)]
    expiration_epoch: NonZeroU64,
}

impl Command {
    /// Run local instruction preparation, preserving clean stdout and status.
    pub(super) fn run(self) -> ExitCode {
        let result = match self {
            Self::CapacityDeclaration(args) => run_capacity_declaration(args),
            Self::ReplicationOrder(args) => run_replication_order(args),
            Self::CompleteOrder(args) => {
                complete_order_instruction(args).and_then(print_instruction_json)
            }
            Self::ExpireOrder(args) => print_instruction_json(expire_order_instruction(args)),
        };
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        }
    }
}

fn run_capacity_declaration(args: CapacityDeclarationArgs) -> Result<(), String> {
    let summary = read_json_map(&args.summary, "declaration summary")?;
    for redundant in ["valid_from_epoch", "valid_until_epoch"] {
        if summary.contains_key(redundant) {
            return Err(format!(
                "`{redundant}` is derived from the canonical capacity payload and must be omitted"
            ));
        }
    }
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
        declaration.valid_from,
        declaration.valid_until,
        metadata,
    );
    print_instruction_json(InstructionBox::from(RegisterCapacityDeclaration::new(
        record,
    )))
}
fn run_replication_order(args: ReplicationOrderArgs) -> Result<(), String> {
    let musubi_archive = args.musubi_archive_id_hex.map(ArchiveId::new);
    let summary = read_json_map(&args.summary, "replication order summary")?;
    let order_b64 = require_string(&summary, "replication_order_b64")?;
    let order_bytes = BASE64_STD
        .decode(order_b64.as_bytes())
        .map_err(|err| format!("invalid base64 in `order_b64`: {err}"))?;
    let order: ReplicationOrderV1 = decode_from_bytes(&order_bytes)
        .map_err(|err| format!("failed to decode `ReplicationOrderV1`: {err}"))?;
    order
        .validate()
        .map_err(|err| format!("replication order validation failed: {err}"))?;
    let order_id = ReplicationOrderId::new(order.order_id);
    if order_id.is_auto() {
        return Err(
            "generic replication-order builder cannot issue a reserved automatic order id"
                .to_owned(),
        );
    }
    let instruction = IssueReplicationOrder::new(
        order_id,
        to_bytes(&order).map_err(|err| format!("failed to re-encode replication order: {err}"))?,
        order.issued_at,
        order.deadline_at,
    );
    let instruction = match musubi_archive {
        Some(archive_id) => instruction.for_musubi_archive(archive_id),
        None => instruction,
    };
    print_instruction_json(InstructionBox::from(instruction))
}
fn complete_order_instruction(args: CompleteOrderArgs) -> Result<InstructionBox, String> {
    let signer_policy_revision = args.signer_policy_revision.get();
    let signer_policy = ProviderIngestCompletionSignerPolicyV1 {
        policy_id: args.signer_policy_id_hex,
        revision: signer_policy_revision,
        predecessor_digest: if signer_policy_revision == 1 {
            if args.signer_policy_predecessor_digest_hex.is_some() {
                return Err(
                    "`--signer-policy-predecessor-digest-hex` is forbidden at revision 1"
                        .to_owned(),
                );
            }
            None
        } else {
            Some(args.signer_policy_predecessor_digest_hex.ok_or_else(|| {
                "missing `--signer-policy-predecessor-digest-hex=<64-hex>`".to_owned()
            })?)
        },
        policy_digest: args.signer_policy_digest_hex,
    };
    let expected_authority =
        ProviderIngestCompletionAuthorityV1::new(args.expected_owner, signer_policy);
    let finalized_anchor = ProviderIngestFinalizedAnchorV1 {
        height: args.finalized_height.get(),
        block_hash: args.finalized_block_hash_hex,
    };
    if !expected_authority.is_valid() || !finalized_anchor.is_valid() {
        return Err("completion authority and finalized anchor must be canonical".to_owned());
    }
    Ok(InstructionBox::from(CompleteReplicationOrder::new(
        ReplicationOrderId::new(args.order_id_hex),
        ProviderId::new(args.provider_id_hex),
        args.completion_epoch.get(),
        expected_authority,
        args.assignment_revision.get(),
        finalized_anchor,
    )))
}

fn expire_order_instruction(args: ExpireOrderArgs) -> InstructionBox {
    InstructionBox::from(ExpireReplicationOrder::new(
        ReplicationOrderId::new(args.order_id_hex),
        args.expiration_epoch.get(),
    ))
}

fn parse_nonzero_digest(value: &str) -> Result<[u8; 32], String> {
    parse_hex_32(value, "hex value")
}

fn parse_positive_u64(value: &str) -> Result<NonZeroU64, String> {
    NonZeroU64::new(parse_u64(value, "value")?)
        .ok_or_else(|| "value must be greater than zero".to_owned())
}

fn parse_canonical_owner(value: &str) -> Result<AccountId, String> {
    let parsed = AccountId::parse_encoded(value)
        .map_err(|error| format!("invalid `--expected-owner` account ID: {error}"))?;
    if parsed.to_string() != value {
        return Err("`--expected-owner` must be an exact canonical I105 account ID".to_owned());
    }
    Ok(parsed)
}

fn read_json_map(path: &Path, label: &str) -> Result<Map, String> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read `{}` for {label}: {err}", path.display()))?;
    let path = path.display();
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
    use clap::Parser as _;

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: Command,
    }

    fn parse_command(
        operation: &str,
        args: impl Iterator<Item = String>,
    ) -> Result<Command, String> {
        TestCli::try_parse_from(
            ["instruction".to_owned(), operation.to_owned()]
                .into_iter()
                .chain(args),
        )
        .map(|cli| cli.command)
        .map_err(|error| error.to_string())
    }

    fn parse_complete_instruction(
        args: impl Iterator<Item = String>,
    ) -> Result<InstructionBox, String> {
        let Command::CompleteOrder(args) = parse_command("complete-order", args)? else {
            unreachable!("selected complete-order")
        };
        complete_order_instruction(args)
    }

    fn parse_expire_instruction(
        args: impl Iterator<Item = String>,
    ) -> Result<InstructionBox, String> {
        let Command::ExpireOrder(args) = parse_command("expire-order", args)? else {
            unreachable!("selected expire-order")
        };
        Ok(expire_order_instruction(args))
    }

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
    fn typed_options_reject_duplicate_values() {
        use clap::CommandFactory as _;
        TestCli::command().debug_assert();
        let order = format!("--order-id-hex={}", "55".repeat(32));
        let Command::ExpireOrder(first) = parse_command(
            "expire-order",
            [order.clone(), "--expiration-epoch=580".to_owned()].into_iter(),
        )
        .expect("first value") else {
            unreachable!("selected expire-order")
        };
        let err = parse_command(
            "expire-order",
            [
                order,
                "--expiration-epoch=580".to_owned(),
                "--expiration-epoch=581".to_owned(),
            ]
            .into_iter(),
        )
        .expect_err("duplicate option must fail");
        assert!(
            err.contains("cannot be used multiple times") && err.contains("--expiration-epoch")
        );
        assert_eq!(first.expiration_epoch.get(), 580);
    }
    #[test]
    fn expire_order_builds_canonical_instruction_and_rejects_bad_epochs() {
        let order_id = "5555555555555555555555555555555555555555555555555555555555555555";
        let actual = parse_expire_instruction(
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
                parse_expire_instruction(args.into_iter()).is_err(),
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
            parse_complete_instruction(args.clone().into_iter()).expect("build exact completion");
        let owner = AccountId::parse_encoded(OWNER_I105).expect("fixture owner");
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
                parse_complete_instruction(invalid.into_iter()).is_err(),
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
        assert!(parse_complete_instruction(missing_predecessor.into_iter()).is_err());
        let mut initial_policy = base.clone();
        initial_policy.push("--signer-policy-revision=1".to_owned());
        assert!(parse_complete_instruction(initial_policy.into_iter()).is_ok());
        let mut forbidden_predecessor = base;
        forbidden_predecessor.push("--signer-policy-revision=1".to_owned());
        forbidden_predecessor.push(format!(
            "--signer-policy-predecessor-digest-hex={}",
            "44".repeat(32)
        ));
        assert!(parse_complete_instruction(forbidden_predecessor.into_iter()).is_err());
    }
}
