//! Exercise offline dispute authoring through the shipping CLI.

use iroha_data_model::soranet::incentives::{
    RelayRewardDisputeStatusV1, RelayRewardDisputeV1, RelayRewardInstructionV1,
};

use super::{account_id, account_literal, command, fs, torii_mock_support::TempDir, xor_asset_id};

#[test]
fn authors_dispute_from_public_instruction_without_unused_treasury() {
    let directory = TempDir::new("incentives_dispute").expect("temporary directory");
    let instruction_path = directory.path().join("instruction.to");
    let dispute_path = directory.path().join("dispute.to");
    let instruction = RelayRewardInstructionV1 {
        relay_id: [7; 32],
        epoch: 42,
        beneficiary: account_id("beneficiary"),
        payout_asset_id: xor_asset_id(),
        payout_amount: 10_u64.into(),
        reward_score: 1_000,
        budget_approval_id: None,
        metadata: Default::default(),
    };
    fs::write(&instruction_path, norito::to_bytes(&instruction).unwrap()).unwrap();
    let submitter = account_id("operator");
    let reason = "missing measured bandwidth";
    for format in ["json", "text"] {
        let mut cli = command();
        cli.args(["--output-format", format])
            .args([
                "app",
                "sorafs",
                "incentives",
                "open-dispute",
                "--instruction",
            ])
            .arg(&instruction_path)
            .args([
                "--submitted-by",
                &account_literal(&submitter),
                "--requested-amount",
                "12",
                "--submitted-at",
                "1234",
                "--reason",
                reason,
                "--pretty",
                "--norito-out",
            ])
            .arg(&dispute_path);
        let output = cli.output().expect("execute offline dispute authoring");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: RelayRewardDisputeV1 =
            norito::json::from_slice(&output.stdout).unwrap_or_else(|error| {
                panic!(
                    "{format} stdout is not dispute JSON: {error}; stdout={:?}; stderr={:?}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        let binary: RelayRewardDisputeV1 =
            norito::decode_from_bytes(&fs::read(&dispute_path).unwrap()).unwrap();
        assert_eq!(json, binary);
        assert_eq!(json.original_instruction, instruction);
        assert_eq!(json.relay_id, instruction.relay_id);
        assert_eq!(json.epoch, instruction.epoch);
        assert_eq!(json.submitted_by, submitter);
        assert_eq!(json.requested_amount, 12_u64.into());
        assert_eq!(json.submitted_at_unix, 1234);
        assert_eq!(json.reason, reason);
        assert_eq!(json.status, RelayRewardDisputeStatusV1::Pending);
        assert!(json.resolution_metadata.is_empty());
    }

    let rejected = command()
        .args([
            "app",
            "sorafs",
            "incentives",
            "open-dispute",
            "--treasury-account",
            &account_literal(&submitter),
        ])
        .output()
        .expect("reject removed unused option");
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("unexpected argument '--treasury-account'")
    );
}
