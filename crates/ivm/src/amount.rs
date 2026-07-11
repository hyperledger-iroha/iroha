//! Deterministic VM-local implementation of Kotodama `Amount` syscalls.

use iroha_crypto::Hash;
use iroha_primitives::{AmountRoundingMode, Numeric};
use norito::{decode_from_bytes, to_bytes};

use crate::{
    IVM, PointerType, VMError,
    host::{preflight_reserved_syscall_gas, quote_tlv_payload_len_at},
    syscalls::{self, *},
};

const AMOUNT_GAS_BASE: u64 = 16;
const AMOUNT_GAS_PER_LIMB: u64 = 4;
const AMOUNT_GAS_PER_BYTE: u64 = 1;
const AMOUNT_EXACT_DIVISION_ATTEMPTS: u64 = 29;
// A canonical nonnegative 512-bit Numeric needs at most 65 signed-magnitude
// bytes. Its fixed V1 Norito envelope and scale field bring the complete
// payload to exactly 115 bytes at that boundary. Keep the proven bound here so
// prepare never constructs an arithmetic result merely to quote its length.
const AMOUNT_MAX_PAYLOAD_BYTES: usize = 115;
// Gas preparation may inspect only the pointer-ABI envelope shape. These
// bounds follow from Amount's public 512-bit mantissa and scale <= 28
// invariants, so quoting never needs to deserialize attacker-controlled
// Numeric payloads before the VM debits gas.
const AMOUNT_MAX_LIMBS: u64 = 8;
const AMOUNT_MAX_ALIGNED_LIMBS: u64 = 10;
const AMOUNT_MAX_MULTIPLICATION_WORK: u64 = AMOUNT_MAX_LIMBS * AMOUNT_MAX_LIMBS;
const AMOUNT_MAX_DIVISION_WORK: u64 = 12 * 10;

fn limb_count(value: &Numeric) -> u64 {
    let bits = u64::try_from(value.mantissa().bit_len()).unwrap_or(u64::MAX);
    (bits.saturating_add(63) / 64).max(1)
}

fn scaled_limb_count(value: &Numeric, decimal_places: u32) -> u64 {
    // 10^n is strictly smaller than 2^(4n). This integer-only bound is stable
    // across hosts and independent of num-bigint's platform limb width.
    let extra_bits = u64::from(decimal_places).saturating_mul(4);
    let bits = u64::try_from(value.mantissa().bit_len())
        .unwrap_or(u64::MAX)
        .saturating_add(extra_bits);
    (bits.saturating_add(63) / 64).max(1)
}

fn amount_gas(limb_work: u64, encoded_bytes: usize) -> u64 {
    AMOUNT_GAS_BASE
        .saturating_add(AMOUNT_GAS_PER_LIMB.saturating_mul(limb_work.max(1)))
        .saturating_add(
            AMOUNT_GAS_PER_BYTE.saturating_mul(u64::try_from(encoded_bytes).unwrap_or(u64::MAX)),
        )
}

fn aligned_work(lhs: &Numeric, rhs: &Numeric) -> u64 {
    let target_scale = lhs.scale().max(rhs.scale());
    scaled_limb_count(lhs, target_scale - lhs.scale())
        .max(scaled_limb_count(rhs, target_scale - rhs.scale()))
}

fn multiplication_work(lhs: &Numeric, rhs: &Numeric) -> u64 {
    limb_count(lhs).saturating_mul(limb_count(rhs))
}

fn division_work(lhs: &Numeric, rhs: &Numeric, output_scale: u32, attempts: u64) -> u64 {
    let numerator_limbs = scaled_limb_count(lhs, rhs.scale().saturating_add(output_scale));
    let denominator_limbs = scaled_limb_count(rhs, lhs.scale());
    numerator_limbs
        .saturating_mul(denominator_limbs)
        .saturating_mul(attempts)
}

fn decode_typed_numeric(vm: &IVM, pointer: u64, expected: PointerType) -> Result<Numeric, VMError> {
    if pointer == 0 {
        return Err(VMError::NoritoInvalid);
    }
    let tlv = vm.validate_tlv(pointer)?;
    if tlv.type_id != expected {
        return Err(VMError::NoritoInvalid);
    }
    let value: Numeric = decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
    if to_bytes(&value).map_err(|_| VMError::DecodeError)? != tlv.payload {
        return Err(VMError::DecodeError);
    }
    Ok(value)
}

fn decode_amount(vm: &IVM, pointer: u64) -> Result<Numeric, VMError> {
    let value = decode_typed_numeric(vm, pointer, PointerType::Amount)?;
    value.validate_amount().map_err(|_| VMError::DecodeError)?;
    Ok(value)
}

fn decode_u128(vm: &IVM, pointer: u64) -> Result<Numeric, VMError> {
    let value = decode_typed_numeric(vm, pointer, PointerType::NoritoBytes)?;
    if value.scale() != 0 || value.try_mantissa_u128().is_none() {
        return Err(VMError::DecodeError);
    }
    Ok(value)
}

fn quote_numeric_operand(vm: &IVM, pointer: u64, expected: PointerType) -> Result<(), VMError> {
    if pointer == 0 {
        return Err(VMError::NoritoInvalid);
    }
    let payload_len = quote_tlv_payload_len_at(vm, pointer, expected)?;
    if payload_len > AMOUNT_MAX_PAYLOAD_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    Ok(())
}

fn encode_tlv(pointer_type: PointerType, payload: &[u8]) -> Result<Vec<u8>, VMError> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&payload_len.to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    Ok(out)
}

fn return_amount(vm: &mut IVM, value: &Numeric, limb_work: u64) -> Result<u64, VMError> {
    value
        .validate_amount()
        .map_err(|_| VMError::AssertionFailed)?;
    let payload = to_bytes(value).map_err(|_| VMError::NoritoInvalid)?;
    let actual_gas = amount_gas(limb_work, payload.len());
    preflight_reserved_syscall_gas(vm, actual_gas)?;
    let pointer = vm.alloc_host_tlv(&encode_tlv(PointerType::Amount, &payload)?)?;
    vm.set_register(10, pointer);
    Ok(actual_gas)
}

fn return_comparison(vm: &mut IVM, limb_work: u64, value: bool) -> Result<u64, VMError> {
    let actual_gas = amount_gas(limb_work, 0);
    preflight_reserved_syscall_gas(vm, actual_gas)?;
    vm.set_register(10, u64::from(value));
    Ok(actual_gas)
}

/// Execute one syscall from the exact V1 Amount family.
pub(crate) fn execute(number: u32, vm: &mut IVM) -> Result<u64, VMError> {
    match number {
        SYSCALL_AMOUNT_FROM_I64 => {
            let value = vm.register(10) as i64;
            if value < 0 {
                return Err(VMError::AssertionFailed);
            }
            let amount = Numeric::from(value)
                .canonicalize_amount()
                .map_err(|_| VMError::AssertionFailed)?;
            return_amount(vm, &amount, 1)
        }
        SYSCALL_AMOUNT_FROM_U128 => {
            let numeric = decode_u128(vm, vm.register(10))?;
            let amount = numeric
                .clone()
                .canonicalize_amount()
                .map_err(|_| VMError::AssertionFailed)?;
            return_amount(vm, &amount, limb_count(&numeric))
        }
        SYSCALL_AMOUNT_TO_I64 => {
            let amount = decode_amount(vm, vm.register(10))?;
            if amount.scale() != 0 {
                return Err(VMError::AssertionFailed);
            }
            let value = amount
                .try_mantissa_i128()
                .and_then(|value| i64::try_from(value).ok())
                .ok_or(VMError::AssertionFailed)?;
            let actual_gas = amount_gas(limb_count(&amount), 0);
            preflight_reserved_syscall_gas(vm, actual_gas)?;
            vm.set_register(10, value as u64);
            Ok(actual_gas)
        }
        SYSCALL_AMOUNT_ADD | SYSCALL_AMOUNT_SUB | SYSCALL_AMOUNT_MUL | SYSCALL_AMOUNT_DIV_EXACT => {
            let lhs = decode_amount(vm, vm.register(10))?;
            let rhs = decode_amount(vm, vm.register(11))?;
            let limb_work = match number {
                SYSCALL_AMOUNT_ADD | SYSCALL_AMOUNT_SUB => aligned_work(&lhs, &rhs),
                SYSCALL_AMOUNT_MUL => multiplication_work(&lhs, &rhs),
                SYSCALL_AMOUNT_DIV_EXACT => {
                    division_work(&lhs, &rhs, 28, AMOUNT_EXACT_DIVISION_ATTEMPTS)
                }
                _ => unreachable!("amount arithmetic family is exhaustive"),
            };
            let result = match number {
                SYSCALL_AMOUNT_ADD => lhs.checked_amount_add(&rhs),
                SYSCALL_AMOUNT_SUB => lhs.checked_amount_sub(&rhs),
                SYSCALL_AMOUNT_MUL => lhs.checked_amount_mul(&rhs),
                SYSCALL_AMOUNT_DIV_EXACT => lhs.checked_amount_div_exact(&rhs),
                _ => unreachable!("amount arithmetic family is exhaustive"),
            }
            .map_err(|_| VMError::AssertionFailed)?;
            return_amount(vm, &result, limb_work)
        }
        SYSCALL_AMOUNT_DIV_ROUND => {
            let lhs = decode_amount(vm, vm.register(10))?;
            let rhs = decode_amount(vm, vm.register(11))?;
            let scale = u32::try_from(vm.register(12)).map_err(|_| VMError::AssertionFailed)?;
            let mode = match vm.register(13) {
                AMOUNT_ROUND_FLOOR => AmountRoundingMode::Floor,
                AMOUNT_ROUND_CEIL => AmountRoundingMode::Ceil,
                AMOUNT_ROUND_NEAREST_EVEN => AmountRoundingMode::NearestEven,
                _ => return Err(VMError::AssertionFailed),
            };
            let result = lhs
                .checked_amount_div_round(&rhs, scale, mode)
                .map_err(|_| VMError::AssertionFailed)?;
            let limb_work = division_work(&lhs, &rhs, scale, 1);
            return_amount(vm, &result, limb_work)
        }
        SYSCALL_AMOUNT_EQ | SYSCALL_AMOUNT_NE | SYSCALL_AMOUNT_LT | SYSCALL_AMOUNT_LE
        | SYSCALL_AMOUNT_GT | SYSCALL_AMOUNT_GE => {
            let lhs = decode_amount(vm, vm.register(10))?;
            let rhs = decode_amount(vm, vm.register(11))?;
            let ordering = lhs.cmp(&rhs);
            let result = match number {
                SYSCALL_AMOUNT_EQ => ordering.is_eq(),
                SYSCALL_AMOUNT_NE => ordering.is_ne(),
                SYSCALL_AMOUNT_LT => ordering.is_lt(),
                SYSCALL_AMOUNT_LE => ordering.is_le(),
                SYSCALL_AMOUNT_GT => ordering.is_gt(),
                SYSCALL_AMOUNT_GE => ordering.is_ge(),
                _ => unreachable!("amount comparison family is exhaustive"),
            };
            return_comparison(vm, aligned_work(&lhs, &rhs), result)
        }
        _ => Err(VMError::UnknownSyscall(number)),
    }
}

/// Return the deterministic upper-bound quote used before Amount execution.
pub(crate) fn gas_quote(number: u32, vm: &IVM) -> Result<Option<u64>, VMError> {
    if !syscalls::is_amount_syscall(number) {
        return Ok(None);
    }
    let quote = match number {
        SYSCALL_AMOUNT_FROM_I64 => {
            if (vm.register(10) as i64) < 0 {
                return Err(VMError::AssertionFailed);
            }
            amount_gas(1, AMOUNT_MAX_PAYLOAD_BYTES)
        }
        SYSCALL_AMOUNT_FROM_U128 => {
            quote_numeric_operand(vm, vm.register(10), PointerType::NoritoBytes)?;
            amount_gas(2, AMOUNT_MAX_PAYLOAD_BYTES)
        }
        SYSCALL_AMOUNT_TO_I64 => {
            quote_numeric_operand(vm, vm.register(10), PointerType::Amount)?;
            amount_gas(AMOUNT_MAX_LIMBS, 0)
        }
        SYSCALL_AMOUNT_ADD | SYSCALL_AMOUNT_SUB => {
            quote_numeric_operand(vm, vm.register(10), PointerType::Amount)?;
            quote_numeric_operand(vm, vm.register(11), PointerType::Amount)?;
            amount_gas(AMOUNT_MAX_ALIGNED_LIMBS, AMOUNT_MAX_PAYLOAD_BYTES)
        }
        SYSCALL_AMOUNT_MUL => {
            quote_numeric_operand(vm, vm.register(10), PointerType::Amount)?;
            quote_numeric_operand(vm, vm.register(11), PointerType::Amount)?;
            amount_gas(AMOUNT_MAX_MULTIPLICATION_WORK, AMOUNT_MAX_PAYLOAD_BYTES)
        }
        SYSCALL_AMOUNT_DIV_EXACT => {
            quote_numeric_operand(vm, vm.register(10), PointerType::Amount)?;
            quote_numeric_operand(vm, vm.register(11), PointerType::Amount)?;
            amount_gas(
                AMOUNT_MAX_DIVISION_WORK.saturating_mul(AMOUNT_EXACT_DIVISION_ATTEMPTS),
                AMOUNT_MAX_PAYLOAD_BYTES,
            )
        }
        SYSCALL_AMOUNT_DIV_ROUND => {
            quote_numeric_operand(vm, vm.register(10), PointerType::Amount)?;
            quote_numeric_operand(vm, vm.register(11), PointerType::Amount)?;
            let scale = u32::try_from(vm.register(12)).map_err(|_| VMError::AssertionFailed)?;
            if scale > 28 || vm.register(13) > AMOUNT_ROUND_NEAREST_EVEN {
                return Err(VMError::AssertionFailed);
            }
            amount_gas(AMOUNT_MAX_DIVISION_WORK, AMOUNT_MAX_PAYLOAD_BYTES)
        }
        SYSCALL_AMOUNT_EQ | SYSCALL_AMOUNT_NE | SYSCALL_AMOUNT_LT | SYSCALL_AMOUNT_LE
        | SYSCALL_AMOUNT_GT | SYSCALL_AMOUNT_GE => {
            quote_numeric_operand(vm, vm.register(10), PointerType::Amount)?;
            quote_numeric_operand(vm, vm.register(11), PointerType::Amount)?;
            amount_gas(AMOUNT_MAX_ALIGNED_LIMBS, 0)
        }
        _ => unreachable!("amount syscall family is exhaustive"),
    };
    Ok(Some(quote))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_numeric(vm: &mut IVM, pointer_type: PointerType, value: &Numeric) -> u64 {
        let payload = to_bytes(value).expect("encode numeric");
        vm.alloc_input_tlv(&encode_tlv(pointer_type, &payload).expect("encode TLV"))
            .expect("allocate input TLV")
    }

    fn output_amount(vm: &IVM) -> Numeric {
        let tlv = vm.validate_tlv(vm.register(10)).expect("validate output");
        assert_eq!(tlv.type_id, PointerType::Amount);
        let value: Numeric = decode_from_bytes(tlv.payload).expect("decode output amount");
        value.validate_amount().expect("canonical output amount");
        value
    }

    #[test]
    fn exact_and_rounded_division_are_distinct() {
        let mut vm = IVM::new(1_000_000);
        let one = input_numeric(&mut vm, PointerType::Amount, &Numeric::from(1_u64));
        let three = input_numeric(&mut vm, PointerType::Amount, &Numeric::from(3_u64));
        vm.set_register(10, one);
        vm.set_register(11, three);
        assert_eq!(
            execute(SYSCALL_AMOUNT_DIV_EXACT, &mut vm),
            Err(VMError::AssertionFailed)
        );

        vm.set_register(10, one);
        vm.set_register(11, three);
        vm.set_register(12, 2);
        vm.set_register(13, AMOUNT_ROUND_NEAREST_EVEN);
        execute(SYSCALL_AMOUNT_DIV_ROUND, &mut vm).expect("rounded division");
        assert_eq!(output_amount(&vm), Numeric::new(33, 2));
    }

    #[test]
    fn amount_syscalls_reject_wrong_and_noncanonical_pointer_payloads() {
        let mut vm = IVM::new(1_000_000);
        let wrong = input_numeric(&mut vm, PointerType::NoritoBytes, &Numeric::from(1_u64));
        vm.set_register(10, wrong);
        vm.set_register(11, wrong);
        assert_eq!(
            execute(SYSCALL_AMOUNT_ADD, &mut vm),
            Err(VMError::NoritoInvalid)
        );

        let noncanonical = input_numeric(&mut vm, PointerType::Amount, &Numeric::new(10, 1));
        vm.set_register(10, noncanonical);
        vm.set_register(11, noncanonical);
        assert_eq!(
            execute(SYSCALL_AMOUNT_ADD, &mut vm),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn rounded_division_rejects_invalid_scale_mode_and_zero_divisor() {
        let mut vm = IVM::new(1_000_000);
        let one = input_numeric(&mut vm, PointerType::Amount, &Numeric::from(1_u64));
        let zero = input_numeric(&mut vm, PointerType::Amount, &Numeric::zero());

        vm.set_register(10, one);
        vm.set_register(11, one);
        vm.set_register(12, 29);
        vm.set_register(13, AMOUNT_ROUND_FLOOR);
        assert_eq!(
            execute(SYSCALL_AMOUNT_DIV_ROUND, &mut vm),
            Err(VMError::AssertionFailed)
        );

        vm.set_register(10, one);
        vm.set_register(11, one);
        vm.set_register(12, 2);
        vm.set_register(13, 3);
        assert_eq!(
            execute(SYSCALL_AMOUNT_DIV_ROUND, &mut vm),
            Err(VMError::AssertionFailed)
        );

        vm.set_register(10, one);
        vm.set_register(11, zero);
        vm.set_register(12, 2);
        vm.set_register(13, AMOUNT_ROUND_FLOOR);
        assert_eq!(
            execute(SYSCALL_AMOUNT_DIV_ROUND, &mut vm),
            Err(VMError::AssertionFailed)
        );
    }

    #[test]
    fn conversions_and_underflow_fail_closed() {
        let mut vm = IVM::new(1_000_000);
        vm.set_register(10, (-1_i64) as u64);
        assert_eq!(
            execute(SYSCALL_AMOUNT_FROM_I64, &mut vm),
            Err(VMError::AssertionFailed)
        );

        let one = input_numeric(&mut vm, PointerType::Amount, &Numeric::from(1_u64));
        let two = input_numeric(&mut vm, PointerType::Amount, &Numeric::from(2_u64));
        vm.set_register(10, one);
        vm.set_register(11, two);
        assert_eq!(
            execute(SYSCALL_AMOUNT_SUB, &mut vm),
            Err(VMError::AssertionFailed)
        );

        let u128_value = input_numeric(
            &mut vm,
            PointerType::NoritoBytes,
            &Numeric::new(u128::MAX, 0),
        );
        vm.set_register(10, u128_value);
        execute(SYSCALL_AMOUNT_FROM_U128, &mut vm).expect("u128 to Amount");
        assert_eq!(output_amount(&vm), Numeric::new(u128::MAX, 0));
    }

    #[test]
    fn gas_scales_with_mantissa_limb_work() {
        let small = Numeric::from(1_u64);
        let large = Numeric::new(u128::MAX, 0);
        assert!(aligned_work(&large, &large) > aligned_work(&small, &small));
        assert!(multiplication_work(&large, &large) > aligned_work(&large, &large));
        assert!(
            division_work(&large, &small, 28, AMOUNT_EXACT_DIVISION_ATTEMPTS)
                > multiplication_work(&large, &large)
        );
        assert!(amount_gas(2, 10) > amount_gas(2, 0));
    }

    #[test]
    fn gas_quote_checks_only_bounded_operand_shapes() {
        let mut vm = IVM::new(1_000_000);
        let malformed = vm
            .alloc_input_tlv(
                &encode_tlv(PointerType::Amount, b"not a canonical Numeric")
                    .expect("encode malformed Amount envelope"),
            )
            .expect("allocate malformed Amount envelope");
        vm.set_register(10, malformed);
        vm.set_register(11, malformed);

        let quote = gas_quote(SYSCALL_AMOUNT_ADD, &vm)
            .expect("shape-valid operands receive a conservative quote")
            .expect("Amount syscall has a quote");
        assert_eq!(
            quote,
            amount_gas(AMOUNT_MAX_ALIGNED_LIMBS, AMOUNT_MAX_PAYLOAD_BYTES)
        );
        assert_eq!(
            execute(SYSCALL_AMOUNT_ADD, &mut vm),
            Err(VMError::DecodeError),
            "canonical Numeric decoding belongs to post-debit execution"
        );
    }

    #[test]
    fn shape_only_quote_bounds_valid_amount_execution() {
        let mut vm = IVM::new(1_000_000);
        let lhs = input_numeric(&mut vm, PointerType::Amount, &Numeric::new(125_u32, 2));
        let rhs = input_numeric(&mut vm, PointerType::Amount, &Numeric::new(75_u32, 2));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);

        let quote = gas_quote(SYSCALL_AMOUNT_ADD, &vm)
            .expect("quote Amount addition")
            .expect("Amount syscall has a quote");
        let actual = execute(SYSCALL_AMOUNT_ADD, &mut vm).expect("execute Amount addition");
        assert!(actual <= quote);
        assert_eq!(output_amount(&vm), Numeric::new(2_u32, 0));
    }

    #[test]
    fn gas_quote_rejects_oversized_amount_envelopes_without_decoding() {
        let mut vm = IVM::new(1_000_000);
        let oversized_payload = vec![0_u8; AMOUNT_MAX_PAYLOAD_BYTES + 1];
        let oversized = vm
            .alloc_input_tlv(
                &encode_tlv(PointerType::Amount, &oversized_payload)
                    .expect("encode oversized Amount envelope"),
            )
            .expect("allocate oversized Amount envelope");
        vm.set_register(10, oversized);
        vm.set_register(11, oversized);
        assert_eq!(
            gas_quote(SYSCALL_AMOUNT_ADD, &vm),
            Err(VMError::NoritoInvalid)
        );
    }

    #[test]
    fn unaffordable_malformed_amount_never_authenticates_or_decodes_before_debit() {
        let mut vm = IVM::new(u64::MAX);
        let mut envelope = encode_tlv(PointerType::Amount, b"not a canonical Numeric")
            .expect("encode malformed Amount envelope");
        *envelope.last_mut().expect("envelope has a digest") ^= 1;
        let malformed = vm
            .alloc_input_tlv(&envelope)
            .expect("allocate malformed Amount envelope");
        vm.set_register(10, malformed);
        vm.set_register(11, malformed);
        let quote = gas_quote(SYSCALL_AMOUNT_ADD, &vm)
            .expect("shape-valid operands receive a conservative quote")
            .expect("Amount syscall has a quote");

        let mut program = crate::metadata::ProgramMetadata::default().encode();
        program.extend_from_slice(
            &crate::encoding::wide::encode_syscallx(SYSCALL_AMOUNT_ADD).to_le_bytes(),
        );
        program.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        vm.load_program(&program).expect("load Amount syscall");
        vm.set_register(10, malformed);
        vm.set_register(11, malformed);
        vm.set_host(crate::host::DefaultHost::new());
        vm.set_gas_limit(5_u64.saturating_add(quote).saturating_sub(1));

        assert_eq!(
            vm.run(),
            Err(VMError::OutOfGas),
            "gas debit must reject the call before digest authentication or Numeric decoding"
        );
        assert_eq!(vm.register(10), malformed);
        assert_eq!(vm.register(11), malformed);
    }

    #[test]
    fn shape_only_work_bounds_cover_maximum_amounts() {
        let mut bytes = vec![0xff_u8; 64];
        bytes.push(0);
        let mantissa =
            iroha_primitives::BigInt::from_twos_bytes(&bytes).expect("512-bit magnitude");
        let max_scale = Numeric::new(mantissa.clone(), 28);
        let zero_scale = Numeric::new(mantissa, 0);

        assert!(limb_count(&zero_scale) <= AMOUNT_MAX_LIMBS);
        assert!(aligned_work(&max_scale, &zero_scale) <= AMOUNT_MAX_ALIGNED_LIMBS);
        assert!(multiplication_work(&max_scale, &zero_scale) <= AMOUNT_MAX_MULTIPLICATION_WORK);
        assert!(division_work(&max_scale, &max_scale, 28, 1) <= AMOUNT_MAX_DIVISION_WORK);
    }

    #[test]
    fn maximum_amount_payload_fits_the_quote_bound() {
        let mut bytes = vec![0xff_u8; 64];
        bytes.push(0);
        let value = Numeric::new(
            iroha_primitives::BigInt::from_twos_bytes(&bytes).expect("512-bit magnitude"),
            0,
        );
        value
            .validate_amount()
            .expect("maximum 512-bit Amount is valid");
        assert_eq!(value.mantissa().bit_len(), 512);
        let encoded = to_bytes(&value).expect("encode maximum Amount");
        assert_eq!(encoded.len(), AMOUNT_MAX_PAYLOAD_BYTES);
    }
}
