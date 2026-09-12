//! Exact, cumulative decoding for opaque iterable-query components.
use super::{Error, QueryLimits};
use norito::core::{DeserializePayload, SerializePayload};
struct ExactBareWriter<'a> {
    expected: &'a [u8],
    written: usize,
}
impl std::io::Write for ExactBareWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .written
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
        if self.expected.get(self.written..end) != Some(bytes) {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
        }
        self.written = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn decode_exact_in_scope<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: SerializePayload + for<'de> DeserializePayload<'de>,
{
    let value = norito::codec::decode_adaptive::<T>(bytes)
        .map_err(|_| Error::Conversion("failed to decode iterable query component".to_owned()))?;
    let mut writer = ExactBareWriter {
        expected: bytes,
        written: 0,
    };
    let written = norito::codec::encode_adaptive_into(&value, &mut writer).map_err(|_| {
        Error::Conversion("iterable query component is not canonically encoded".to_owned())
    })?;
    if written != bytes.len() || writer.written != bytes.len() {
        return Err(Error::Conversion(
            "iterable query component is not canonically encoded".to_owned(),
        ));
    }
    Ok(value)
}
/// Cumulative decoder for the query, predicate, and selector in one Start.
///
/// Ordinary ingress retains the outer erased request in half of its observable
/// request-graph cap. The three nested typed graphs share the other half, and
/// each successful decode permanently reduces that remaining allowance.
pub(super) struct FastIterComponentDecoder {
    remaining_allocated_bytes: usize,
    maximum_component_bytes: usize,
}
impl FastIterComponentDecoder {
    pub(super) fn new(limits: QueryLimits, components: [&[u8]; 3]) -> Result<Self, Error> {
        let encoded_bytes = components.into_iter().try_fold(0_usize, |total, bytes| {
            total.checked_add(bytes.len()).ok_or(Error::CapacityLimit)
        })?;
        let ordinary_half = limits
            .ordinary_execution_limits
            .map(|ordinary| ordinary.max_request_graph_bytes() / 2)
            .map(|bytes| usize::try_from(bytes).map_err(|_| Error::CapacityLimit))
            .transpose()?;
        if ordinary_half.is_some_and(|maximum| encoded_bytes > maximum) {
            return Err(Error::CapacityLimit);
        }
        Ok(Self {
            remaining_allocated_bytes: ordinary_half.unwrap_or(usize::MAX),
            maximum_component_bytes: ordinary_half.unwrap_or(usize::MAX),
        })
    }
    pub(super) fn decode<T>(&mut self, bytes: &[u8]) -> Result<T, Error>
    where
        T: SerializePayload + for<'de> DeserializePayload<'de>,
    {
        self.try_decode_measured(bytes)?.ok_or_else(|| {
            Error::Conversion("failed to decode iterable query component".to_owned())
        })
    }
    /// Try one of several concrete query variants sharing an item kind.
    pub(super) fn try_decode<T>(&mut self, bytes: &[u8]) -> Result<Option<T>, Error>
    where
        T: SerializePayload + for<'de> DeserializePayload<'de>,
    {
        self.try_decode_measured(bytes)
    }
    fn try_decode_measured<T>(&mut self, bytes: &[u8]) -> Result<Option<T>, Error>
    where
        T: SerializePayload + for<'de> DeserializePayload<'de>,
    {
        if bytes.len() > self.maximum_component_bytes {
            return Err(Error::CapacityLimit);
        }
        let elements = bytes.len().checked_mul(8).ok_or(Error::CapacityLimit)?;
        let limits = norito::DecodeLimits::new(
            elements,
            bytes.len(),
            elements,
            self.remaining_allocated_bytes,
            norito::core::MAX_VALUE_NESTING_DEPTH,
        );
        let (decoded, usage) =
            norito::core::with_decode_limits_measured(limits, || decode_exact_in_scope::<T>(bytes));
        let Ok(decoded) = decoded else {
            return Ok(None);
        };
        self.remaining_allocated_bytes = self
            .remaining_allocated_bytes
            .checked_sub(usage.total_allocated_bytes())
            .ok_or(Error::CapacityLimit)?;
        Ok(Some(decoded))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::query::{domain::prelude::FindDomains, dsl::CompoundPredicate};
    use iroha_model_base::peer::PeerId;
    // Iterable components are embedded payloads; this fixture intentionally has no frame identity.
    #[derive(Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
    struct PayloadOnlyComponent {
        values: Vec<String>,
    }
    #[test]
    fn payload_only_components_require_exact_canonical_bytes() {
        let value = PayloadOnlyComponent {
            values: vec!["first".to_owned(), "second".to_owned()],
        };
        let bytes = norito::codec::Encode::encode(&value);
        assert_eq!(
            decode_exact_in_scope::<PayloadOnlyComponent>(&bytes).unwrap(),
            value
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_exact_in_scope::<PayloadOnlyComponent>(&trailing).is_err());
        assert!(decode_exact_in_scope::<PayloadOnlyComponent>(&bytes[..bytes.len() - 1]).is_err());
    }
    #[test]
    fn payload_only_components_share_the_cumulative_allocation_budget() {
        let value = PayloadOnlyComponent {
            values: vec!["first".to_owned(), "second".to_owned()],
        };
        let bytes = norito::codec::Encode::encode(&value);
        let mut decoder = FastIterComponentDecoder::new(QueryLimits::default(), [&bytes, &[], &[]])
            .expect("payload fits default limits");
        let before = decoder.remaining_allocated_bytes;
        assert_eq!(
            decoder.decode::<PayloadOnlyComponent>(&bytes).unwrap(),
            value
        );
        let charge = before - decoder.remaining_allocated_bytes;
        assert!(charge > 0, "decoded vectors and strings must be charged");
        decoder.remaining_allocated_bytes = charge;
        assert_eq!(
            decoder.try_decode::<PayloadOnlyComponent>(&bytes).unwrap(),
            Some(value)
        );
        assert_eq!(decoder.remaining_allocated_bytes, 0);
        assert!(
            decoder
                .try_decode::<PayloadOnlyComponent>(&bytes)
                .unwrap()
                .is_none()
        );
        assert_eq!(decoder.remaining_allocated_bytes, 0);
        assert!(decoder.decode::<PayloadOnlyComponent>(&bytes).is_err());
    }
    #[test]
    fn exact_decode_accepts_canonical_unit_and_rejects_trailing_or_incomplete_values() {
        let bytes = norito::codec::Encode::encode(&FindDomains);
        assert!(decode_exact_in_scope::<FindDomains>(&bytes).is_ok());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_exact_in_scope::<FindDomains>(&trailing).is_err());
        assert!(decode_exact_in_scope::<u64>(&[]).is_err());
    }
    #[test]
    fn component_decoder_accepts_compact_values_shorter_than_their_archived_layout() {
        let bytes = norito::codec::Encode::encode(&CompoundPredicate::<PeerId>::PASS);
        assert!(bytes.len() < norito::core::archived_payload_size::<CompoundPredicate<PeerId>>());
        let mut decoder = FastIterComponentDecoder::new(QueryLimits::default(), [&bytes, &[], &[]])
            .expect("compact components fit the default query limits");
        assert!(decoder.decode::<CompoundPredicate<PeerId>>(&bytes).is_ok());
    }
}
