//! Decode an offline transfer bundle from Norito/base64 into canonical JSON.

use std::{fs, path::PathBuf};

use base64::Engine as _;
use clap::Parser;
use eyre::{Result, WrapErr, bail, eyre};
use iroha::data_model::{
    asset::AssetId,
    offline::{
        AggregateProofEnvelope, OfflineBalanceProof, OfflineCertificateBalanceProof,
        OfflinePlatformProof, OfflinePlatformTokenSnapshot, OfflineSpendReceipt,
        OfflineToOnlineTransfer, OfflineWalletCertificate,
    },
    prelude::AccountId,
    proof::ProofAttachmentList,
};
use iroha_crypto::{Hash, Signature};
use iroha_primitives::numeric::Numeric;
use norito::codec::{Decode, Encode};

#[derive(Parser, Debug)]
#[command(about = "Decode OfflineToOnlineTransfer Norito payload into canonical JSON")]
struct Args {
    /// Input file containing raw Norito bytes OR text payload (`norito:<b64>` or raw base64).
    #[arg(long)]
    input: PathBuf,
    /// Output JSON file path.
    #[arg(long)]
    output: PathBuf,
}

fn parse_input(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.starts_with(b"NRT0") {
        return Ok(bytes.to_vec());
    }

    let text = std::str::from_utf8(bytes)
        .wrap_err("input is not valid UTF-8 text and not raw Norito bytes")?
        .trim();
    let payload = text.strip_prefix("norito:").unwrap_or(text);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .wrap_err("failed to base64 decode payload")?;
    if !decoded.starts_with(b"NRT0") {
        bail!("decoded payload is not a Norito frame (missing NRT0 header)");
    }
    Ok(decoded)
}

fn read_u64_le(bytes: &[u8], offset: usize, label: &str) -> Result<u64> {
    if offset + 8 > bytes.len() {
        bail!("{label}: truncated u64 at offset {offset}");
    }
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(&bytes[offset..offset + 8]);
    Ok(u64::from_le_bytes(raw))
}

fn parse_frame_payload(frame: &[u8]) -> Result<&[u8]> {
    if frame.len() < 40 {
        bail!("Norito frame is too short: {} bytes", frame.len());
    }
    if !frame.starts_with(b"NRT0") {
        bail!("Norito frame magic mismatch");
    }
    let payload_len = read_u64_le(frame, 23, "norito.payload_len")?;
    let payload_len: usize =
        usize::try_from(payload_len).map_err(|_| eyre!("payload length does not fit usize"))?;
    if payload_len > frame.len() {
        bail!(
            "Norito payload length {} exceeds frame length {}",
            payload_len,
            frame.len()
        );
    }
    let payload_start = frame.len() - payload_len;
    if payload_start < 40 {
        bail!("Norito payload starts inside the header");
    }
    Ok(&frame[payload_start..])
}

fn split_fields(payload: &[u8], label: &str) -> Result<Vec<Vec<u8>>> {
    let mut fields = Vec::new();
    let mut offset = 0_usize;
    while offset < payload.len() {
        let len = read_u64_le(payload, offset, label)?;
        offset += 8;
        let len: usize =
            usize::try_from(len).map_err(|_| eyre!("{label}: field length overflow"))?;
        if offset + len > payload.len() {
            bail!(
                "{label}: field length {} at offset {} exceeds payload size {}",
                len,
                offset - 8,
                payload.len()
            );
        }
        fields.push(payload[offset..offset + len].to_vec());
        offset += len;
    }
    if offset != payload.len() {
        bail!("{label}: trailing bytes remain after field split");
    }
    Ok(fields)
}

fn decode_bare<T: Decode>(bytes: &[u8], label: &str) -> Result<T> {
    let mut cursor = bytes;
    let value = T::decode(&mut cursor).map_err(|err| eyre!("{label}: decode error: {err}"))?;
    if !cursor.is_empty() {
        bail!("{label}: decode left {} trailing bytes", cursor.len());
    }
    Ok(value)
}

fn decode_wire_receipt(payload: &[u8]) -> Result<OfflineSpendReceipt> {
    let fields = split_fields(payload, "receipt")?;
    if fields.len() != 10 && fields.len() != 11 {
        bail!("receipt: expected 10 or 11 fields, got {}", fields.len());
    }

    let tx_id: Hash = decode_bare(&fields[0], "receipt.tx_id")?;
    let from: AccountId = decode_bare(&fields[1], "receipt.from")?;
    let to: AccountId = decode_bare(&fields[2], "receipt.to")?;
    let asset: AssetId = decode_bare(&fields[3], "receipt.asset")?;
    let amount: Numeric = decode_bare(&fields[4], "receipt.amount")?;
    let issued_at_ms: u64 = decode_bare(&fields[5], "receipt.issued_at_ms")?;
    let invoice_id: String = decode_bare(&fields[6], "receipt.invoice_id")?;
    let platform_proof: OfflinePlatformProof = decode_bare(&fields[7], "receipt.platform_proof")?;

    let (platform_snapshot, certificate_field, signature_field) = if fields.len() == 11 {
        (
            decode_bare::<Option<OfflinePlatformTokenSnapshot>>(
                &fields[8],
                "receipt.platform_snapshot",
            )?,
            &fields[9],
            &fields[10],
        )
    } else {
        (None, &fields[8], &fields[9])
    };

    let certificate: OfflineWalletCertificate =
        decode_bare(certificate_field, "receipt.sender_certificate")?;
    let sender_signature: Signature = decode_bare(signature_field, "receipt.sender_signature")?;
    let sender_certificate_id = certificate.certificate_id();

    Ok(OfflineSpendReceipt {
        tx_id,
        from,
        to,
        asset,
        amount,
        issued_at_ms,
        invoice_id,
        platform_proof,
        platform_snapshot,
        sender_certificate_id,
        sender_signature,
    })
}

fn decode_wire_receipts(payload: &[u8]) -> Result<Vec<OfflineSpendReceipt>> {
    if payload.len() < 8 {
        bail!("transfer.receipts: missing vector length");
    }
    let count = read_u64_le(payload, 0, "transfer.receipts.count")?;
    let count: usize =
        usize::try_from(count).map_err(|_| eyre!("transfer.receipts.count overflow"))?;
    let mut receipts = Vec::with_capacity(count);
    let mut offset = 8_usize;
    for index in 0..count {
        let len = read_u64_le(payload, offset, "transfer.receipts.item_len")?;
        offset += 8;
        let len: usize = usize::try_from(len)
            .map_err(|_| eyre!("transfer.receipts[{index}] length overflow"))?;
        if offset + len > payload.len() {
            bail!(
                "transfer.receipts[{index}]: length {} exceeds payload size {}",
                len,
                payload.len()
            );
        }
        let receipt = decode_wire_receipt(&payload[offset..offset + len])?;
        receipts.push(receipt);
        offset += len;
    }
    if offset != payload.len() {
        bail!(
            "transfer.receipts: {} trailing bytes remain after decoding",
            payload.len() - offset
        );
    }
    Ok(receipts)
}

fn decode_wire_transfer(payload: &[u8]) -> Result<OfflineToOnlineTransfer> {
    let fields = split_fields(payload, "transfer")?;
    if fields.len() != 8 && fields.len() != 9 {
        bail!("transfer: expected 8 or 9 fields, got {}", fields.len());
    }

    let bundle_id: Hash = decode_bare(&fields[0], "transfer.bundle_id")?;
    let receiver: AccountId = decode_bare(&fields[1], "transfer.receiver")?;
    let deposit_account: AccountId = decode_bare(&fields[2], "transfer.deposit_account")?;
    let receipts = decode_wire_receipts(&fields[3])?;
    let balance_proof: OfflineBalanceProof = decode_bare(&fields[4], "transfer.balance_proof")?;

    let (balance_proofs, aggregate_proof, attachments, platform_snapshot) = if fields.len() == 9 {
        (
            decode_bare::<Option<Vec<OfflineCertificateBalanceProof>>>(
                &fields[5],
                "transfer.balance_proofs",
            )?,
            decode_bare::<Option<AggregateProofEnvelope>>(&fields[6], "transfer.aggregate_proof")?,
            decode_bare::<Option<ProofAttachmentList>>(&fields[7], "transfer.attachments")?,
            decode_bare::<Option<OfflinePlatformTokenSnapshot>>(
                &fields[8],
                "transfer.platform_snapshot",
            )?,
        )
    } else {
        (
            None,
            decode_bare::<Option<AggregateProofEnvelope>>(&fields[5], "transfer.aggregate_proof")?,
            decode_bare::<Option<ProofAttachmentList>>(&fields[6], "transfer.attachments")?,
            decode_bare::<Option<OfflinePlatformTokenSnapshot>>(
                &fields[7],
                "transfer.platform_snapshot",
            )?,
        )
    };

    Ok(OfflineToOnlineTransfer {
        bundle_id,
        receiver,
        deposit_account,
        receipts,
        balance_proof,
        balance_proofs,
        aggregate_proof,
        attachments,
        platform_snapshot,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    let raw = fs::read(&args.input)
        .wrap_err_with(|| format!("failed to read {}", args.input.display()))?;
    let frame = parse_input(&raw)?;
    let payload = parse_frame_payload(&frame)?;
    let transfer =
        decode_wire_transfer(payload).wrap_err("failed to decode wire bundle payload")?;

    let _encoded_bundle_id_payload: Vec<u8> = transfer.bundle_id.encode();

    let json =
        norito::json::to_json_pretty(&transfer).wrap_err("failed to encode transfer as JSON")?;
    fs::write(&args.output, format!("{}\n", json))
        .wrap_err_with(|| format!("failed to write {}", args.output.display()))?;

    println!("decoded_bundle_id={}", transfer.bundle_id);
    println!("output={}", args.output.display());
    Ok(())
}
