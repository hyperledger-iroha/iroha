//! Explicit Norito layout selection for transactions and instruction envelopes.
//!
//! Every entry point takes the layout (and, for transaction archives, the framing) as an explicit
//! argument; nothing infers a layout from payload bytes. The current layout keeps its existing
//! bytes, versioned submission wire and hashes. The dense layout (`norito.md`, "Dense layout")
//! is available for offline archives and instruction envelopes only.
//!
//! TODO(dense-activation): no ledger definition selects the dense layout yet, so no versioned
//! submission wire carries it and no dense transaction identity is defined
//! (`specs/live_runtime_upgrades.md`).

use std::slice;

use iroha_crypto::PrivateKey;
use iroha_data_model::{
    NetworkId,
    isi::{
        InstructionBox, decode_instruction_with_layout, instruction_payload_with_layout,
        instruction_wire_id, registry::is_instruction_wire_id_registered,
    },
    parameter::system::TransactionParameters,
    transaction::{SignedTransaction, TransactionBuilder, TransactionPayload},
};
use iroha_version::codec::EncodeVersioned as _;
use libc::{c_char, c_int, c_uchar, c_ulong};
use norito::{NoritoDeserialize, NoritoSerialize, core::WireLayout};

use crate::{
    BridgeError, BridgeResult, JsonValue, bridge_result_to_code, clear_bridge_output_or_null,
    parse_algorithm_code, parse_private_key_with_algorithm, read_network_id_bridge,
    read_string_bridge, signed_transaction_bridge_debug_json, write_bytes_bridge,
};

/// Layout selector for the current layout: minor 0 with compact lengths.
pub(crate) const LAYOUT_CURRENT: c_uchar = 0;
/// Layout selector for the dense layout: header minor 1.
pub(crate) const LAYOUT_DENSE: c_uchar = 1;
/// Archive selector for the governed version-1 submission wire (current layout only).
pub(crate) const FORMAT_VERSIONED: c_uchar = 0;
/// Archive selector for a headerless payload whose owner names its layout.
pub(crate) const FORMAT_BARE: c_uchar = 1;
/// Archive selector for a Norito frame whose header must declare the selected layout.
pub(crate) const FORMAT_FRAMED: c_uchar = 2;

/// Longest instruction envelope payload the transcoder admits (the network's transaction bound).
pub(crate) fn transaction_max_bytes() -> usize {
    usize::try_from(TransactionParameters::default().max_tx_bytes().get()).unwrap_or(usize::MAX)
}

/// Parse an exact layout selector; the fixed-width minor-0 layout is never selectable.
pub(crate) fn layout(code: c_uchar) -> BridgeResult<WireLayout> {
    match code {
        LAYOUT_CURRENT => Ok(WireLayout::Compact),
        LAYOUT_DENSE => Ok(WireLayout::Dense),
        _ => Err(BridgeError::WireLayout),
    }
}

/// Framing of one transaction archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    /// The governed version-1 submission wire.
    Versioned,
    /// A headerless payload in the selected layout.
    Bare,
    /// A Norito frame whose header declares the selected layout.
    Framed,
}

/// An explicit layout and framing for one transaction archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) layout: WireLayout,
    pub(crate) format: Format,
}

impl Options {
    /// Select a signed-transaction archive. Only the current layout has a versioned wire.
    pub(crate) fn signed(layout_code: c_uchar, format_code: c_uchar) -> BridgeResult<Self> {
        let layout = layout(layout_code)?;
        let format = match format_code {
            FORMAT_VERSIONED if layout == WireLayout::Compact => Format::Versioned,
            FORMAT_BARE => Format::Bare,
            FORMAT_FRAMED => Format::Framed,
            _ => return Err(BridgeError::WireLayout),
        };
        Ok(Self { layout, format })
    }

    /// Select an unsigned transaction-payload archive, which is bare or framed.
    pub(crate) fn payload(layout_code: c_uchar, format_code: c_uchar) -> BridgeResult<Self> {
        let options = Self::signed(layout_code, format_code)?;
        if options.format == Format::Versioned {
            return Err(BridgeError::WireLayout);
        }
        Ok(options)
    }
}

fn bounded(bytes: &[u8]) -> BridgeResult<()> {
    if bytes.is_empty() || bytes.len() > transaction_max_bytes() {
        return Err(BridgeError::TransactionArchive);
    }
    Ok(())
}

fn encode_archive<T: NoritoSerialize>(value: &T, options: Options) -> BridgeResult<Vec<u8>> {
    let bytes = match options.format {
        Format::Bare => norito::codec::encode_bare_with_layout(value, options.layout),
        Format::Framed => norito::encode_canonical_with_layout(value, options.layout),
        Format::Versioned => return Err(BridgeError::WireLayout),
    }
    .map_err(|_| BridgeError::TransactionArchive)?;
    bounded(&bytes)?;
    Ok(bytes)
}

/// Decode one exact archive and require its canonical re-encoding to reproduce it.
fn decode_archive<T>(bytes: &[u8], options: Options) -> BridgeResult<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    bounded(bytes)?;
    // Bare decoding resets ambient decode state before selecting its own layout; the outer
    // guard restores the caller's selection afterwards.
    let _caller = norito::core::DecodeFlagsGuard::enter(norito::core::get_decode_flags());
    let value: T = match options.format {
        Format::Bare => norito::codec::decode_bare_with_layout(bytes, options.layout),
        Format::Framed => norito::decode_canonical_with_layout(bytes, options.layout),
        Format::Versioned => return Err(BridgeError::WireLayout),
    }
    .map_err(|_| BridgeError::TransactionArchive)?;
    if encode_archive(&value, options)? != bytes {
        return Err(BridgeError::TransactionArchive);
    }
    Ok(value)
}

/// Decode exactly one signed transaction in an explicit archive selection.
///
/// This does not authenticate the transaction's signatures.
pub(crate) fn decode_signed(bytes: &[u8], options: Options) -> BridgeResult<SignedTransaction> {
    if options.format == Format::Versioned {
        bounded(bytes)?;
        return crate::decode_signed_transaction(bytes)
            .map_err(|_| BridgeError::TransactionArchive);
    }
    decode_archive(bytes, options)
}

/// Encode one signed transaction; the dense layout never claims the versioned submission wire.
pub(crate) fn encode_signed(value: &SignedTransaction, options: Options) -> BridgeResult<Vec<u8>> {
    if options.format == Format::Versioned {
        return Ok(value.encode_versioned());
    }
    encode_archive(value, options)
}

/// Decode one ordinary transaction payload and keep the builder's admission rules.
pub(crate) fn decode_payload(bytes: &[u8], options: Options) -> BridgeResult<TransactionBuilder> {
    let payload: TransactionPayload = decode_archive(bytes, options)?;
    TransactionBuilder::from_payload(payload).map_err(|_| BridgeError::TransactionArchive)
}

/// Sign an admitted builder through the data model's explicit-layout signing path.
///
/// The signature covers the payload's typed hash in `layout`; the current layout signs exactly
/// what [`TransactionBuilder::try_sign`] signs.
pub(crate) fn sign_builder(
    builder: TransactionBuilder,
    private_key: &PrivateKey,
    layout: WireLayout,
) -> BridgeResult<SignedTransaction> {
    builder
        .try_sign_with_layout(private_key, layout)
        .map_err(|_| BridgeError::TransactionSign)
}

/// Sign one exact payload archive after binding the caller's expected network.
pub(crate) fn sign_payload(
    expected_network: &NetworkId,
    payload_bytes: &[u8],
    private_key: &PrivateKey,
    options: Options,
) -> BridgeResult<Vec<u8>> {
    let builder = decode_payload(payload_bytes, options)?;
    if builder.payload().network_id() != Some(expected_network) {
        return Err(BridgeError::NetworkId);
    }
    let signed = sign_builder(builder, private_key, options.layout)?;
    encode_signed(&signed, options)
}

/// Transcode one typed instruction envelope payload between layouts, keyed by its wire ID.
///
/// The current layout carries the instruction's Norito frame; the dense layout carries its bare
/// dense payload. The input must be exactly the canonical payload of the registered instruction
/// in `from`. An unregistered wire ID is [`BridgeError::InstructionUnregistered`], which a client
/// keeps as an opaque envelope in the layout it was decoded from.
pub(crate) fn transcode_instruction(
    wire_id: &str,
    payload: &[u8],
    from: WireLayout,
    to: WireLayout,
) -> BridgeResult<Vec<u8>> {
    if !is_instruction_wire_id_registered(wire_id) {
        return Err(BridgeError::InstructionUnregistered);
    }
    // A dense payload of an instruction without fields is empty; a frame never is.
    if payload.len() > transaction_max_bytes() {
        return Err(BridgeError::InstructionPayload);
    }
    let instruction = decode_instruction_with_layout(wire_id, payload, from)
        .map_err(|_| BridgeError::InstructionPayload)?;
    if instruction.as_opaque().is_some() || instruction_wire_id(&instruction) != Some(wire_id) {
        return Err(BridgeError::InstructionPayload);
    }
    let canonical = instruction_payload(&instruction, from)?;
    if canonical != payload {
        return Err(BridgeError::InstructionPayload);
    }
    if from == to {
        return Ok(canonical);
    }
    instruction_payload(&instruction, to)
}

fn instruction_payload(instruction: &InstructionBox, layout: WireLayout) -> BridgeResult<Vec<u8>> {
    instruction_payload_with_layout(instruction, layout)
        .map(|(_, payload)| payload)
        .ok_or(BridgeError::InstructionPayload)
}

/// Render one signed transaction as typed JSON with the bridge's debug projection.
pub(crate) fn signed_transaction_json(tx: &SignedTransaction) -> BridgeResult<Vec<u8>> {
    let mut value = norito::json::value::to_value(tx).map_err(|_| BridgeError::JsonSerialize)?;
    if let JsonValue::Object(root) = &mut value {
        root.insert(
            "bridge_debug".into(),
            signed_transaction_bridge_debug_json(tx),
        );
    }
    norito::json::to_vec(&value).map_err(|_| BridgeError::JsonSerialize)
}

/// Borrow one input buffer; an empty buffer may be passed as a null pointer.
unsafe fn read_bytes<'a>(ptr: *const c_uchar, len: c_ulong) -> BridgeResult<&'a [u8]> {
    let len = usize::try_from(len).map_err(|_| BridgeError::Alloc)?;
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

/// Transcode one instruction envelope payload between explicitly selected layouts.
///
/// `from_layout` and `to_layout` are `0` (current: the instruction's Norito frame) or `1`
/// (dense: its bare dense payload). The payload must be exactly the canonical payload of the
/// instruction registered under `wire_id` in `from_layout`; equal layouts verify and return it.
///
/// Returns `0` on success, `CONNECT_NORITO_ERR_INSTRUCTION_UNREGISTERED` for a wire ID this
/// bridge registers no instruction under, `CONNECT_NORITO_ERR_INSTRUCTION_PAYLOAD` for any other
/// payload and `CONNECT_NORITO_ERR_WIRE_LAYOUT` for an unknown layout selector.
///
/// # Safety
/// Input pointers must be valid for their lengths; the output pair must be writable and the
/// returned buffer is released with `connect_norito_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_transcode_instruction_v1(
    wire_id_ptr: *const c_char,
    wire_id_len: c_ulong,
    payload_ptr: *const c_uchar,
    payload_len: c_ulong,
    from_layout: c_uchar,
    to_layout: c_uchar,
    out_payload_ptr: *mut *mut c_uchar,
    out_payload_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        clear_bridge_output_or_null(out_payload_ptr, out_payload_len)?;
        let from = layout(from_layout)?;
        let to = layout(to_layout)?;
        let wire_id = unsafe { read_string_bridge(wire_id_ptr, wire_id_len) }?;
        let payload = unsafe { read_bytes(payload_ptr, payload_len) }?;
        let transcoded = transcode_instruction(&wire_id, payload, from, to)?;
        unsafe { write_bytes_bridge(out_payload_ptr, out_payload_len, &transcoded) }
    })();
    bridge_result_to_code(result)
}

/// Decode exactly one signed transaction in an explicit layout and framing into typed JSON.
///
/// `layout` is `0` (current) or `1` (dense). `format` is `0` (the versioned submission wire,
/// current layout only), `1` (bare) or `2` (framed, whose header must declare `layout`). The
/// bytes must be canonical. Signatures are not authenticated.
///
/// # Safety
/// Input pointers must be valid for their lengths; the output pair must be writable and the
/// returned buffer is released with `connect_norito_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_signed_transaction_json_v1(
    signed_ptr: *const c_uchar,
    signed_len: c_ulong,
    layout: c_uchar,
    format: c_uchar,
    out_json_ptr: *mut *mut c_uchar,
    out_json_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        clear_bridge_output_or_null(out_json_ptr, out_json_len)?;
        let options = Options::signed(layout, format)?;
        let bytes = unsafe { read_bytes(signed_ptr, signed_len) }?;
        let transaction = decode_signed(bytes, options)?;
        let json = signed_transaction_json(&transaction)?;
        unsafe { write_bytes_bridge(out_json_ptr, out_json_len, &json) }
    })();
    bridge_result_to_code(result)
}

/// Sign one exact transaction-payload archive in an explicit layout after binding its network.
///
/// `layout` is `0` (current) or `1` (dense) and `format` is `1` (bare) or `2` (framed); the
/// signed transaction is returned in the same selection. The signature covers the payload's
/// typed hash in `layout`, so a dense signature verifies only against the dense encoding. No
/// transaction identity is returned: a dense one is defined only by a governed activation.
/// Private key material is runtime-only.
///
/// # Safety
/// Input pointers must be valid for their lengths; the output pair must be writable and the
/// returned buffer is released with `connect_norito_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sign_transaction_payload_v1(
    network_id_ptr: *const c_char,
    network_id_len: c_ulong,
    payload_ptr: *const c_uchar,
    payload_len: c_ulong,
    layout: c_uchar,
    format: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: c_uchar,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        clear_bridge_output_or_null(out_signed_ptr, out_signed_len)?;
        let options = Options::payload(layout, format)?;
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let network_id = unsafe { read_network_id_bridge(network_id_ptr, network_id_len) }?;
        let payload = unsafe { read_bytes(payload_ptr, payload_len) }?;
        let key_bytes = unsafe { read_bytes(private_key_ptr, private_key_len) }?;
        let private_key = parse_private_key_with_algorithm(key_bytes, algorithm)?;
        let signed = sign_payload(&network_id, payload, &private_key, options)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed) }
    })();
    bridge_result_to_code(result)
}

#[cfg(test)]
mod tests;
