//! Direct committed gas decoding with exact original allocation custody.
//!
//! The caller retains the canonical source and enclosing decoder/diagnostic
//! storage. This leaf neither authenticates a parameter nor publishes State.

use std::alloc::Layout;

use iroha_allocation::{AllocationBudget, AllocationCharge, ChargedBuffer, ChargedBufferError};
use iroha_data_model::account::{AccountLiteralAdmissionError, ChargedAccount};
use iroha_primitives::numeric::{ChargedNumeric, NumericDecimalAdmissionError};
use norito::json::{self, JsonDeserialize};

use super::{
    ACCEPTED_ASSETS, AssetDefinitionId, CommittedGasRate, Gas, GasLiquidity, GasRate,
    GasVolatility, Json, Numeric, Parameters, RATES, TECH_ACCOUNT,
};

/// The original gas-policy error or original allocation refusal.
#[derive(Debug, thiserror::Error)]
pub enum GasAdmissionError {
    /// Unchanged syntax, decoder provenance, address or semantic rejection.
    #[error(transparent)]
    Decode(#[from] norito::Error),
    /// Original finite-pool or physical allocation refusal.
    #[error(transparent)]
    Allocation(#[from] ChargedBufferError),
}
impl From<json::Error> for GasAdmissionError {
    fn from(error: json::Error) -> Self {
        Self::Decode(error.into_core_error())
    }
}

/// Original immutable gas strings, arrays, mantissas and ledger custody.
pub struct GasCustody {
    technical: AllocationCharge,
    assets: TextCustody,
    rates: RateCustody,
}
impl GasCustody {
    /// Test identity of every original pool, including empty arrays.
    #[must_use]
    pub fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.technical.belongs_to(pool)
            && self.assets.belongs_to(pool)
            && self.rates.belongs_to(pool)
    }
}

/// A gas projection retaining the original backing of every dynamic field.
pub struct AdmittedGas {
    value: Gas,
    custody: GasCustody,
}
impl AdmittedGas {
    /// Decode the three committed fields through the original shared parser.
    ///
    /// Missing fields use their protocol defaults. Present malformed fields
    /// reject in technical-account, accepted-assets, rates order. Every raw rate
    /// record is decoded before semantic conversion, preserving error precedence.
    ///
    /// # Errors
    /// Returns original decoder/semantic errors or exact allocation refusals.
    /// Source text and error diagnostics remain the enclosing caller's custody.
    #[allow(unsafe_code)]
    pub fn try_from_parameters(
        parameters: &Parameters,
        pool: &AllocationBudget,
    ) -> Result<Self, GasAdmissionError> {
        // Borrow IDs instead of allocating three lookup keys before admission.
        let payload = |id| {
            parameters
                .custom()
                .values()
                .find(|p| p.id().name().as_ref() == id)
                .map(|p| p.payload())
        };
        let technical = match payload(TECH_ACCOUNT) {
            Some(source) => {
                let text = document(source, |p| Text::decode(p, pool))?;
                ChargedAccount::try_from_encoded(text.as_ref(), None, pool).map_err(|error| {
                    match error {
                        AccountLiteralAdmissionError::Address(error) => {
                            GasAdmissionError::Decode(super::receiver_error(error))
                        }
                        AccountLiteralAdmissionError::Allocation(error) => error.into(),
                    }
                })?;
                text
            }
            None => Text::copy(
                iroha_config::parameters::defaults::pipeline::GAS_TECH_ACCOUNT_ID,
                pool,
            )?,
        };
        let assets = match payload(ACCEPTED_ASSETS) {
            Some(source) => decode_assets(source, pool)?,
            None => TextArray::new(0, pool)?.finish(),
        };
        let rates = match payload(RATES) {
            Some(source) => decode_rates(source, pool)?,
            None => RateArray::new(0, pool)?.finish(),
        };
        // SAFETY: all parsing, allocation and validation is complete. Every
        // original allocation and its charge move into one immutable value-first
        // owner; there are no fallible operations between extraction and pairing.
        let (tech_account_id, technical) = unsafe { technical.into_parts() };
        Ok(Self {
            value: Gas {
                tech_account_id,
                accepted_assets: assets.value,
                units_per_gas: rates.value,
            },
            custody: GasCustody {
                technical,
                assets: assets.custody,
                rates: rates.custody,
            },
        })
    }
    /// Borrow this exact immutable projection.
    #[must_use]
    pub fn get(&self) -> &Gas {
        &self.value
    }
    /// Check original allocation-pool identity for the complete projection.
    #[must_use]
    pub fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.custody.belongs_to(pool)
    }
    /// Transfer exact original backing to an already funded enclosing graph.
    ///
    /// # Safety
    /// Pair both immediately in one immutable owner; never clone, grow or replace
    /// value backing while using this custody. Destroy the value before custody on
    /// all paths and postpone refunds beyond enclosing State/storage guards.
    #[allow(unsafe_code)]
    pub unsafe fn into_allocation_parts(self) -> (Gas, GasCustody) {
        (self.value, self.custody)
    }
}
impl core::fmt::Debug for AdmittedGas {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("AdmittedGas").field(self.get()).finish()
    }
}

struct Text(ChargedBuffer<u8>);
impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        core::str::from_utf8(self.0.as_slice()).expect("shared JSON string parser yields UTF-8")
    }
}
impl AsMut<[u8]> for Text {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}
impl Text {
    fn new(length: usize, pool: &AllocationBudget) -> Result<Self, GasAdmissionError> {
        let mut value = ChargedBuffer::new(length, pool)?;
        for _ in 0..length {
            value.push_reserved(0);
        }
        Ok(Self(value))
    }
    fn copy(source: &str, pool: &AllocationBudget) -> Result<Self, GasAdmissionError> {
        let mut value = Self::new(source.len(), pool)?;
        value.as_mut().copy_from_slice(source.as_bytes());
        Ok(value)
    }
    fn decode(
        p: &mut json::Parser<'_>,
        pool: &AllocationBudget,
    ) -> Result<Self, GasAdmissionError> {
        p.parse_string_with_buffer(|length| Self::new(length, pool))
    }
    #[allow(unsafe_code)]
    unsafe fn into_parts(self) -> (String, AllocationCharge) {
        // SAFETY: the parser/copy constructor establishes UTF-8. String adopts
        // the exact byte allocation without any growth or normalization.
        let (bytes, charge) = unsafe { self.0.into_allocation_parts() };
        (unsafe { String::from_utf8_unchecked(bytes) }, charge)
    }
}

fn document<T>(
    source: &Json,
    decode: impl FnOnce(&mut json::Parser<'_>) -> Result<T, GasAdmissionError>,
) -> Result<T, GasAdmissionError> {
    let mut parser = json::Parser::new(source.get());
    parser.preflight_document()?;
    parser.skip_ws();
    let value = decode(&mut parser)?;
    parser.finish_document()?;
    Ok(value)
}
fn reserve_logical_array<T>(count: usize) -> Result<(), GasAdmissionError> {
    let layout = Layout::array::<T>(count).map_err(|_| json::Error::DecodeResourceLimit)?;
    norito::core::reserve_decode_allocation(layout.size())?;
    Ok(())
}

struct TextCustody {
    array: AllocationCharge,
    strings: ChargedBuffer<AllocationCharge>,
}
impl TextCustody {
    fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.array.belongs_to(pool)
            && self.strings.belongs_to(pool)
            && self.strings.as_slice().iter().all(|c| c.belongs_to(pool))
    }
}
struct TextArray {
    values: ChargedBuffer<String>,
    strings: ChargedBuffer<AllocationCharge>,
}
struct Texts {
    value: Vec<String>,
    custody: TextCustody,
}
impl TextArray {
    fn new(count: usize, pool: &AllocationBudget) -> Result<Self, GasAdmissionError> {
        Ok(Self {
            values: ChargedBuffer::new(count, pool)?,
            strings: ChargedBuffer::new(count, pool)?,
        })
    }
    #[allow(unsafe_code)]
    fn push(&mut self, value: Text) {
        // SAFETY: both arrays have exact preadmitted capacity and cannot grow.
        // Physical strings retire before the original nested charge ledger.
        let (value, charge) = unsafe { value.into_parts() };
        self.values.push_reserved(value);
        self.strings.push_reserved(charge);
    }
    #[allow(unsafe_code)]
    fn finish(self) -> Texts {
        // SAFETY: exact move into a closed immutable value-first owner.
        let (value, array) = unsafe { self.values.into_allocation_parts() };
        Texts {
            value,
            custody: TextCustody {
                array,
                strings: self.strings,
            },
        }
    }
}

// Sort only preadmitted inline keys/indices. Invalid addresses are left for the
// original semantic walk, so a later invalid address cannot hide an earlier
// duplicate (or an earlier rate's invalid decimal). O(n log n), no tree nodes.
fn first_duplicate<'a>(
    values: impl ExactSizeIterator<Item = &'a str>,
    pool: &AllocationBudget,
) -> Result<Option<usize>, GasAdmissionError> {
    let mut scratch = ChargedBuffer::new(values.len(), pool)?;
    for (index, text) in values.enumerate() {
        let key = AssetDefinitionId::parse_address_literal(text)
            .ok()
            .map(|id| id.aid_bytes());
        scratch.push_reserved((key, index));
    }
    scratch.as_mut_slice().sort_unstable();
    Ok(scratch
        .as_slice()
        .windows(2)
        .filter(|w| w[0].0.is_some() && w[0].0 == w[1].0)
        .map(|w| w[1].1)
        .min())
}
fn decode_assets(source: &Json, pool: &AllocationBudget) -> Result<Texts, GasAdmissionError> {
    let values = document(source, |p| {
        p.parse_array_with(
            |count| {
                reserve_logical_array::<String>(count)?;
                TextArray::new(count, pool)
            },
            |p| Text::decode(p, pool),
            TextArray::push,
        )
    })?;
    let duplicate = first_duplicate(values.values.as_slice().iter().map(String::as_str), pool)?;
    for (index, value) in values.values.as_slice().iter().enumerate() {
        super::canonical_asset(value, ACCEPTED_ASSETS).map_err(norito::Error::Message)?;
        if duplicate == Some(index) {
            return Err(norito::Error::Message(format!(
                "{ACCEPTED_ASSETS} contains duplicate asset `{value}`"
            ))
            .into());
        }
    }
    Ok(values.finish())
}

struct RateCustody {
    array: AllocationCharge,
    leaves: ChargedBuffer<[AllocationCharge; 2]>,
}
impl RateCustody {
    fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.array.belongs_to(pool)
            && self.leaves.belongs_to(pool)
            && self
                .leaves
                .as_slice()
                .iter()
                .flatten()
                .all(|c| c.belongs_to(pool))
    }
}
struct RateArray {
    values: ChargedBuffer<GasRate>,
    leaves: ChargedBuffer<[AllocationCharge; 2]>,
}
struct Rates {
    value: Vec<GasRate>,
    custody: RateCustody,
}
impl RateArray {
    fn new(count: usize, pool: &AllocationBudget) -> Result<Self, GasAdmissionError> {
        Ok(Self {
            values: ChargedBuffer::new(count, pool)?,
            leaves: ChargedBuffer::new(count, pool)?,
        })
    }
    #[allow(unsafe_code)]
    fn push(
        &mut self,
        asset: Text,
        units_per_gas: u64,
        twap: ChargedNumeric,
        liquidity: GasLiquidity,
        volatility: GasVolatility,
    ) {
        // SAFETY: both original leaves transfer immediately to preadmitted fixed
        // arrays; values are destroyed before their exact original charges.
        let (asset, asset_charge) = unsafe { asset.into_parts() };
        let (twap_local_per_xor, numeric_charge) = unsafe { twap.into_allocation_parts() };
        self.values.push_reserved(GasRate {
            asset,
            units_per_gas,
            twap_local_per_xor,
            liquidity,
            volatility,
        });
        self.leaves.push_reserved([asset_charge, numeric_charge]);
    }
    #[allow(unsafe_code)]
    fn finish(self) -> Rates {
        // SAFETY: exact immutable value-first ownership transfer without growth.
        let (value, array) = unsafe { self.values.into_allocation_parts() };
        Rates {
            value,
            custody: RateCustody {
                array,
                leaves: self.leaves,
            },
        }
    }
}

struct RawRate<T> {
    asset: T,
    units_per_gas: u64,
    twap_local_per_xor: Option<T>,
    liquidity_profile: Option<T>,
    volatility_class: Option<T>,
}

trait TextDestination {
    type Text: AsRef<str>;
    type Error: From<json::Error>;
    fn text(&self, p: &mut json::Parser<'_>) -> Result<Self::Text, Self::Error>;
    fn key<'a>(
        &self,
        p: &mut json::Parser<'a>,
    ) -> Result<json::KeyRef<'a, Self::Text>, Self::Error>;
}
struct Ordinary;
impl TextDestination for Ordinary {
    type Text = String;
    type Error = json::Error;
    fn text(&self, p: &mut json::Parser<'_>) -> Result<String, json::Error> {
        p.parse_string()
    }
    fn key<'a>(&self, p: &mut json::Parser<'a>) -> Result<json::KeyRef<'a>, json::Error> {
        p.parse_key()
    }
}
struct Admitted<'a>(&'a AllocationBudget);
impl TextDestination for Admitted<'_> {
    type Text = Text;
    type Error = GasAdmissionError;
    fn text(&self, p: &mut json::Parser<'_>) -> Result<Text, Self::Error> {
        Text::decode(p, self.0)
    }
    fn key<'a>(&self, p: &mut json::Parser<'a>) -> Result<json::KeyRef<'a, Text>, Self::Error> {
        p.parse_key_with_buffer(|count| Text::new(count, self.0))
    }
}
fn optional_text<D: TextDestination>(
    p: &mut json::Parser<'_>,
    d: &D,
) -> Result<Option<D::Text>, D::Error> {
    if p.try_consume_null()? {
        Ok(None)
    } else {
        d.text(p).map(Some)
    }
}
fn raw_rate<D: TextDestination>(
    p: &mut json::Parser<'_>,
    d: &D,
) -> Result<RawRate<D::Text>, D::Error> {
    p.skip_ws();
    p.preflight_object_entries()?;
    p.expect(b'{')?;
    p.skip_ws();
    let mut asset = None;
    let mut units_per_gas = None;
    let mut twap_local_per_xor = None;
    let mut liquidity_profile = None;
    let mut volatility_class = None;
    if !p.try_consume_char(b'}')? {
        loop {
            p.skip_ws();
            let key = d.key(p)?;
            let key = match &key {
                json::KeyRef::Borrowed(v) => *v,
                json::KeyRef::Owned(v) => v.as_ref(),
            };
            macro_rules! field {
                ($slot:ident, $value:expr) => {{
                    if $slot.is_some() {
                        return Err(json::Error::duplicate_field(stringify!($slot)).into());
                    }
                    $slot = Some($value);
                }};
            }
            match key {
                "asset" => field!(asset, d.text(p)?),
                "units_per_gas" => field!(units_per_gas, u64::json_deserialize(p)?),
                "twap_local_per_xor" => field!(twap_local_per_xor, optional_text(p, d)?),
                "liquidity_profile" => field!(liquidity_profile, optional_text(p, d)?),
                "volatility_class" => field!(volatility_class, optional_text(p, d)?),
                _ => return Err(json::Error::unknown_field(key).into()),
            }
            p.skip_ws();
            if p.try_consume_char(b',')? {
                continue;
            }
            p.expect(b'}')?;
            break;
        }
    }
    Ok(RawRate {
        asset: asset.ok_or_else(|| json::Error::missing_field("asset"))?,
        units_per_gas: units_per_gas.ok_or_else(|| json::Error::missing_field("units_per_gas"))?,
        // Match the canonical derive: omitted optional fields are None. Keep
        // the outer presence slots until here so repeated null still rejects.
        twap_local_per_xor: twap_local_per_xor.unwrap_or(None),
        liquidity_profile: liquidity_profile.unwrap_or(None),
        volatility_class: volatility_class.unwrap_or(None),
    })
}
pub(super) fn decode_ordinary_rate(
    p: &mut json::Parser<'_>,
) -> Result<CommittedGasRate, json::Error> {
    let value = raw_rate(p, &Ordinary)?;
    Ok(CommittedGasRate {
        asset: value.asset,
        units_per_gas: value.units_per_gas,
        twap_local_per_xor: value.twap_local_per_xor,
        liquidity_profile: value.liquidity_profile,
        volatility_class: value.volatility_class,
    })
}

fn decode_rates(source: &Json, pool: &AllocationBudget) -> Result<Rates, GasAdmissionError> {
    let mut raw = document(source, |p| {
        p.parse_array_with(
            |count| {
                reserve_logical_array::<CommittedGasRate>(count)?;
                ChargedBuffer::<RawRate<Text>>::new(count, pool).map_err(GasAdmissionError::from)
            },
            |p| raw_rate(p, &Admitted(pool)),
            ChargedBuffer::push_reserved,
        )
    })?;
    let duplicate = first_duplicate(raw.as_slice().iter().map(|v| v.asset.as_ref()), pool)?;
    let mut output = RateArray::new(raw.as_slice().len(), pool)?;
    for (index, value) in raw.drain_all().enumerate() {
        super::canonical_asset(value.asset.as_ref(), RATES).map_err(norito::Error::Message)?;
        if duplicate == Some(index) {
            return Err(norito::Error::Message(format!(
                "{RATES} contains duplicate asset `{}`",
                value.asset.as_ref()
            ))
            .into());
        }
        let twap_text = value.twap_local_per_xor.as_ref().map_or("1", AsRef::as_ref);
        let twap =
            ChargedNumeric::try_from_decimal(twap_text, pool).map_err(|error| match error {
                NumericDecimalAdmissionError::Numeric(error) => GasAdmissionError::Decode(
                    norito::Error::Message(format!("invalid {RATES} twap `{twap_text}`: {error}")),
                ),
                NumericDecimalAdmissionError::Decode(error) => error.into(),
                NumericDecimalAdmissionError::Allocation(error) => error.into(),
            })?;
        if twap.get() <= &Numeric::zero() {
            return Err(norito::Error::Message(format!(
                "invalid {RATES} twap `{twap_text}`: value must be positive"
            ))
            .into());
        }
        let liquidity = super::rate_liquidity(value.liquidity_profile.as_ref().map(AsRef::as_ref))?;
        let volatility =
            super::rate_volatility(value.volatility_class.as_ref().map(AsRef::as_ref))?;
        output.push(
            value.asset,
            value.units_per_gas,
            twap,
            liquidity,
            volatility,
        );
    }
    Ok(output.finish())
}

#[cfg(test)]
mod tests;
