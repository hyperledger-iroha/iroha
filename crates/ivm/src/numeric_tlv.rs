//! Schema-bound pointer envelopes for Kotodama V1 exact numbers.
//!
//! Staged decoding charges bounded public work before each read, snapshots the
//! complete envelope once, authenticates that snapshot, and then performs
//! strict schema/canonical decoding. This ordering is consensus-visible.

use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, Quantity},
    numeric_abi::{
        DecimalValueV1, IntValueV1, MAX_DECIMAL_FRAME_BYTES_V1, MAX_INT_FRAME_BYTES_V1,
        MAX_QUANTITY_FRAME_BYTES_V1, NUMERIC_FRAME_HEADER_BYTES_V1,
        NUMERIC_POINTER_ENVELOPE_OVERHEAD_V1, NumericAbiError, QuantityValueV1,
    },
};

use crate::{
    IVM, PointerType, VMError,
    numeric::PointerAbiFaultV1,
    numeric_gas,
    syscall_metering::SyscallMeteringPhase,
};

const OUTER_HEADER_BYTES: usize = 7;
const OUTER_HASH_BYTES: usize = iroha_crypto::Hash::LENGTH;

fn pointer_fault(fault: PointerAbiFaultV1) -> VMError {
    VMError::PointerAbiFault(fault)
}

fn map_frame_error(error: NumericAbiError) -> VMError {
    let fault = match error {
        NumericAbiError::SchemaMismatch => PointerAbiFaultV1::SchemaMismatch,
        NumericAbiError::NonCanonicalMantissa
        | NumericAbiError::NonCanonicalDecimal
        | NumericAbiError::MantissaOverflow
        | NumericAbiError::InvalidScale
        | NumericAbiError::NegativeQuantity => PointerAbiFaultV1::NonCanonical,
        NumericAbiError::FrameTooLarge => PointerAbiFaultV1::OversizedLength,
        NumericAbiError::FrameTooShort
        | NumericAbiError::InvalidHeader
        | NumericAbiError::CompressionNotAllowed
        | NumericAbiError::LayoutFlagsNotAllowed
        | NumericAbiError::LengthMismatch
        | NumericAbiError::Norito(_) => PointerAbiFaultV1::MalformedFrame,
    };
    pointer_fault(fault)
}

fn encode_envelope(pointer_type: PointerType, frame: &[u8]) -> Result<Vec<u8>, VMError> {
    let length = u32::try_from(frame.len()).map_err(|_| VMError::GasCostOverflow)?;
    let capacity = OUTER_HEADER_BYTES
        .checked_add(frame.len())
        .and_then(|bytes| bytes.checked_add(OUTER_HASH_BYTES))
        .ok_or(VMError::GasCostOverflow)?;
    let mut envelope = Vec::with_capacity(capacity);
    envelope.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    envelope.push(1);
    envelope.extend_from_slice(&length.to_be_bytes());
    envelope.extend_from_slice(frame);
    envelope.extend_from_slice(iroha_crypto::Hash::new(frame).as_ref());
    Ok(envelope)
}

fn decode_envelope_bytes<'a>(
    envelope: &'a [u8],
    expected: PointerType,
    maximum_frame: usize,
) -> Result<&'a [u8], VMError> {
    if envelope.len() < OUTER_HEADER_BYTES {
        return Err(pointer_fault(PointerAbiFaultV1::TruncatedEnvelope));
    }
    let raw_type = u16::from_be_bytes([envelope[0], envelope[1]]);
    let pointer_type = PointerType::from_u16(raw_type)
        .ok_or_else(|| pointer_fault(PointerAbiFaultV1::UnknownType))?;
    if !crate::pointer_abi::is_type_allowed_for_policy(crate::SyscallPolicy::AbiV1, pointer_type) {
        return Err(pointer_fault(PointerAbiFaultV1::TypeNotAllowed));
    }
    if pointer_type != expected {
        return Err(pointer_fault(PointerAbiFaultV1::WrongType));
    }
    if envelope[2] != 1 {
        return Err(pointer_fault(PointerAbiFaultV1::InvalidEnvelopeVersion));
    }
    let frame_len = usize::try_from(u32::from_be_bytes([
        envelope[3],
        envelope[4],
        envelope[5],
        envelope[6],
    ]))
    .map_err(|_| pointer_fault(PointerAbiFaultV1::OversizedLength))?;
    if frame_len > maximum_frame {
        return Err(pointer_fault(PointerAbiFaultV1::OversizedLength));
    }
    let expected_len = OUTER_HEADER_BYTES
        .checked_add(frame_len)
        .and_then(|bytes| bytes.checked_add(OUTER_HASH_BYTES))
        .ok_or_else(|| pointer_fault(PointerAbiFaultV1::OversizedLength))?;
    if expected_len != envelope.len() {
        return Err(pointer_fault(PointerAbiFaultV1::TruncatedEnvelope));
    }
    let frame = &envelope[OUTER_HEADER_BYTES..OUTER_HEADER_BYTES + frame_len];
    let expected_hash = &envelope[OUTER_HEADER_BYTES + frame_len..];
    if iroha_crypto::Hash::new(frame).as_ref() != expected_hash {
        return Err(pointer_fault(PointerAbiFaultV1::PayloadHashMismatch));
    }
    Ok(frame)
}

fn snapshot_metered(
    vm: &mut IVM,
    pointer: u64,
    expected: PointerType,
    maximum_frame: usize,
) -> Result<Vec<u8>, VMError> {
    vm.charge_syscall_stage(
        SyscallMeteringPhase::PointerHeader,
        numeric_gas::POINTER_HEADER_BYTES,
    )?;
    vm.ensure_owned_public_tlv_range(pointer, OUTER_HEADER_BYTES as u64)
        .map_err(|_| pointer_fault(PointerAbiFaultV1::InvalidAddress))?;
    let header = vm
        .memory
        .load_region(pointer, OUTER_HEADER_BYTES as u64)
        .map_err(|_| pointer_fault(PointerAbiFaultV1::InvalidAddress))?;

    let raw_type = u16::from_be_bytes([header[0], header[1]]);
    let pointer_type = PointerType::from_u16(raw_type)
        .ok_or_else(|| pointer_fault(PointerAbiFaultV1::UnknownType))?;
    if !crate::pointer_abi::is_type_allowed_for_policy(vm.syscall_policy(), pointer_type) {
        return Err(pointer_fault(PointerAbiFaultV1::TypeNotAllowed));
    }
    if pointer_type != expected {
        return Err(pointer_fault(PointerAbiFaultV1::WrongType));
    }
    if header[2] != 1 {
        return Err(pointer_fault(PointerAbiFaultV1::InvalidEnvelopeVersion));
    }
    let frame_len = usize::try_from(u32::from_be_bytes([
        header[3], header[4], header[5], header[6],
    ]))
    .map_err(|_| pointer_fault(PointerAbiFaultV1::OversizedLength))?;
    if frame_len > maximum_frame {
        return Err(pointer_fault(PointerAbiFaultV1::OversizedLength));
    }
    let total = OUTER_HEADER_BYTES
        .checked_add(frame_len)
        .and_then(|bytes| bytes.checked_add(OUTER_HASH_BYTES))
        .ok_or_else(|| pointer_fault(PointerAbiFaultV1::OversizedLength))?;

    // Both bounded tail charges happen before the VM checks or reads the tail.
    // Malformed/truncated inputs therefore cannot perform free range probes.
    vm.charge_syscall_stage(
        SyscallMeteringPhase::PointerEnvelope,
        numeric_gas::POINTER_HASH_BYTES,
    )?;
    vm.charge_syscall_stage(
        SyscallMeteringPhase::PayloadHash,
        numeric_gas::checked_bytes(frame_len)?,
    )?;
    vm.ensure_owned_public_tlv_range(
        pointer,
        u64::try_from(total).map_err(|_| VMError::GasCostOverflow)?,
    )
    .map_err(|_| pointer_fault(PointerAbiFaultV1::TruncatedEnvelope))?;
    let snapshot = vm
        .memory
        .load_region(
            pointer,
            u64::try_from(total).map_err(|_| VMError::GasCostOverflow)?,
        )
        .map_err(|_| pointer_fault(PointerAbiFaultV1::TruncatedEnvelope))?
        .to_vec();

    // Authenticate exactly the bytes that subsequent stages decode.
    let frame = &snapshot[OUTER_HEADER_BYTES..OUTER_HEADER_BYTES + frame_len];
    let supplied_hash = &snapshot[OUTER_HEADER_BYTES + frame_len..];
    if iroha_crypto::Hash::new(frame).as_ref() != supplied_hash {
        return Err(pointer_fault(PointerAbiFaultV1::PayloadHashMismatch));
    }

    // The normative validation formula charges one logical unit for the first
    // complete/partial eight-byte word (header/schema/length decoding), then
    // one unit for each remaining word (minimal mantissa and domain checks).
    // Both charges precede the single strict decode below, and their sum is
    // exactly `4 * ceil(frame_len / 8)`.
    let (decode_work, canonical_work) =
        numeric_gas::numeric_frame_validation_phase_work(frame_len)?;
    vm.charge_syscall_stage(
        SyscallMeteringPhase::NoritoDecode,
        numeric_gas::work_gas(decode_work)?,
    )?;
    vm.charge_syscall_stage(
        SyscallMeteringPhase::CanonicalValidation,
        numeric_gas::work_gas(canonical_work)?,
    )?;
    Ok(snapshot)
}

fn exact_int_frame_len(value: &BigInt) -> Result<usize, VMError> {
    NUMERIC_FRAME_HEADER_BYTES_V1
        .checked_add(4)
        .and_then(|bytes| bytes.checked_add(value.to_twos_bytes().len()))
        .ok_or(VMError::GasCostOverflow)
}

fn exact_scaled_frame_len(value: &Numeric) -> Result<usize, VMError> {
    exact_int_frame_len(value.mantissa())?
        .checked_add(1)
        .ok_or(VMError::GasCostOverflow)
}

fn exact_envelope_len(frame_len: usize) -> Result<usize, VMError> {
    frame_len
        .checked_add(NUMERIC_POINTER_ENVELOPE_OVERHEAD_V1)
        .ok_or(VMError::GasCostOverflow)
}

fn charge_output(vm: &mut IVM, envelope_len: usize) -> Result<(), VMError> {
    vm.charge_syscall_stage(
        SyscallMeteringPhase::OutputSerialization,
        numeric_gas::checked_bytes(envelope_len)?,
    )
}

/// Encode a canonical V1 integer pointer envelope.
pub fn encode_int(value: &BigInt) -> Result<Vec<u8>, VMError> {
    let frame = IntValueV1::new(value.clone())
        .encode_frame()
        .map_err(map_frame_error)?;
    encode_envelope(PointerType::Int, &frame)
}

/// Encode a canonical V1 decimal pointer envelope.
pub fn encode_decimal(value: &Numeric) -> Result<Vec<u8>, VMError> {
    let frame = DecimalValueV1::from_canonical_numeric(value.clone())
        .map_err(|_| pointer_fault(PointerAbiFaultV1::NonCanonical))?
        .encode_frame()
        .map_err(map_frame_error)?;
    encode_envelope(PointerType::Decimal, &frame)
}

/// Encode a canonical V1 quantity pointer envelope.
pub fn encode_quantity(value: &Quantity) -> Result<Vec<u8>, VMError> {
    let frame = QuantityValueV1::new(value.clone())
        .encode_frame()
        .map_err(map_frame_error)?;
    encode_envelope(PointerType::Quantity, &frame)
}

/// Strictly decode an integer from a complete pointer envelope snapshot.
pub fn decode_int_bytes(envelope: &[u8]) -> Result<BigInt, VMError> {
    let frame = decode_envelope_bytes(envelope, PointerType::Int, MAX_INT_FRAME_BYTES_V1)?;
    IntValueV1::decode_frame(frame)
        .map(IntValueV1::into_int)
        .map_err(map_frame_error)
}

/// Strictly decode a decimal from a complete pointer envelope snapshot.
pub fn decode_decimal_bytes(envelope: &[u8]) -> Result<Numeric, VMError> {
    let frame = decode_envelope_bytes(
        envelope,
        PointerType::Decimal,
        MAX_DECIMAL_FRAME_BYTES_V1,
    )?;
    DecimalValueV1::decode_frame(frame)
        .map(DecimalValueV1::into_numeric)
        .map_err(map_frame_error)
}

/// Strictly decode a quantity from a complete pointer envelope snapshot.
pub fn decode_quantity_bytes(envelope: &[u8]) -> Result<Quantity, VMError> {
    let frame = decode_envelope_bytes(
        envelope,
        PointerType::Quantity,
        MAX_QUANTITY_FRAME_BYTES_V1,
    )?;
    QuantityValueV1::decode_frame(frame)
        .map(QuantityValueV1::into_quantity)
        .map_err(map_frame_error)
}

/// Strictly decode a staged integer operand.
pub fn decode_int_metered(vm: &mut IVM, pointer: u64) -> Result<BigInt, VMError> {
    let snapshot = snapshot_metered(vm, pointer, PointerType::Int, MAX_INT_FRAME_BYTES_V1)?;
    decode_int_bytes(&snapshot)
}

/// Strictly decode a staged decimal operand.
pub fn decode_decimal_metered(vm: &mut IVM, pointer: u64) -> Result<Numeric, VMError> {
    let snapshot = snapshot_metered(
        vm,
        pointer,
        PointerType::Decimal,
        MAX_DECIMAL_FRAME_BYTES_V1,
    )?;
    decode_decimal_bytes(&snapshot)
}

/// Strictly decode a staged quantity operand.
pub fn decode_quantity_metered(vm: &mut IVM, pointer: u64) -> Result<Quantity, VMError> {
    let snapshot = snapshot_metered(
        vm,
        pointer,
        PointerType::Quantity,
        MAX_QUANTITY_FRAME_BYTES_V1,
    )?;
    decode_quantity_bytes(&snapshot)
}

/// Debit, serialize, and allocate a staged integer result.
pub fn allocate_int_metered(vm: &mut IVM, value: &BigInt) -> Result<u64, VMError> {
    let envelope_len = exact_envelope_len(exact_int_frame_len(value)?)?;
    charge_output(vm, envelope_len)?;
    let envelope = encode_int(value)?;
    debug_assert_eq!(envelope.len(), envelope_len);
    vm.alloc_host_tlv(&envelope)
}

/// Debit, serialize, and allocate a staged decimal result.
pub fn allocate_decimal_metered(vm: &mut IVM, value: &Numeric) -> Result<u64, VMError> {
    let envelope_len = exact_envelope_len(exact_scaled_frame_len(value)?)?;
    charge_output(vm, envelope_len)?;
    let envelope = encode_decimal(value)?;
    debug_assert_eq!(envelope.len(), envelope_len);
    vm.alloc_host_tlv(&envelope)
}

/// Debit, serialize, and allocate a staged quantity result.
pub fn allocate_quantity_metered(vm: &mut IVM, value: &Quantity) -> Result<u64, VMError> {
    let envelope_len = exact_envelope_len(exact_scaled_frame_len(value.as_numeric())?)?;
    charge_output(vm, envelope_len)?;
    let envelope = encode_quantity(value)?;
    debug_assert_eq!(envelope.len(), envelope_len);
    vm.alloc_host_tlv(&envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_three_envelopes_roundtrip_and_cross_types_fail() {
        let integer = BigInt::from_i128(-129);
        let decimal = Numeric::new(-125, 2);
        let quantity: Quantity = "1.25".parse().expect("quantity");

        let int_envelope = encode_int(&integer).expect("int envelope");
        let decimal_envelope = encode_decimal(&decimal).expect("decimal envelope");
        let quantity_envelope = encode_quantity(&quantity).expect("quantity envelope");
        assert_eq!(decode_int_bytes(&int_envelope), Ok(integer));
        assert_eq!(decode_decimal_bytes(&decimal_envelope), Ok(decimal));
        assert_eq!(decode_quantity_bytes(&quantity_envelope), Ok(quantity));
        assert!(matches!(
            decode_decimal_bytes(&int_envelope),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::WrongType))
        ));
    }

    #[test]
    fn outer_envelope_attacks_have_stable_precedence() {
        let envelope = encode_int(&BigInt::one()).expect("envelope");

        let mut unknown = envelope.clone();
        unknown[..2].copy_from_slice(&0xffff_u16.to_be_bytes());
        assert!(matches!(
            decode_int_bytes(&unknown),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::UnknownType))
        ));

        let mut retired = envelope.clone();
        retired[..2].copy_from_slice(&(PointerType::RetiredAmount as u16).to_be_bytes());
        assert!(matches!(
            decode_int_bytes(&retired),
            Err(VMError::PointerAbiFault(PointerAbiFaultV1::TypeNotAllowed))
        ));

        let mut bad_version = envelope.clone();
        bad_version[2] = 2;
        assert!(matches!(
            decode_int_bytes(&bad_version),
            Err(VMError::PointerAbiFault(
                PointerAbiFaultV1::InvalidEnvelopeVersion
            ))
        ));

        let mut bad_hash = envelope;
        let last = bad_hash.len() - 1;
        bad_hash[last] ^= 1;
        assert!(matches!(
            decode_int_bytes(&bad_hash),
            Err(VMError::PointerAbiFault(
                PointerAbiFaultV1::PayloadHashMismatch
            ))
        ));
    }

    #[test]
    fn exact_lengths_cover_empty_zero_and_signed_boundaries() {
        let zero = encode_int(&BigInt::zero()).expect("zero");
        assert_eq!(zero.len(), NUMERIC_POINTER_ENVELOPE_OVERHEAD_V1 + 44);
        for value in [127_i128, 128, -128, -129] {
            let value = BigInt::from_i128(value);
            assert_eq!(
                encode_int(&value).expect("boundary envelope").len(),
                exact_envelope_len(exact_int_frame_len(&value).expect("frame length"))
                    .expect("envelope length")
            );
        }
    }
}
