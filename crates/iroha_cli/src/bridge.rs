use clap::Subcommand;
use eyre::Result;
use iroha::data_model::prelude::*;

use crate::RunContext;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Emit a bridge receipt as a typed event
    EmitReceipt(EmitReceiptArgs),
}

#[derive(clap::Args, Debug)]
pub struct EmitReceiptArgs {
    /// Bridge lane id (numeric)
    #[arg(long)]
    lane: u32,
    /// Direction: lock|mint|burn|release
    #[arg(long)]
    direction: String,
    /// Source tx hash (hex, 32 bytes)
    #[arg(long)]
    source_tx: String,
    /// Amount (integer units)
    #[arg(long)]
    amount: u128,
    /// Asset id (Iroha canonical), e.g., "wBTC#btc"
    #[arg(long)]
    asset_id: String,
    /// Recipient (Iroha account id or external address payload)
    #[arg(long)]
    recipient: String,
    /// Optional destination tx hash (hex, 32 bytes)
    #[arg(long)]
    dest_tx: Option<String>,
    /// Proof hash (hex, 32 bytes)
    #[arg(long)]
    proof_hash: Option<String>,
}

pub fn run(ctx: &mut impl RunContext, cmd: Command) -> Result<()> {
    match cmd {
        Command::EmitReceipt(args) => emit_receipt(ctx, args),
    }
}

fn hex32(s: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(s.trim_start_matches("0x"))?;
    let mut out = [0u8; 32];
    if bytes.len() != 32 {
        return Err(eyre::eyre!("expected 32 bytes, got {}", bytes.len()));
    }
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn emit_receipt(ctx: &mut impl RunContext, a: EmitReceiptArgs) -> Result<()> {
    let source_tx = hex32(&a.source_tx)?;
    let dest_tx = match &a.dest_tx {
        Some(h) => Some(hex32(h)?),
        None => None,
    };
    let proof_hash = match &a.proof_hash {
        Some(h) => hex32(h)?,
        None => [0u8; 32],
    };
    let receipt = BridgeReceipt {
        lane: LaneId::new(a.lane),
        direction: a.direction.into_bytes(),
        source_tx,
        dest_tx,
        proof_hash,
        amount: a.amount,
        asset_id: a.asset_id.into_bytes(),
        recipient: a.recipient.into_bytes(),
    };
    let isi = RecordBridgeReceipt::new(receipt);
    ctx.finish(vec![InstructionBox::from(isi)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_i18n::{Bundle, Language, Localizer};
    use url::Url;

    struct TestContext {
        cfg: iroha::config::Config,
        i18n: Localizer,
        captured: Option<Executable>,
    }

    impl TestContext {
        fn new() -> Self {
            let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let account_id = AccountId::new(key_pair.public_key().clone());
            let cfg = iroha::config::Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account: account_id,
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                i18n: Localizer::new(Bundle::Cli, Language::English),
                captured: None,
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, _data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            Ok(())
        }

        fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
            Ok(())
        }

        fn finish_with_mode(
            &mut self,
            instructions: impl Into<Executable>,
            _wait_for_confirmation: bool,
        ) -> Result<()> {
            self.captured = Some(instructions.into());
            Ok(())
        }
    }

    #[test]
    fn emit_receipt_builds_record_bridge_receipt() {
        let mut ctx = TestContext::new();
        let args = EmitReceiptArgs {
            lane: 3,
            direction: "mint".to_string(),
            source_tx: "11".repeat(32),
            amount: 5,
            asset_id: "wBTC#btc".to_string(),
            recipient: "alice@main".to_string(),
            dest_tx: Some("22".repeat(32)),
            proof_hash: Some("33".repeat(32)),
        };

        emit_receipt(&mut ctx, args).expect("emit receipt");

        let executable = ctx.captured.expect("captured executable");
        let Executable::Instructions(instructions) = executable else {
            panic!("expected instruction executable");
        };
        let instructions = instructions.into_vec();
        assert_eq!(instructions.len(), 1, "expected one instruction");

        let record = instructions[0]
            .as_any()
            .downcast_ref::<RecordBridgeReceipt>()
            .expect("record bridge receipt instruction");
        let receipt = &record.receipt;
        assert_eq!(receipt.lane, LaneId::new(3));
        assert_eq!(receipt.direction, b"mint".to_vec());
        assert_eq!(receipt.source_tx, [0x11; 32]);
        assert_eq!(receipt.dest_tx, Some([0x22; 32]));
        assert_eq!(receipt.proof_hash, [0x33; 32]);
        assert_eq!(receipt.amount, 5);
        assert_eq!(receipt.asset_id, b"wBTC#btc".to_vec());
        assert_eq!(receipt.recipient, b"alice@main".to_vec());
    }
}
