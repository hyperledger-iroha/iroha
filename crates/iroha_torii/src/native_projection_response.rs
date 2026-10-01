//! One finite encoder and last-byte memory owner for native complete-state projections.

use super::*;
use iroha_allocation::AllocationBudget;
use norito::json::{BoundedJsonError, JsonWriteSink};

fn capacity() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
    ))
}

pub(crate) struct EncodedBody {
    bytes: iroha_allocation::ChargedBuffer<u8>,
    pub(crate) memory: Option<QueryFanoutMemoryReservation>,
}
impl AsRef<[u8]> for EncodedBody {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}
struct ChargedWriter(iroha_allocation::ChargedBuffer<u8>);
impl std::io::Write for ChargedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.0.capacity().saturating_sub(self.0.as_slice().len()) {
            return Err(std::io::Error::other("native response length changed"));
        }
        for byte in bytes {
            self.0.push_reserved(*byte);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl JsonWriteSink for ChargedWriter {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes))
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        std::io::Write::write_all(self, value.as_bytes())
            .map_err(|_| BoundedJsonError::LengthMismatch)
    }
}
struct CountJson {
    length: usize,
    limit: usize,
    depth: usize,
}
impl JsonWriteSink for CountJson {
    fn push(&mut self, value: char) -> Result<(), BoundedJsonError> {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes))
    }
    fn push_str(&mut self, value: &str) -> Result<(), BoundedJsonError> {
        let next = self
            .length
            .checked_add(value.len())
            .ok_or(BoundedJsonError::BodyTooLarge)?;
        if next > self.limit {
            return Err(BoundedJsonError::BodyTooLarge);
        }
        self.length = next;
        Ok(())
    }
    fn begin_container(&mut self) -> Result<(), BoundedJsonError> {
        let next = self
            .depth
            .checked_add(1)
            .ok_or(BoundedJsonError::Unsupported)?;
        if next >= norito::json::MAX_JSON_VALUE_NESTING_DEPTH {
            return Err(BoundedJsonError::Unsupported);
        }
        self.depth = next;
        Ok(())
    }
    fn end_container(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}
pub(crate) fn encode<T: norito::core::NoritoSerialize + norito::json::JsonSerialize>(
    payload: &T,
    format: ResponseFormat,
    limit: usize,
    budget: &AllocationBudget,
    unavailable: fn() -> Error,
) -> Result<EncodedBody, Error> {
    let length = match format {
        ResponseFormat::Norito => {
            norito::canonical_frame_len(payload).map_err(|_| unavailable())?
        }
        ResponseFormat::Json => {
            let mut count = CountJson {
                length: 0,
                limit,
                depth: 0,
            };
            payload
                .json_serialize_to(&mut count)
                .map_err(|_| capacity())?;
            count.length
        }
    };
    if length > limit {
        return Err(capacity());
    }
    let bytes = iroha_allocation::ChargedBuffer::new(length, budget).map_err(|_| capacity())?;
    let mut writer = ChargedWriter(bytes);
    match format {
        ResponseFormat::Norito => norito::core::write_canonical_to_writer(payload, &mut writer)
            .map_err(|_| unavailable())?,
        ResponseFormat::Json => payload
            .json_serialize_to(&mut writer)
            .map_err(|_| unavailable())?,
    }
    if writer.0.as_slice().len() != length {
        return Err(unavailable());
    }
    Ok(EncodedBody {
        bytes: writer.0,
        memory: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn charged_native_output_rejects_growth_and_refunds_only_when_backing_drops() {
        let budget = AllocationBudget::new(3);
        let mut writer = ChargedWriter(iroha_allocation::ChargedBuffer::new(3, &budget).unwrap());
        writer.write_all(b"abc").unwrap();
        assert!(writer.write_all(b"d").is_err());
        assert_eq!(writer.0.as_slice(), b"abc");
        let bytes = Bytes::from_owner(EncodedBody {
            bytes: writer.0,
            memory: None,
        });
        let retained = bytes.clone();
        drop(bytes);
        assert_eq!(budget.reserved_bytes(), 3);
        drop(retained);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn json_preflight_counts_utf8_and_rejects_length_and_depth_before_output() {
        let mut count = CountJson {
            length: 0,
            limit: 4,
            depth: 0,
        };
        count.push('😀').unwrap();
        assert_eq!(count.length, 4);
        assert_eq!(count.push('a'), Err(BoundedJsonError::BodyTooLarge));
        assert_eq!(count.length, 4);
        for _ in 1..norito::json::MAX_JSON_VALUE_NESTING_DEPTH {
            count.begin_container().unwrap();
        }
        assert_eq!(count.begin_container(), Err(BoundedJsonError::Unsupported));
    }

    #[test]
    fn native_response_last_byte_retains_real_aggregate_query_permit() {
        let pool = ByteWeightedMemoryPool::new(3).unwrap();
        let permit = pool.try_acquire_parts([3]).unwrap();
        let budget = AllocationBudget::new(3);
        let mut writer = ChargedWriter(iroha_allocation::ChargedBuffer::new(3, &budget).unwrap());
        writer.write_all(b"abc").unwrap();
        let bytes = Bytes::from_owner(EncodedBody {
            bytes: writer.0,
            memory: Some(QueryFanoutMemoryReservation::new(permit)),
        });
        let retained = bytes.slice(1..);
        drop(bytes);
        assert!(pool.try_acquire_parts([1]).is_none());
        assert_eq!(budget.reserved_bytes(), 3);
        drop(retained);
        assert!(pool.try_acquire_parts([3]).is_some());
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
