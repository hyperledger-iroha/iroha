//! Bridge commands: genesis readiness probes and typed bridge receipts.
//!
//! The retired SCCP subcommands are gone; SCCP v1 has its own `iroha sccp` command group
//! (specs/sccp.md §10).
// TODO(ws42): the SCCP v1 wallet commands live under `iroha sccp`, not under `ops bridge`.
use crate::{Run, RunContext};
mod genesis_readiness;
use clap::Subcommand;
use eyre::Result;
use iroha::data_model::prelude::*;
use iroha_model_base::topology::LaneId;
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Probe the original node and genesis once with authenticated readiness evidence.
    GenesisReadiness(genesis_readiness::Args),
    /// Emit a bridge receipt as a typed event.
    EmitReceipt(EmitReceiptArgs),
}
#[derive(clap::Args, Debug)]
pub struct EmitReceiptArgs {
    /// Bridge lane id (numeric).
    #[arg(long)]
    lane: u32,
    /// Direction: lock|mint|burn|release.
    #[arg(long)]
    direction: String,
    /// Source transaction hash (hex, 32 bytes).
    #[arg(long)]
    source_tx: String,
    /// Exact non-negative asset quantity.
    #[arg(long, value_name = "QUANTITY")]
    amount: Quantity,
    /// Canonical Iroha asset id.
    #[arg(long)]
    asset_id: String,
    /// Iroha account id or external address payload.
    #[arg(long)]
    recipient: String,
    /// Optional destination transaction hash (hex, 32 bytes).
    #[arg(long)]
    dest_tx: Option<String>,
    /// Proof hash (hex, 32 bytes).
    #[arg(long)]
    proof_hash: Option<String>,
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::GenesisReadiness(args) => genesis_readiness::run(context, args),
            Self::EmitReceipt(args) => emit_receipt(context, args),
        }
    }
}
fn hex32(value: &str) -> Result<[u8; 32]> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.len() != 64 {
        return Err(eyre::eyre!(
            "expected exactly 32 hexadecimal bytes, got {} characters",
            value.len()
        ));
    }
    let mut out = [0_u8; 32];
    hex::decode_to_slice(value, &mut out)?;
    Ok(out)
}
fn emit_receipt(ctx: &mut impl RunContext, args: EmitReceiptArgs) -> Result<()> {
    let source_tx = hex32(&args.source_tx)?;
    let dest_tx = args.dest_tx.as_deref().map(hex32).transpose()?;
    let proof_hash = args
        .proof_hash
        .as_deref()
        .map(hex32)
        .transpose()?
        .unwrap_or([0; 32]);
    let receipt = BridgeReceipt {
        lane: LaneId::new(args.lane),
        direction: args.direction.into_bytes(),
        source_tx,
        dest_tx,
        proof_hash,
        amount: args.amount,
        asset_id: args.asset_id.into_bytes(),
        recipient: args.recipient.into_bytes(),
    };
    ctx.finish(vec![InstructionBox::from(RecordBridgeReceipt::new(
        receipt,
    ))])
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
    #[test]
    fn bridge_commands_expose_no_retired_sccp_surface() {
        assert!(
            TestCli::try_parse_from(["iroha", "sccp", "capabilities"]).is_err(),
            "the retired `ops bridge sccp` group must not parse"
        );
        let parsed = TestCli::try_parse_from([
            "iroha",
            "emit-receipt",
            "--lane",
            "3",
            "--direction",
            "mint",
            "--source-tx",
            &"11".repeat(32),
            "--amount",
            "5",
            "--asset-id",
            "rose",
            "--recipient",
            "alice",
        ])
        .expect("emit-receipt grammar");
        assert!(matches!(parsed.command, Command::EmitReceipt(_)));
    }
    #[test]
    fn hex32_rejects_ambiguous_or_wrong_width_values() {
        for hostile in [
            "",
            "00",
            &format!(" {}", "11".repeat(32)),
            &format!("0x0X{}", "11".repeat(32)),
            &"gg".repeat(32),
        ] {
            assert!(hex32(hostile).is_err(), "accepted hostile hex: {hostile:?}");
        }
        assert_eq!(
            hex32(&format!("0X{}", "AB".repeat(32))).expect("valid hash"),
            [0xAB; 32]
        );
    }
}
