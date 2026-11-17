//! Explicit-layout transaction codecs backed by the canonical Rust data model.

use iroha_crypto::{Algorithm, Hash, HashOf, PrivateKey};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    transaction::{SignedTransaction, TransactionBuilder, TransactionPayload},
};
use napi::bindgen_prelude::{Buffer, Uint8Array};
use napi_derive::napi;
use norito::{NoritoDeserialize, NoritoSerialize, core::WireLayout, json};

use crate::{EXTERNAL_TRANSACTION_PAYLOAD_MAX_BYTES, codec_to_napi, norito_to_napi};

// New offline archives admit the existing large proof and verifying-key builders.
// Current versioned wire retains its model-owned limits.
const SIGNED_MAX_BYTES: usize = 64 * 1024 * 1024;
const JSON_MAX_BYTES: usize = SIGNED_MAX_BYTES * 4;

/// Explicit framing selected at the native API boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Format {
    /// Bare archive; its caller owns the layout selection.
    Bare,
    /// Headered canonical archive with a matching declared layout.
    Framed,
    /// The governed version-1 submission wire, which always uses current layout.
    Versioned,
}

/// Validated immutable options for one synchronous codec operation.
#[derive(Clone, Copy)]
pub(super) struct Options {
    /// Payload layout selected by the owner.
    pub layout: WireLayout,
    /// Explicit framing; never inferred from payload bytes.
    pub format: Format,
}

fn invalid(message: impl Into<String>) -> napi::Error {
    napi::Error::new(napi::Status::InvalidArg, message.into())
}

/// Parse the exact public layout name, retaining the current default.
pub(super) fn layout(value: Option<&str>) -> napi::Result<WireLayout> {
    match value.unwrap_or("current") {
        "current" => Ok(WireLayout::Compact),
        "dense" => Ok(WireLayout::Dense),
        _ => Err(invalid("layout must be exactly current or dense")),
    }
}

impl Options {
    /// Select a bare or headered unsigned payload archive.
    pub fn payload(layout_name: Option<&str>, format: Option<&str>) -> napi::Result<Self> {
        Self::parse(layout_name, format.unwrap_or("bare"), false)
    }

    /// Select a signed archive, retaining the existing versioned default.
    pub fn signed(layout_name: Option<&str>, format: Option<&str>) -> napi::Result<Self> {
        Self::parse(layout_name, format.unwrap_or("versioned"), true)
    }

    fn parse(layout_name: Option<&str>, format: &str, signed: bool) -> napi::Result<Self> {
        let layout = layout(layout_name)?;
        let format = match format {
            "bare" => Format::Bare,
            "framed" => Format::Framed,
            "versioned" if signed && layout == WireLayout::Compact => Format::Versioned,
            "versioned" if signed => {
                return Err(invalid(
                    "dense has no governed versioned transaction mapping; select bare or framed for an offline archive",
                ));
            }
            _ => {
                return Err(invalid(
                    "format must be bare or framed, or versioned for current signed transactions",
                ));
            }
        };
        Ok(Self { layout, format })
    }
}

fn bounded(bytes: &[u8], maximum: usize, context: &str) -> napi::Result<()> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!(
            "{context} must contain 1..={maximum} bytes"
        )));
    }
    Ok(())
}

fn decode<T>(bytes: &[u8], options: Options, maximum: usize) -> napi::Result<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    bounded(bytes, maximum, "transaction archive")?;
    // Bare decoding resets ambient decode state before selecting its own layout.
    // Keep an outer guard so the synchronous SDK operation restores its caller.
    let _caller_layout = norito::core::DecodeFlagsGuard::enter(options.layout.code());
    match options.format {
        Format::Bare => norito::codec::decode_bare_with_layout(bytes, options.layout),
        Format::Framed => norito::decode_canonical_with_layout(bytes, options.layout),
        Format::Versioned => return Err(invalid("unsigned payloads have no versioned framing")),
    }
    .map_err(norito_to_napi)
}

fn encode<T: NoritoSerialize>(
    value: &T,
    options: Options,
    maximum: usize,
) -> napi::Result<Vec<u8>> {
    let bytes = match options.format {
        Format::Bare => norito::codec::encode_bare_with_layout(value, options.layout),
        Format::Framed => norito::encode_canonical_with_layout(value, options.layout),
        Format::Versioned => return Err(invalid("unsigned payloads have no versioned framing")),
    }
    .map_err(norito_to_napi)?;
    bounded(&bytes, maximum, "transaction archive")?;
    Ok(bytes)
}

/// Decode an ordinary transaction payload and retain the builder's admission rules.
pub(super) fn decode_payload(bytes: &[u8], options: Options) -> napi::Result<TransactionBuilder> {
    let payload =
        decode::<TransactionPayload>(bytes, options, EXTERNAL_TRANSACTION_PAYLOAD_MAX_BYTES)?;
    TransactionBuilder::from_payload(payload).map_err(norito_to_napi)
}

/// Encode an ordinary payload in the explicitly selected archive format.
pub(super) fn encode_payload(
    payload: &TransactionPayload,
    options: Options,
) -> napi::Result<Vec<u8>> {
    encode(payload, options, EXTERNAL_TRANSACTION_PAYLOAD_MAX_BYTES)
}

/// Decode exactly one signed transaction, without authenticating its signatures.
pub(super) fn decode_signed(bytes: &[u8], options: Options) -> napi::Result<SignedTransaction> {
    if options.format == Format::Versioned {
        return crate::decode_canonical_signed_transaction_v1(bytes);
    }
    decode(bytes, options, SIGNED_MAX_BYTES)
}

/// Encode one signed transaction; dense never claims the current submission version.
pub(super) fn encode_signed(value: &SignedTransaction, options: Options) -> napi::Result<Vec<u8>> {
    if options.format == Format::Versioned {
        return crate::encode_canonical_signed_transaction_v1(value);
    }
    encode(value, options, SIGNED_MAX_BYTES)
}

/// Hash the exact typed signing payload in its selected layout.
pub(super) fn payload_hash(
    payload: &TransactionPayload,
    layout: WireLayout,
) -> napi::Result<[u8; Hash::LENGTH]> {
    HashOf::try_new_with_layout(payload, layout)
        .map(|hash| *hash.as_ref())
        .map_err(norito_to_napi)
}

// The model's ExternalEntrypointRef is private. This borrowed projection uses its
// exact canonical enum tag and field serializer; authorization proofs stay out of
// the identity. A dense identity is an offline calculation until governed activation.
struct ExternalIntent<'a>(&'a TransactionPayload);

impl norito::core::SerializePayload for ExternalIntent<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::write_enum_tag(writer, 0)?;
        norito::core::write_len_prefixed(writer, self.0)
    }
}

/// Hash the external intent without changing the model's fixed current-layout cache.
pub(super) fn signed_hash(
    value: &SignedTransaction,
    layout: WireLayout,
) -> napi::Result<[u8; Hash::LENGTH]> {
    HashOf::try_new_with_layout(&ExternalIntent(value.payload()), layout)
        .map(|hash| *hash.as_ref())
        .map_err(norito_to_napi)
}

/// Sign an admitted builder using the data model's explicit-layout signing path.
pub(super) fn sign(
    builder: TransactionBuilder,
    key: &PrivateKey,
    layout: WireLayout,
) -> napi::Result<SignedTransaction> {
    builder
        .try_sign_with_layout(key, layout)
        .map_err(norito_to_napi)
}

/// Package signed bytes and the corresponding external intent hash.
pub(super) fn signed_result(
    value: &SignedTransaction,
    options: Options,
) -> napi::Result<crate::JsSignedTransaction> {
    Ok(crate::JsSignedTransaction {
        signed_transaction: Buffer::from(encode_signed(value, options)?),
        hash: Buffer::from(signed_hash(value, options.layout)?.to_vec()),
    })
}

fn network_guard(network_prefix: f64) -> napi::Result<ChainDiscriminantGuard> {
    let prefix = iroha_js_codec::checked_network_prefix(network_prefix).map_err(codec_to_napi)?;
    Ok(ChainDiscriminantGuard::enter(prefix))
}

/// Encode typed transaction-payload JSON as an explicit bare or framed archive.
#[napi]
pub fn encode_transaction_payload_json(
    payload_json: String,
    network_prefix: f64,
    layout: Option<String>,
    format: Option<String>,
) -> napi::Result<Buffer> {
    let options = Options::payload(layout.as_deref(), format.as_deref())?;
    bounded(payload_json.as_bytes(), JSON_MAX_BYTES, "transaction JSON")?;
    let _network = network_guard(network_prefix)?;
    let payload = json::from_str(&payload_json).map_err(norito_to_napi)?;
    let builder = TransactionBuilder::from_payload(payload).map_err(norito_to_napi)?;
    Ok(Buffer::from(encode_payload(builder.payload(), options)?))
}

/// Encode typed signed-transaction JSON; this does not authenticate its signatures.
#[napi]
pub fn encode_signed_transaction_json(
    transaction_json: String,
    network_prefix: f64,
    layout: Option<String>,
    format: Option<String>,
) -> napi::Result<Buffer> {
    let options = Options::signed(layout.as_deref(), format.as_deref())?;
    bounded(
        transaction_json.as_bytes(),
        JSON_MAX_BYTES,
        "signed transaction JSON",
    )?;
    let _network = network_guard(network_prefix)?;
    let value = json::from_str(&transaction_json).map_err(norito_to_napi)?;
    Ok(Buffer::from(encode_signed(&value, options)?))
}

/// Sign one exact ordinary payload archive after binding the caller's expected network.
///
/// Bare/framed input and output use the same explicit selection. This API does
/// not select a network submission wire; private key material is runtime-only.
#[napi]
pub fn sign_transaction_payload(
    network_id: Uint8Array,
    payload_bytes: Uint8Array,
    secret: Uint8Array,
    network_prefix: f64,
    layout: Option<String>,
    format: Option<String>,
) -> napi::Result<Buffer> {
    let options = Options::payload(layout.as_deref(), format.as_deref())?;
    let _network = network_guard(network_prefix)?;
    let expected_network = crate::parse_transaction_network_id_bytes(network_id.as_ref())?;
    let builder = decode_payload(payload_bytes.as_ref(), options)?;
    crate::require_expected_transaction_network_id(
        builder.payload().network_id(),
        &expected_network,
        "JavaScript payload signer",
    )?;
    let key =
        PrivateKey::from_bytes(Algorithm::Ed25519, secret.as_ref()).map_err(norito_to_napi)?;
    let signed = sign(builder, &key, options.layout)?;
    Ok(Buffer::from(encode_signed(&signed, options)?))
}

#[cfg(test)]
mod tests;
