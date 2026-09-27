//! Encode governance instructions as tx-stdin JSON.
use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    data_model::isi::{InstructionBox, decode_instruction_from_pair, governance::RegisterCitizen},
};
use iroha_primitives::numeric::Quantity;
#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand, Debug)]
enum Command {
    /// Encode a `RegisterCitizen` instruction.
    RegisterCitizen {
        #[arg(long)]
        owner: String,
        #[arg(long)]
        amount: Quantity,
        #[arg(long, default_value_t = 369)]
        chain_discriminant: u16,
    },
    /// Wrap an app-api `payload_hex` field into tx-stdin JSON.
    WrapPayloadHex {
        #[arg(long)]
        wire_id: String,
        #[arg(long)]
        payload_hex: String,
    },
}
fn print_tx_stdin_json(bytes: &[u8]) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let encoded = STANDARD.encode(bytes);
    println!("[\"{encoded}\"]");
}
fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::RegisterCitizen {
            owner,
            amount,
            chain_discriminant,
        } => {
            let owner = parse_account_address(&owner, Some(chain_discriminant))
                .wrap_err("failed to parse --owner as canonical account address")?
                .to_account_id()
                .map_err(|err| eyre!(err.to_string()))
                .wrap_err("failed to decode --owner into account id")?;
            let instruction = InstructionBox::from(RegisterCitizen { owner, amount });
            let bytes = norito::to_bytes(&instruction).wrap_err("failed to encode instruction")?;
            print_tx_stdin_json(&bytes);
        }
        Command::WrapPayloadHex {
            wire_id,
            payload_hex,
        } => {
            let bytes = hex::decode(payload_hex.trim())
                .wrap_err("failed to decode --payload-hex as lowercase hex")?;
            let instruction = decode_instruction_from_pair(&wire_id, &bytes)
                .wrap_err("failed to decode instruction from --wire-id and --payload-hex")?;
            let encoded = norito::to_bytes(&instruction)
                .wrap_err("failed to encode reconstructed instruction")?;
            print_tx_stdin_json(&encoded);
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retired_sccp_commands_are_not_parseable() {
        // SCCP v1 governance goes through `iroha sccp governance propose` (specs/sccp.md §10).
        for command in [
            "build-sccp-transfer-ivm-derive-request",
            "publish-sccp-route-manifest",
            "ensure-ivm-execution-vk",
            "propose-sccp-route-governance",
        ] {
            assert!(
                Args::try_parse_from(["gov_instruction", command]).is_err(),
                "retired SCCP helper `{command}` must not remain in the CLI grammar"
            );
        }
    }
    #[test]
    fn register_citizen_and_wrap_payload_hex_parse() {
        let parsed = Args::try_parse_from([
            "gov_instruction",
            "wrap-payload-hex",
            "--wire-id",
            "iroha::RegisterCitizen",
            "--payload-hex",
            "00",
        ])
        .expect("wrap-payload-hex grammar");
        assert!(matches!(parsed.command, Command::WrapPayloadHex { .. }));
        let parsed = Args::try_parse_from([
            "gov_instruction",
            "register-citizen",
            "--owner",
            "owner",
            "--amount",
            "10",
        ])
        .expect("register-citizen grammar");
        assert!(matches!(
            parsed.command,
            Command::RegisterCitizen {
                chain_discriminant: 369,
                ..
            }
        ));
    }
}
