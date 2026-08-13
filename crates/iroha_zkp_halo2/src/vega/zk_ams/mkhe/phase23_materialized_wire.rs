//! Bounded canonical streaming for materialized Phase-II/III accumulators.
use super::manifest::release_profile_v1;
use super::phase23_encrypted::{
    PHASE23_ENCRYPTED_VERSION_V1, PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1,
    PHASE23_MAX_BATCH_SIZE_V1, ZkAmsPhase23AccumulatorShapeV1,
    ZkAmsPhase23MaterializedAccumulatorsV1, materialized_wire_length, validate_accumulator_shape,
    validate_materialized,
};
use super::{Scalar, ZkAmsMkheErrorV1};
use std::io::{Read, Write};
struct ZeroizingMaterializedScalarBytesV1([u8; 32]);
impl ZeroizingMaterializedScalarBytesV1 {
    fn new(value: Scalar) -> Self {
        Self(value.to_be_bytes())
    }
}
#[cfg(test)]
std::thread_local! {
    static MATERIALIZED_SCALAR_BYTES_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}
#[cfg(test)]
pub(super) fn materialized_scalar_bytes_zeroized_drop_count_v1() -> usize {
    MATERIALIZED_SCALAR_BYTES_ZEROIZED_DROPS_V1
        .try_with(std::cell::Cell::get)
        .unwrap_or(0)
}
impl Drop for ZeroizingMaterializedScalarBytesV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        #[cfg(test)]
        if bytes.iter().all(|byte| *byte == 0) {
            let _ = MATERIALIZED_SCALAR_BYTES_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
        let _ = core::hint::black_box(&mut *bytes);
    }
}
struct ZeroizingMaterializedWireBufferV1([u8; PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1]);
impl ZeroizingMaterializedWireBufferV1 {
    const fn new() -> Self {
        Self([0; PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1])
    }
    fn prefix_mut(&mut self, length: usize) -> Result<&mut [u8], ZkAmsMkheErrorV1> {
        self.0
            .get_mut(..length)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
    fn prefix(&self, length: usize) -> Result<&[u8], ZkAmsMkheErrorV1> {
        self.0
            .get(..length)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
}
#[cfg(test)]
std::thread_local! {
    static MATERIALIZED_WIRE_BUFFER_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}
#[cfg(test)]
pub(super) fn materialized_wire_buffer_zeroized_drop_count_v1() -> usize {
    MATERIALIZED_WIRE_BUFFER_ZEROIZED_DROPS_V1
        .try_with(std::cell::Cell::get)
        .unwrap_or(0)
}
impl Drop for ZeroizingMaterializedWireBufferV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        #[cfg(test)]
        if bytes.iter().all(|byte| *byte == 0) {
            let _ = MATERIALIZED_WIRE_BUFFER_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
        let _ = core::hint::black_box(&mut *bytes);
    }
}
/// Stream the exact canonical materialized-accumulator representation.
///
/// A writer failure can leave an unauthoritative canonical prefix in the
/// supplied sink. Callers that require atomic persistence must provide a
/// transactional sink and commit it only after this function returns `Ok`.
/// The codec itself never prebuffers, but a caller-selected buffering writer
/// can still retain the complete wire as provider-owned residency.
pub fn write_zk_ams_phase23_materialized_accumulators_canonical_v1<W: Write + ?Sized>(
    value: &ZkAmsPhase23MaterializedAccumulatorsV1,
    writer: &mut W,
) -> Result<(), ZkAmsMkheErrorV1> {
    if value.profile_digest != release_profile_v1().digest()? {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    validate_materialized(value)?;
    let expected_bytes = materialized_wire_length(value.shape)?;
    let mut written_bytes = 0_usize;
    write_exact_v1(writer, &[value.version], &mut written_bytes)?;
    for digest in [
        &value.profile_digest,
        &value.roster_digest,
        &value.transcript_digest,
        &value.batch_id,
        &value.ordered_batch_input_digest,
    ] {
        write_exact_v1(writer, digest, &mut written_bytes)?;
    }
    write_exact_v1(writer, &[value.fold_count], &mut written_bytes)?;
    for length in [
        value.shape.x,
        1,
        value.shape.e,
        value.shape.r_e,
        value.shape.w,
        value.shape.r_w,
    ] {
        write_exact_v1(writer, &length.to_be_bytes(), &mut written_bytes)?;
    }
    for family in [
        value.x.as_slice(),
        value.u.as_slice(),
        value.e.as_slice(),
        value.r_e.as_slice(),
        value.w.as_slice(),
        value.r_w.as_slice(),
    ] {
        for scalar in family {
            let scalar_bytes = ZeroizingMaterializedScalarBytesV1::new(*scalar);
            write_exact_v1(writer, &scalar_bytes.0, &mut written_bytes)?;
        }
    }
    write_exact_v1(writer, &value.digest, &mut written_bytes)?;
    if written_bytes != expected_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
/// Read exactly one canonical materialized-accumulator representation.
///
/// The fixed header is validated before any family allocation. All six final
/// family owners are then reserved before the first family byte is read, so a
/// partial read, malformed scalar, I/O error, or unwind zeroizes the partially
/// populated final owner. Immediate EOF is required after the digest footer.
/// The codec never prebuffers its source; a caller-selected reader such as a
/// `Cursor<Vec<u8>>` can still retain the whole wire as provider-owned state.
pub fn read_zk_ams_phase23_materialized_accumulators_canonical_exact_v1<R: Read + ?Sized>(
    reader: &mut R,
) -> Result<ZkAmsPhase23MaterializedAccumulatorsV1, ZkAmsMkheErrorV1> {
    let mut buffer = ZeroizingMaterializedWireBufferV1::new();
    read_exact_v1(
        reader,
        buffer.prefix_mut(PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1)?,
    )?;
    let header = buffer.prefix(PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1)?;
    let mut cursor = 0_usize;
    let version = read_header_array_v1::<1>(header, &mut cursor)?[0];
    let profile_digest = read_header_array_v1(header, &mut cursor)?;
    let roster_digest = read_header_array_v1(header, &mut cursor)?;
    let transcript_digest = read_header_array_v1(header, &mut cursor)?;
    let batch_id = read_header_array_v1(header, &mut cursor)?;
    let ordered_batch_input_digest = read_header_array_v1(header, &mut cursor)?;
    let fold_count = read_header_array_v1::<1>(header, &mut cursor)?[0];
    let mut lengths = [0_u32; 6];
    for length in &mut lengths {
        *length = u32::from_be_bytes(read_header_array_v1(header, &mut cursor)?);
    }
    let release_profile_digest = release_profile_v1().digest()?;
    if cursor != PHASE23_MATERIALIZED_WIRE_HEADER_BYTES_V1
        || version != PHASE23_ENCRYPTED_VERSION_V1
        || profile_digest != release_profile_digest
        || fold_count == 0
        || fold_count > PHASE23_MAX_BATCH_SIZE_V1
        || [
            profile_digest,
            roster_digest,
            transcript_digest,
            batch_id,
            ordered_batch_input_digest,
        ]
        .contains(&[0; 32])
        || lengths[1] != 1
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let shape = ZkAmsPhase23AccumulatorShapeV1 {
        x: lengths[0],
        e: lengths[2],
        r_e: lengths[3],
        w: lengths[4],
        r_w: lengths[5],
    };
    validate_accumulator_shape(shape).map_err(wire_validation_error_v1)?;
    let _expected_bytes = materialized_wire_length(shape)?;
    let mut materialized = ZkAmsPhase23MaterializedAccumulatorsV1 {
        version,
        profile_digest,
        roster_digest,
        transcript_digest,
        batch_id,
        ordered_batch_input_digest,
        fold_count,
        shape,
        x: Vec::new(),
        u: Vec::new(),
        e: Vec::new(),
        r_e: Vec::new(),
        w: Vec::new(),
        r_w: Vec::new(),
        digest: [0; 32],
    };
    for (family, length) in [
        &mut materialized.x,
        &mut materialized.u,
        &mut materialized.e,
        &mut materialized.r_e,
        &mut materialized.w,
        &mut materialized.r_w,
    ]
    .into_iter()
    .zip(lengths)
    {
        family
            .try_reserve_exact(
                usize::try_from(length).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    for (family, length) in [
        &mut materialized.x,
        &mut materialized.u,
        &mut materialized.e,
        &mut materialized.r_e,
        &mut materialized.w,
        &mut materialized.r_w,
    ]
    .into_iter()
    .zip(lengths)
    {
        for _ in 0..length {
            read_exact_v1(reader, buffer.prefix_mut(32)?)?;
            let scalar = Scalar::from_be_bytes_exact(
                buffer
                    .prefix(32)?
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            )
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            family.push(scalar);
        }
    }
    read_exact_v1(reader, buffer.prefix_mut(32)?)?;
    materialized.digest = buffer
        .prefix(32)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    require_eof_v1(reader, &mut buffer)?;
    validate_materialized(&materialized).map_err(wire_validation_error_v1)?;
    Ok(materialized)
}
fn write_exact_v1<W: Write + ?Sized>(
    writer: &mut W,
    bytes: &[u8],
    written_bytes: &mut usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    writer
        .write_all(bytes)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *written_bytes = written_bytes
        .checked_add(bytes.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    Ok(())
}
fn read_exact_v1<R: Read + ?Sized>(
    reader: &mut R,
    bytes: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    reader
        .read_exact(bytes)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
}
fn read_header_array_v1<const N: usize>(
    header: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let end = cursor
        .checked_add(N)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let value = header
        .get(*cursor..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *cursor = end;
    Ok(value)
}
fn require_eof_v1<R: Read + ?Sized>(
    reader: &mut R,
    buffer: &mut ZeroizingMaterializedWireBufferV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    loop {
        match reader.read(buffer.prefix_mut(1)?) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}
fn wire_validation_error_v1(error: ZkAmsMkheErrorV1) -> ZkAmsMkheErrorV1 {
    match error {
        ZkAmsMkheErrorV1::ResourceCeilingExceeded => error,
        _ => ZkAmsMkheErrorV1::InvalidWireEncoding,
    }
}
