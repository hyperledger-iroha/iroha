//! Diagnostic tool for inspecting Norito-encoded genesis and block payloads.
//!
//! The binary prints a compact summary of the Norito header, verifies the CRC,
//! and exercises multiple decode paths (versioned, manual, and transaction-only)
//! while handling panics so that malformed blocks can be analysed without
//! crashing the host process.
#![allow(clippy::collapsible_if, clippy::collapsible_match)]

use std::{
    any::Any,
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};

use eyre::{Result, eyre};
use iroha_data_model::{
    block::SignedBlock,
    isi::SetParameter,
    parameter::{Parameter, Parameters, system::SumeragiNposParameters},
    transaction::Executable,
};
use iroha_version::codec::DecodeVersioned;
use norito::{
    codec::Decode,
    core::{self, Header, LittleEndian, ReadBytesExt, header_flags},
    hardware_crc64,
};

fn main() -> Result<()> {
    iroha_genesis::init_instruction_registry();
    let mut args = std::env::args().skip(1);
    let path: PathBuf = args
        .next()
        .ok_or_else(|| eyre!("usage: genesis_probe <block-file>"))?
        .into();

    let bytes = fs::read(&path)?;
    println!(
        "event=probe stage=start path={} size={}B",
        path.display(),
        bytes.len()
    );

    let (version, header, bare_payload) = peel_version_and_header(&bytes)?;
    let flags_desc = describe_flags(header.flags);
    println!(
        "event=probe stage=header version={version:#x} magic={magic:?} major={major} minor={minor} schema={schema:?} compression={compression} length={length} checksum={checksum:#x} flags={flags:#x} flags_desc={flags_desc} payload_len={payload_len}",
        magic = header.magic,
        major = header.major,
        minor = header.minor,
        schema = header.schema,
        compression = header.compression_name(),
        length = header.length,
        checksum = header.checksum,
        flags = header.flags,
        flags_desc = flags_desc,
        payload_len = bare_payload.len(),
    );

    let crc = hardware_crc64(bare_payload);
    let checksum_match = header.checksum == crc;
    println!(
        "event=probe stage=crc expected={expected:#x} computed={crc:#x} match={checksum_match}",
        expected = header.checksum,
        crc = crc,
        checksum_match = checksum_match,
    );

    diagnose_signed_block(&bytes);

    diagnose_manual_decode(bare_payload, header.flags, "header-flags");
    diagnose_manual_decode(bare_payload, 0, "flags-0");

    if let Ok(block) = std::panic::catch_unwind(|| {
        core::reset_decode_state();
        core::set_decode_flags(header.flags);
        norito::codec::decode_adaptive::<SignedBlock>(bare_payload)
    }) {
        if let Ok(block) = block {
            dump_parameters(&block);
        }
    }

    if let Err(err) = decode_transactions(bare_payload) {
        println!("event=probe stage=transactions outcome=error err={err:?}");
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct HeaderSummary {
    magic: [u8; 4],
    major: u8,
    minor: u8,
    schema: [u8; 16],
    compression: u8,
    length: u64,
    checksum: u64,
    flags: u8,
}

impl HeaderSummary {
    fn compression_name(&self) -> &'static str {
        match self.compression {
            0 => "none",
            1 => "zstd",
            _ => "unknown",
        }
    }
}

fn peel_version_and_header(bytes: &[u8]) -> Result<(u8, HeaderSummary, &[u8])> {
    if bytes.is_empty() {
        return Err(eyre!("empty payload"));
    }
    let version = bytes[0];
    let payload = bytes
        .get(1..)
        .ok_or_else(|| eyre!("missing Norito payload after version byte"))?;
    if payload.len() < Header::SIZE {
        return Err(eyre!(
            "payload shorter ({}) than Norito header ({})",
            payload.len(),
            Header::SIZE
        ));
    }
    let (header_bytes, rest) = payload.split_at(Header::SIZE);
    let mut cursor = Cursor::new(header_bytes);

    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if magic != core::MAGIC {
        return Err(eyre!("unexpected header magic {magic:?}"));
    }
    let major = cursor.read_u8()?;
    let minor = cursor.read_u8()?;
    let mut schema = [0u8; 16];
    cursor.read_exact(&mut schema)?;
    let compression = cursor.read_u8()?;
    let length = cursor.read_u64::<LittleEndian>()?;
    let checksum = cursor.read_u64::<LittleEndian>()?;
    let flags = cursor.read_u8()?;

    Ok((
        version,
        HeaderSummary {
            magic,
            major,
            minor,
            schema,
            compression,
            length,
            checksum,
            flags,
        },
        rest,
    ))
}

fn describe_flags(flags: u8) -> String {
    let mut parts = Vec::with_capacity(4);
    if flags & header_flags::PACKED_SEQ != 0 {
        parts.push("PACKED_SEQ");
    }
    if flags & header_flags::COMPACT_LEN != 0 {
        parts.push("COMPACT_LEN");
    }
    if flags & header_flags::PACKED_STRUCT != 0 {
        parts.push("PACKED_STRUCT");
    }
    if flags & header_flags::VARINT_OFFSETS != 0 {
        parts.push("VARINT_OFFSETS(reserved)");
    }
    if flags & header_flags::COMPACT_SEQ_LEN != 0 {
        parts.push("COMPACT_SEQ_LEN(reserved)");
    }
    if flags & header_flags::FIELD_BITSET != 0 {
        parts.push("FIELD_BITSET");
    }
    if parts.is_empty() {
        "(none)".to_string()
    } else {
        parts.join("|")
    }
}

fn diagnose_signed_block(bytes: &[u8]) {
    println!("event=probe stage=decode kind=SignedBlock path=versioned start");
    match std::panic::catch_unwind(|| SignedBlock::decode_all_versioned(bytes)) {
        Ok(Ok(block)) => {
            let tx_count = block.external_transactions().len();
            let time_triggers = block.time_triggers().len();
            println!(
                "event=probe stage=decode kind=SignedBlock outcome=ok tx_count={tx_count} time_triggers={time_triggers}"
            );
        }
        Ok(Err(err)) => {
            println!("event=probe stage=decode kind=SignedBlock outcome=error err={err:?}")
        }
        Err(panic) => {
            let msg = describe_panic(panic.as_ref());
            println!("event=probe stage=decode kind=SignedBlock outcome=panic msg={msg}");
        }
    }
}

fn diagnose_manual_decode(payload: &[u8], flags: u8, label: &str) {
    println!("event=probe stage=decode kind=manual label={label} flags={flags:#x} start");
    let outcome = std::panic::catch_unwind(|| manual_decode(payload, flags));
    match outcome {
        Ok(Ok(stats)) => {
            let ManualDecodeStats {
                bytes_consumed,
                signatures,
                transactions,
                results,
            } = stats;
            println!(
                "event=probe stage=decode kind=manual label={label} outcome=ok consumed={bytes_consumed} signatures={signatures} tx={transactions} results={results}"
            );
        }
        Ok(Err(err)) => {
            println!(
                "event=probe stage=decode kind=manual label={label} outcome=error err={err:?}"
            );
        }
        Err(panic) => {
            let msg = describe_panic(panic.as_ref());
            println!("event=probe stage=decode kind=manual label={label} outcome=panic msg={msg}");
        }
    }
}

fn manual_decode(payload: &[u8], flags: u8) -> Result<ManualDecodeStats> {
    core::reset_decode_state();
    core::set_decode_flags(flags);
    let mut cursor = Cursor::new(payload);

    let block: SignedBlock = Decode::decode(&mut cursor)?;

    let signature_count = block.signatures().count();
    let transaction_count = block.transactions_vec().len();
    let result_count = block.results().len();
    let bytes_consumed = usize::try_from(cursor.position())
        .map_err(|_| eyre!("decoded payload length exceeds usize"))?;

    Ok(ManualDecodeStats {
        bytes_consumed,
        signatures: signature_count,
        transactions: transaction_count,
        results: result_count,
    })
}

fn dump_parameters(block: &SignedBlock) {
    let mut params = Parameters::default();
    let mut handshake_entries = Vec::new();
    for tx in block.external_transactions() {
        if let Executable::Instructions(batch) = tx.instructions() {
            for instr in batch {
                if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>() {
                    if let Parameter::Custom(custom) = set_param.inner()
                        && custom.id() == &iroha_data_model::parameter::system::consensus_metadata::handshake_meta_id()
                    {
                        handshake_entries.push(custom.payload().clone());
                    }
                    params.set_parameter(set_param.inner().clone());
                }
            }
        }
    }

    let npos = params
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter);
    let epoch_length = npos
        .as_ref()
        .map_or(0, SumeragiNposParameters::epoch_length_blocks);
    println!(
        "event=params sumeragi={:?} smart_contract={:?} block_max_tx={} epoch_length_blocks={epoch_length}",
        params.sumeragi(),
        params.smart_contract(),
        params.block().max_transactions()
    );
    println!(
        "event=params custom_keys={:?}",
        params.custom().keys().collect::<Vec<_>>()
    );
    for (idx, payload) in handshake_entries.iter().enumerate() {
        println!("event=params handshake[{idx}] raw={payload}");
        match payload.try_into_any::<norito::json::Value>() {
            Ok(value) => println!("event=params handshake[{idx}] json={value:?}"),
            Err(err) => println!("event=params handshake[{idx}] json_decode_error={err}"),
        }
    }
}

fn decode_transactions(payload: &[u8]) -> Result<()> {
    match std::panic::catch_unwind(|| norito::codec::decode_adaptive::<SignedBlock>(payload)) {
        Ok(Ok(block)) => {
            let tx_count = block.transactions_vec().len();
            println!("event=probe stage=transactions outcome=ok tx_count={tx_count}");
            Ok(())
        }
        Ok(Err(err)) => {
            println!("event=probe stage=transactions outcome=error err={err:?}");
            Err(eyre!("failed to decode transactions: {err:?}"))
        }
        Err(panic) => {
            let msg = describe_panic(panic.as_ref());
            println!("event=probe stage=transactions outcome=panic msg={msg}");
            Err(eyre!("transaction decode panicked: {msg}"))
        }
    }
}

fn describe_panic(payload: &(dyn Any + Send + 'static)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<opaque panic>".to_string())
}

struct ManualDecodeStats {
    bytes_consumed: usize,
    signatures: usize,
    transactions: usize,
    results: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_succeeds() {
        let mut bytes = Vec::new();
        bytes.push(0xAA);
        bytes.extend_from_slice(b"NRT0");
        bytes.push(2);
        bytes.push(1);
        bytes.extend_from_slice(&[0x11; 16]);
        bytes.push(0);
        bytes.extend_from_slice(&123u64.to_le_bytes());
        bytes.extend_from_slice(&456u64.to_le_bytes());
        bytes.push(header_flags::PACKED_SEQ | header_flags::COMPACT_LEN);
        bytes.extend_from_slice(&[0xFF; 8]);

        let (version, header, rest) = peel_version_and_header(&bytes).expect("parse");
        assert_eq!(version, 0xAA);
        assert_eq!(header.magic, *b"NRT0");
        assert_eq!(header.major, 2);
        assert_eq!(header.minor, 1);
        assert_eq!(header.schema, [0x11; 16]);
        assert_eq!(header.length, 123);
        assert_eq!(header.checksum, 456);
        assert_eq!(
            header.flags,
            header_flags::PACKED_SEQ | header_flags::COMPACT_LEN
        );
        assert_eq!(rest, &[0xFF; 8]);
    }

    #[test]
    fn describe_flags_formats_bits() {
        let desc = describe_flags(
            header_flags::PACKED_SEQ | header_flags::VARINT_OFFSETS | header_flags::FIELD_BITSET,
        );
        assert!(desc.contains("PACKED_SEQ"));
        assert!(desc.contains("VARINT_OFFSETS(reserved)"));
        assert!(desc.contains("FIELD_BITSET"));
    }
}
