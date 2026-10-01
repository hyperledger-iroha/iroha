//! Borrowed identity selection inside already authenticated canonical lane-state bytes.
//!
//! These scanners allocate no selected graph and grant no authority to their input. Only the
//! enclosing original carrier owner exposes their results. The current fixed Norito layout is
//! the only accepted layout; no fallback field counts or byte-array encodings are accepted.

use norito::core as ncore;

pub(super) fn field<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], norito::Error> {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    // This borrows already funded bytes; charge no decoder allocation for field length.
    let (length, prefix) = ncore::inspect_len_from_slice(input)?;
    let end = prefix
        .checked_add(length)
        .ok_or(norito::Error::LengthMismatch)?;
    let value = input
        .get(prefix..end)
        .ok_or(norito::Error::LengthMismatch)?;
    *input = input.get(end..).ok_or(norito::Error::LengthMismatch)?;
    Ok(value)
}
pub(super) fn fields<const N: usize>(mut input: &[u8]) -> Result<[&[u8]; N], norito::Error> {
    let mut output = [&[][..]; N];
    for entry in &mut output {
        *entry = field(&mut input)?;
    }
    if !input.is_empty() {
        return Err(norito::Error::LengthMismatch);
    }
    Ok(output)
}
pub(super) fn identity(bytes: &[u8]) -> Result<[u8; 32], norito::Error> {
    // Norito's derived named [u8; N] fields use their exact raw width. The generic array
    // codec has a different layout and must never become a fallback for these fields.
    bytes.try_into().map_err(|_| norito::Error::LengthMismatch)
}

fn selected<'a, const N: usize>(
    sequence: &'a [u8],
    identity_field: usize,
    expected: &[u8; 32],
    maximum: usize,
) -> Result<Option<&'a [u8]>, norito::Error> {
    let count = sequence.get(..8).ok_or(norito::Error::LengthMismatch)?;
    let count = usize::try_from(u64::from_le_bytes(
        count.try_into().expect("eight-byte count"),
    ))
    .map_err(|_| norito::Error::LengthMismatch)?;
    let mut bytes = &sequence[8..];
    if count > maximum || count > bytes.len() {
        return Err(norito::Error::LengthMismatch);
    }
    let mut selected = None;
    for _ in 0..count {
        let record = field(&mut bytes)?;
        let values = fields::<N>(record)?;
        if identity(values[identity_field])? == *expected {
            if selected.is_some() {
                return Err(norito::Error::LengthMismatch);
            }
            selected = Some(record);
        }
    }
    if !bytes.is_empty() {
        return Err(norito::Error::LengthMismatch);
    }
    Ok(selected)
}

pub(super) fn lane<'a>(
    payload: &'a [u8],
    incarnation: &[u8; 32],
) -> Result<Option<&'a [u8]>, norito::Error> {
    let state = fields::<5>(payload)?;
    selected::<13>(
        state[0],
        3,
        incarnation,
        iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES,
    )
}
/// Select a retained original incarnation, including after its live lane record is removed.
pub(super) fn custody<'a>(
    payload: &'a [u8],
    incarnation: &[u8; 32],
) -> Result<Option<&'a [u8]>, norito::Error> {
    let state = fields::<5>(payload)?;
    selected::<10>(
        state[1],
        1,
        incarnation,
        iroha_data_model::sumeragi_lanes::MAX_LANE_CUSTODY_OBLIGATIONS,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::sumeragi_lanes::{
        SumeragiLaneFrontier, SumeragiLaneRecord, SumeragiLaneState,
    };
    use iroha_model_base::topology::{DataSpaceId, LaneId};
    use norito::codec::Encode;

    fn state() -> SumeragiLaneState {
        let lane = SumeragiLaneRecord {
            lane: LaneId::new(7),
            dataspace: DataSpaceId::new(1),
            incarnation: [1; 32],
            params: iroha_data_model::parameter::system::SumeragiParameters::default(),
            committee: Vec::new(),
            created_at: 20,
            active_from: 22,
            closing: None,
            anchor_freshness: 4,
            merged: SumeragiLaneFrontier::default(),
            merged_at: 22,
            rescued: 0,
            da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
        };
        SumeragiLaneState {
            lanes: vec![lane],
            ..SumeragiLaneState::default()
        }
    }

    #[test]
    fn borrowed_selection_matches_the_actual_current_codec_and_keeps_identities_distinct() {
        let state = state();
        let bytes = state.encode();
        let state_fields = fields::<5>(&bytes).expect("current lane-state fields");
        let mut records = &state_fields[0][8..];
        let encoded_record = field(&mut records).expect("canonical lane-record element");
        let record_fields = fields::<13>(encoded_record).expect("current lane-record fields");
        assert_eq!(
            identity(record_fields[3]).expect("canonical incarnation"),
            [1; 32]
        );
        let record = lane(&bytes, &[1; 32]).unwrap().unwrap();
        assert_eq!(record, state.lanes[0].encode());
        let first = bytes.as_ptr() as usize;
        assert!((record.as_ptr() as usize) >= first);
        assert!((record.as_ptr() as usize) + record.len() <= first + bytes.len());
        assert!(lane(&bytes, &[2; 32]).unwrap().is_none());
        let empty = SumeragiLaneState::default().encode();
        assert!(lane(&empty, &[1; 32]).unwrap().is_none());
    }

    #[test]
    fn borrowed_selection_rejects_duplicates_truncation_suffixes_and_unbounded_counts() {
        let mut source = state();
        source.lanes.push(source.lanes[0].clone());
        assert!(lane(&source.encode(), &[1; 32]).is_err());
        assert!(source.lanes.pop().is_some());
        let bytes = state().encode();
        for end in 0..bytes.len() {
            assert!(lane(&bytes[..end], &[1; 32]).is_err());
        }
        let mut suffix = bytes;
        suffix.push(0);
        assert!(lane(&suffix, &[1; 32]).is_err());
        assert!(selected::<13>(&u64::MAX.to_le_bytes(), 3, &[1; 32], 1024).is_err());
        assert!(
            identity(&[1_u8; 32].encode()).is_err(),
            "generic array framing is not the derived named-field layout"
        );
    }
}
