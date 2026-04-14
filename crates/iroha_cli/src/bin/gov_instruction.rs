//! Encode governance instructions into `iroha_cli tx stdin` JSON payloads.

use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    data_model::isi::{
        InstructionBox, bridge::RecordSccpMessage, decode_instruction_from_pair,
        governance::RegisterCitizen,
    },
};
use iroha_sccp::{SccpPayloadV1, TransferPayloadV1, canonical_sccp_payload_bytes, sccp_message_id};

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
        amount: u128,
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
    /// Encode a `RecordSccpMessage` instruction from an SCCP transfer payload.
    RecordSccpTransfer {
        #[arg(long)]
        source_domain: u32,
        #[arg(long)]
        dest_domain: u32,
        #[arg(long)]
        nonce: u64,
        #[arg(long)]
        asset_home_domain: u32,
        #[arg(long)]
        asset_id_codec: u8,
        #[arg(long)]
        asset_id: String,
        #[arg(long)]
        amount: u128,
        #[arg(long)]
        sender_codec: u8,
        #[arg(long)]
        sender: String,
        #[arg(long)]
        recipient_codec: u8,
        #[arg(long)]
        recipient: String,
        #[arg(long)]
        route_id_codec: u8,
        #[arg(long)]
        route_id: String,
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
                .address
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
        Command::RecordSccpTransfer {
            source_domain,
            dest_domain,
            nonce,
            asset_home_domain,
            asset_id_codec,
            asset_id,
            amount,
            sender_codec,
            sender,
            recipient_codec,
            recipient,
            route_id_codec,
            route_id,
        } => {
            let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
                version: 1,
                source_domain,
                dest_domain,
                nonce,
                asset_home_domain,
                asset_id_codec,
                asset_id: asset_id.into_bytes(),
                amount,
                sender_codec,
                sender: sender.into_bytes(),
                recipient_codec,
                recipient: recipient.into_bytes(),
                route_id_codec,
                route_id: route_id.into_bytes(),
            });
            let message_id = hex::encode(sccp_message_id(&payload));
            eprintln!("message_id={message_id}");
            let payload_bytes = canonical_sccp_payload_bytes(&payload);
            let instruction = InstructionBox::from(RecordSccpMessage::new(payload_bytes));
            let bytes = norito::to_bytes(&instruction).wrap_err("failed to encode instruction")?;
            print_tx_stdin_json(&bytes);
        }
    }
    Ok(())
}
