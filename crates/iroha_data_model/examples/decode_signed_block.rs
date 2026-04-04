//! Inspect a Norito-framed `SignedBlock` (e.g., encoded genesis) for debugging.

use std::{collections::BTreeSet, convert::TryFrom, env, error::Error, fs};

use iroha_crypto::{KeyPair, SignatureOf};
use iroha_data_model::{
    block::{
        BlockHeader, BlockSignature, decode_framed_signed_block,
        deframe_versioned_signed_block_bytes,
    },
    prelude::SetParameter,
    transaction::executable::Executable,
};
use nonzero_ext::nonzero;

fn read_varint(bytes: &[u8], mut pos: usize) -> Result<(u64, usize), Box<dyn Error>> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *bytes
            .get(pos)
            .ok_or_else(|| format!("varint: unexpected EOF at offset {pos}"))?;
        value |= u64::from(byte & 0x7F) << shift;
        pos += 1;
        if (byte & 0x80) == 0 {
            return Ok((value, pos));
        }
        shift += 7;
        if shift >= 64 {
            return Err("varint too large".into());
        }
    }
}

fn extract_first_btreeset_element(payload: &[u8]) -> Result<&[u8], Box<dyn Error>> {
    let (len, mut pos) = read_varint(payload, 0)?;
    if len == 0 {
        return Err("empty set".into());
    }
    let offset_slots = usize::try_from(len + 1).map_err(|_| "offset count overflow")?;
    let mut offsets = Vec::with_capacity(offset_slots);
    for _ in 0..=len {
        let (offset, next) = read_varint(payload, pos)?;
        let offset = usize::try_from(offset).map_err(|_| "offset too large")?;
        offsets.push(offset);
        pos = next;
    }
    let total = *offsets.last().ok_or("missing offsets")?;
    let data_region = payload
        .get(pos..pos + total)
        .ok_or("data region truncated")?;
    let start = offsets[0];
    let end = offsets[1];
    Ok(data_region
        .get(start..end)
        .ok_or("element span out of range")?)
}

fn dump_reference_encoding() {
    let kp = KeyPair::random();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let sig = SignatureOf::<BlockHeader>::from_hash(kp.private_key(), header.hash());
    let block_sig = BlockSignature::new(0, sig);
    let mut set = BTreeSet::new();
    set.insert(block_sig);
    let mut buf = Vec::new();
    norito::core::NoritoSerialize::serialize(&set, &mut buf).unwrap();
    println!(
        "reference BTreeSet bytes len={} head={}",
        buf.len(),
        buf.iter()
            .take(32)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn Error>> {
    dump_reference_encoding();

    let path = env::args()
        .nth(1)
        .ok_or("usage: decode_signed_block <path>")?;
    let bytes = fs::read(&path)?;
    println!("file: {path}");
    println!("total bytes: {}", bytes.len());

    let deframed = deframe_versioned_signed_block_bytes(&bytes)?;
    let header_index = 1 + norito::core::Header::SIZE - 1;
    let header_flags = bytes
        .get(header_index)
        .copied()
        .unwrap_or(norito::core::default_encode_flags());
    println!("framed header flags: 0x{header_flags:02x}");
    println!("bare versioned len: {}", deframed.bare_versioned.len());

    let versioned = deframed.bare_versioned.as_ref();
    if versioned.is_empty() {
        return Err("versioned payload empty".into());
    }
    let payload = &versioned[1..];
    println!("payload len: {}", payload.len());

    println!(
        "payload head: {}",
        payload
            .iter()
            .take(32)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    norito::core::reset_decode_state();
    match norito::core::decode_field_canonical::<BTreeSet<BlockSignature>>(payload) {
        Ok((signatures, used)) => {
            println!(
                "decoded signatures set: {} entries (bytes consumed: {used})",
                signatures.len()
            );
        }
        Err(err) => {
            println!("failed to decode signatures set: {err}");
        }
    }
    match extract_first_btreeset_element(payload) {
        Ok(element) => {
            println!(
                "element bytes len={} head={}",
                element.len(),
                element
                    .iter()
                    .take(32)
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            match std::panic::catch_unwind(|| {
                norito::core::decode_field_canonical::<BlockSignature>(element)
            }) {
                Ok(Ok((sig, used))) => {
                    println!(
                        "decoded BlockSignature (used {used} bytes): index={} payload_len={}",
                        sig.index(),
                        sig.signature().payload().len()
                    );
                }
                Ok(Err(err)) => println!("decode BlockSignature error: {err}"),
                Err(_) => println!("decode BlockSignature panicked"),
            }
        }
        Err(err) => {
            println!("failed to extract element: {err}");
        }
    }

    match std::panic::catch_unwind(|| decode_framed_signed_block(&bytes)) {
        Ok(Ok(block)) => {
            let header = block.header();
            println!(
                "block header: height={} prev={:?} txs={}",
                header.height(),
                header.prev_block_hash(),
                block.transactions_vec().len()
            );
            println!("block has results: {}", block.has_results());
            println!("block hash: {:?}", block.hash());
            println!("block signatures via API: {}", block.signatures().count());
            println!(
                "transaction signature lens: {:?}",
                block
                    .transactions_vec()
                    .iter()
                    .map(|tx| {
                        let mut buf = Vec::new();
                        norito::core::NoritoSerialize::serialize(tx.signature(), &mut buf)
                            .map(|()| buf.len())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
            );
            for (idx, tx) in block.transactions_vec().iter().enumerate() {
                println!("tx[{idx}] authority={}", tx.authority());
                match tx.instructions() {
                    Executable::Instructions(instrs) => {
                        for (j, instr) in instrs.iter().enumerate() {
                            println!("  instr[{j}]: {instr:?}");
                            if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>() {
                                println!("    set parameter payload: {:?}", set_param.0);
                            }
                        }
                    }
                    Executable::ContractCall(call) => {
                        println!(
                            "  contract call: address={} entrypoint={} payload={:?}",
                            call.contract_address, call.entrypoint, call.payload
                        );
                    }
                    Executable::Ivm(bytecode) => {
                        println!("  ivm bytecode: {bytecode:?}");
                    }
                    Executable::IvmProved(proved) => {
                        println!(
                            "  ivm proved: bytecode {} bytes overlay {} instructions",
                            proved.bytecode.size_bytes(),
                            proved.overlay.len()
                        );
                    }
                }
                if let Some(err) = block.error(idx) {
                    println!("  tx[{idx}] execution error: {err:?}");
                } else if block.has_results() {
                    println!("  tx[{idx}] execution result: Ok (details not printed)");
                } else {
                    println!("  tx[{idx}] execution result: not attached");
                }
            }
        }
        Ok(Err(err)) => {
            println!("decode_framed_signed_block error: {err}");
        }
        Err(_) => {
            println!("decode_framed_signed_block panicked");
        }
    }

    Ok(())
}
