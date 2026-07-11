//! Consensus and adversarial coverage for the Kotodama V1 numeric syscall ABI.

use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, Quantity},
    numeric_abi::{DecimalValueV1, IntValueV1, QuantityValueV1},
};
use ivm::{
    IVM, Memory, ProgramMetadata, VMError, encoding,
    host::{DefaultHost, IVMHost},
    ivm_mode,
    numeric::{
        NUMERIC_FAILURE_STATUS, NUMERIC_FAILURE_TRAP, NumericFaultV1, PointerAbiFaultV1,
        RoundingModeV1,
    },
    syscall_metering::{SyscallCompletion, SyscallMetering, SyscallMeteringPhase},
    syscalls,
};
use std::collections::{BTreeMap, BTreeSet};

fn program(syscall: u32) -> Vec<u8> {
    let mut program = ProgramMetadata::default_for(1, 0, 1).encode();
    program.extend_from_slice(&encoding::wide::encode_syscallx(syscall).to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    program
}

fn vm_for(syscall: u32, gas: u64) -> IVM {
    let mut vm = IVM::new(gas);
    vm.load_program(&program(syscall))
        .expect("load numeric program");
    vm.set_host(DefaultHost::new());
    vm
}

fn zk_vm_for(syscall: u32, gas: u64) -> IVM {
    let mut metadata = ProgramMetadata::default_for(1, 0, 1);
    metadata.mode = ivm_mode::ZK;
    let mut program = metadata.encode();
    program.extend_from_slice(&encoding::wide::encode_syscallx(syscall).to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(gas);
    vm.load_program(&program).expect("load ZK numeric program");
    vm.set_host(DefaultHost::new());
    vm
}

fn install_int(vm: &mut IVM, value: &BigInt) -> u64 {
    let envelope = ivm::numeric_tlv::encode_int(value).expect("encode int envelope");
    vm.alloc_host_tlv(&envelope).expect("install int")
}

fn install_decimal(vm: &mut IVM, value: &Numeric) -> u64 {
    let envelope = ivm::numeric_tlv::encode_decimal(value).expect("encode decimal envelope");
    vm.alloc_host_tlv(&envelope).expect("install decimal")
}

fn install_quantity(vm: &mut IVM, value: &Quantity) -> u64 {
    let envelope = ivm::numeric_tlv::encode_quantity(value).expect("encode quantity envelope");
    vm.alloc_host_tlv(&envelope).expect("install quantity")
}

fn numeric_envelope_from_frame(pointer_type: ivm::PointerType, frame: &[u8]) -> Vec<u8> {
    let mut envelope = Vec::with_capacity(7 + frame.len() + iroha_crypto::Hash::LENGTH);
    envelope.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    envelope.push(1);
    envelope.extend_from_slice(
        &u32::try_from(frame.len())
            .expect("bounded numeric frame")
            .to_be_bytes(),
    );
    envelope.extend_from_slice(frame);
    envelope.extend_from_slice(iroha_crypto::Hash::new(frame).as_ref());
    envelope
}

fn result_int(vm: &IVM) -> BigInt {
    IntValueV1::decode_frame(
        vm.validate_tlv(vm.register(10))
            .expect("result int TLV")
            .payload,
    )
    .expect("result int frame")
    .into_int()
}

fn result_decimal(vm: &IVM) -> Numeric {
    DecimalValueV1::decode_frame(
        vm.validate_tlv(vm.register(10))
            .expect("result decimal TLV")
            .payload,
    )
    .expect("result decimal frame")
    .into_numeric()
}

fn result_quantity(vm: &IVM) -> Quantity {
    QuantityValueV1::decode_frame(
        vm.validate_tlv(vm.register(10))
            .expect("result quantity TLV")
            .payload,
    )
    .expect("result quantity frame")
    .into_quantity()
}

fn run_int_binary(syscall: u32, lhs: i128, rhs: i128) -> IVM {
    let mut vm = vm_for(syscall, u64::MAX);
    let lhs = install_int(&mut vm, &BigInt::from_i128(lhs));
    let rhs = install_int(&mut vm, &BigInt::from_i128(rhs));
    vm.set_register(10, lhs);
    vm.set_register(11, rhs);
    vm.set_register(14, NUMERIC_FAILURE_TRAP);
    vm.run().expect("execute int binary syscall");
    vm
}

fn run_decimal_binary(syscall: u32, lhs: &str, rhs: &str) -> IVM {
    let mut vm = vm_for(syscall, u64::MAX);
    let lhs = install_decimal(&mut vm, &lhs.parse().expect("lhs decimal"));
    let rhs = install_decimal(&mut vm, &rhs.parse().expect("rhs decimal"));
    vm.set_register(10, lhs);
    vm.set_register(11, rhs);
    vm.set_register(14, NUMERIC_FAILURE_TRAP);
    vm.run().expect("execute decimal binary syscall");
    vm
}

fn run_quantity_binary(syscall: u32, lhs: &str, rhs: &str) -> IVM {
    let mut vm = vm_for(syscall, u64::MAX);
    let lhs = install_quantity(&mut vm, &lhs.parse().expect("lhs quantity"));
    let rhs = install_quantity(&mut vm, &rhs.parse().expect("rhs quantity"));
    vm.set_register(10, lhs);
    vm.set_register(11, rhs);
    vm.set_register(14, NUMERIC_FAILURE_TRAP);
    vm.run().expect("execute quantity binary syscall");
    vm
}

fn max_int() -> BigInt {
    let mut bytes = vec![0xff; iroha_primitives::numeric::MAX_MANTISSA_BYTES];
    bytes[iroha_primitives::numeric::MAX_MANTISSA_BYTES - 1] = 0x7f;
    BigInt::from_twos_bytes(&bytes).expect("maximum int")
}

fn min_int() -> BigInt {
    let mut bytes = vec![0; iroha_primitives::numeric::MAX_MANTISSA_BYTES];
    bytes[iroha_primitives::numeric::MAX_MANTISSA_BYTES - 1] = 0x80;
    BigInt::from_twos_bytes(&bytes).expect("minimum int")
}

fn positive_int_with_limbs(limbs: usize) -> BigInt {
    let mut bytes = vec![0_u8; limbs * 8];
    bytes[0] = 1;
    *bytes.last_mut().expect("positive limb count") = 0x3f;
    BigInt::from_twos_bytes(&bytes).expect("bounded limb fixture")
}

fn envelope_len(vm: &IVM, pointer: u64) -> u64 {
    let payload = vm.validate_tlv(pointer).expect("numeric TLV").payload.len();
    u64::try_from(payload + 39).expect("bounded envelope")
}

fn frame_len_for_envelope(envelope_bytes: u64) -> u64 {
    envelope_bytes
        .checked_sub(39)
        .expect("numeric envelope includes fixed overhead")
}

fn output_length_work(value: &BigInt) -> u64 {
    let magnitude_bits = u64::try_from(value.bit_len()).expect("bounded bit width");
    ivm::numeric_gas::limbs_for_bits(magnitude_bits + 1)
}

fn validation_work_for_envelope(bytes: u64) -> u64 {
    let frame_bytes = bytes.checked_sub(39).expect("numeric envelope overhead");
    ivm::numeric_gas::numeric_frame_validation_work(
        usize::try_from(frame_bytes).expect("bounded frame length"),
    )
    .expect("bounded validation work")
}

fn decimal_validation_work_for_envelope(bytes: u64, value: &Numeric) -> u64 {
    let mut work = validation_work_for_envelope(bytes);
    if value.scale() > 0 {
        let limbs = u64::try_from(value.mantissa().bit_len())
            .expect("bounded bit width")
            .max(1)
            .div_ceil(64);
        work +=
            ivm::numeric_gas::quotient_remainder_work(limbs, 1).expect("bounded canonicality work");
    }
    work
}

fn collect_oog_prefixes<F>(syscall: u32, setup: F) -> BTreeMap<u8, BTreeSet<u64>>
where
    F: Fn(&mut IVM),
{
    let mut baseline = vm_for(syscall, u64::MAX);
    setup(&mut baseline);
    baseline.run().expect("baseline staged operation");
    let staged = baseline
        .last_staged_syscall_context()
        .expect("baseline staged context")
        .charged();
    let instruction = ivm::cost_of(encoding::wide::encode_syscallx(syscall)).expect("SCALLX gas");
    let mut prefixes = BTreeMap::<u8, BTreeSet<u64>>::new();
    for gas in instruction..=instruction + staged {
        let mut vm = vm_for(syscall, gas);
        setup(&mut vm);
        let original_result_register = vm.register(10);
        if let Err(VMError::SyscallOutOfGas {
            syscall: failed,
            phase,
        }) = vm.run()
        {
            assert_eq!(failed, syscall);
            assert_eq!(vm.register(10), original_result_register);
            let context = vm
                .last_staged_syscall_context()
                .expect("failed staged context");
            assert_eq!(context.completion(), Some(SyscallCompletion::Trap));
            assert_eq!(
                gas - vm.remaining_gas(),
                instruction + context.charged(),
                "only completed phases consume gas",
            );
            prefixes.entry(phase).or_default().insert(context.charged());
        }
    }
    prefixes
}

#[test]
fn abi_contains_exactly_the_52_numeric_calls_and_rejects_retired_numbers() {
    let numbers = (0x01_0100..=0x01_0113)
        .chain(0x01_0120..=0x01_012f)
        .chain(0x01_0140..=0x01_014f)
        .collect::<Vec<_>>();
    assert_eq!(numbers.len(), 52);
    let vm = IVM::new(u64::MAX);
    let host = DefaultHost::new();
    for number in numbers {
        assert!(syscalls::is_numeric_v1_syscall(number));
        let metering = ivm::host::host_syscall_metering_spec(vm.syscall_policy(), number)
            .expect("numeric metering registration");
        assert_eq!(metering.metering, SyscallMetering::Staged);
        assert_eq!(host.prepare_syscall(number, &vm), Ok(0));
    }
    for retired in (0x69..=0x76)
        .chain(0xd2..=0xde)
        .chain(0x01_0040..=0x01_004d)
    {
        assert_eq!(
            host.prepare_syscall(retired, &vm),
            Err(VMError::UnknownSyscall(retired))
        );
    }
}

#[test]
fn every_shipping_numeric_syscall_executes_with_a_semantic_assertion() {
    let expected = (0x01_0100..=0x01_0113)
        .chain(0x01_0120..=0x01_012f)
        .chain(0x01_0140..=0x01_014f)
        .collect::<BTreeSet<_>>();
    let mut covered = BTreeSet::new();

    for (syscall, raw, expected) in [
        (syscalls::SYSCALL_INT_FROM_I64, (-7_i64) as u64, -7_i128),
        (syscalls::SYSCALL_INT_FROM_U64, 7_u64, 7_i128),
    ] {
        let mut vm = vm_for(syscall, u64::MAX);
        vm.set_register(10, raw);
        vm.run().expect("construct int");
        assert_eq!(result_int(&vm), BigInt::from_i128(expected));
        covered.insert(syscall);
    }

    for (syscall, input, expected) in [
        (syscalls::SYSCALL_INT_TRY_TO_I64, -7_i128, (-7_i64) as u64),
        (syscalls::SYSCALL_INT_TRY_TO_U64, 7_i128, 7_u64),
    ] {
        let mut vm = vm_for(syscall, u64::MAX);
        let input = install_int(&mut vm, &BigInt::from_i128(input));
        vm.set_register(10, input);
        vm.run().expect("convert int");
        assert_eq!(vm.register(10), expected);
        assert_eq!(vm.register(11), 0);
        covered.insert(syscall);
    }

    for (syscall, expected) in [
        (syscalls::SYSCALL_INT_NEG, -5_i128),
        (syscalls::SYSCALL_INT_WRAP_NEG, -5_i128),
    ] {
        let mut vm = vm_for(syscall, u64::MAX);
        let input = install_int(&mut vm, &BigInt::from_i128(5));
        vm.set_register(10, input);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
        vm.run().expect("negate int");
        assert_eq!(result_int(&vm), BigInt::from_i128(expected));
        covered.insert(syscall);
    }

    for (syscall, lhs, rhs, expected) in [
        (syscalls::SYSCALL_INT_ADD, 7, 3, 10),
        (syscalls::SYSCALL_INT_SUB, 7, 3, 4),
        (syscalls::SYSCALL_INT_MUL, 7, 3, 21),
        (syscalls::SYSCALL_INT_DIV, 7, 3, 2),
        (syscalls::SYSCALL_INT_REM, 7, 3, 1),
        (syscalls::SYSCALL_INT_WRAP_ADD, 7, 3, 10),
        (syscalls::SYSCALL_INT_WRAP_SUB, 7, 3, 4),
        (syscalls::SYSCALL_INT_WRAP_MUL, 7, 3, 21),
    ] {
        let vm = run_int_binary(syscall, lhs, rhs);
        assert_eq!(result_int(&vm), BigInt::from_i128(expected));
        covered.insert(syscall);
    }

    let comparison_expectations = [false, true, true, true, false, false];
    for (syscall, expected_value) in [
        syscalls::SYSCALL_INT_EQ,
        syscalls::SYSCALL_INT_NE,
        syscalls::SYSCALL_INT_LT,
        syscalls::SYSCALL_INT_LE,
        syscalls::SYSCALL_INT_GT,
        syscalls::SYSCALL_INT_GE,
    ]
    .into_iter()
    .zip(comparison_expectations)
    {
        let vm = run_int_binary(syscall, 1, 2);
        assert_eq!(vm.register(10), u64::from(expected_value));
        covered.insert(syscall);
    }

    let mut from_int = vm_for(syscalls::SYSCALL_DECIMAL_FROM_INT, u64::MAX);
    let integer = install_int(&mut from_int, &BigInt::from_i128(7));
    from_int.set_register(10, integer);
    from_int.run().expect("int to decimal");
    assert_eq!(result_decimal(&from_int).to_string(), "7");
    covered.insert(syscalls::SYSCALL_DECIMAL_FROM_INT);

    let mut decimal_neg = vm_for(syscalls::SYSCALL_DECIMAL_NEG, u64::MAX);
    let value = install_decimal(&mut decimal_neg, &"1.25".parse().expect("decimal"));
    decimal_neg.set_register(10, value);
    decimal_neg.set_register(14, NUMERIC_FAILURE_TRAP);
    decimal_neg.run().expect("decimal negation");
    assert_eq!(result_decimal(&decimal_neg).to_string(), "-1.25");
    covered.insert(syscalls::SYSCALL_DECIMAL_NEG);

    for (syscall, lhs, rhs, expected_value) in [
        (syscalls::SYSCALL_DECIMAL_ADD, "1.25", "0.5", "1.75"),
        (syscalls::SYSCALL_DECIMAL_SUB, "1.25", "0.5", "0.75"),
        (syscalls::SYSCALL_DECIMAL_MUL, "1.25", "0.4", "0.5"),
        (syscalls::SYSCALL_DECIMAL_DIV_EXACT, "6", "4", "1.5"),
    ] {
        let vm = run_decimal_binary(syscall, lhs, rhs);
        assert_eq!(result_decimal(&vm).to_string(), expected_value);
        covered.insert(syscall);
    }

    let mut decimal_round = vm_for(syscalls::SYSCALL_DECIMAL_DIV_ROUND, u64::MAX);
    let lhs = install_decimal(&mut decimal_round, &Numeric::new(1, 0));
    let rhs = install_decimal(&mut decimal_round, &Numeric::new(8, 0));
    let scale = install_int(&mut decimal_round, &BigInt::from_i128(2));
    decimal_round.set_register(10, lhs);
    decimal_round.set_register(11, rhs);
    decimal_round.set_register(12, scale);
    decimal_round.set_register(13, RoundingModeV1::NearestEven.tag());
    decimal_round.set_register(14, NUMERIC_FAILURE_TRAP);
    decimal_round.run().expect("rounded decimal division");
    assert_eq!(result_decimal(&decimal_round).to_string(), "0.12");
    covered.insert(syscalls::SYSCALL_DECIMAL_DIV_ROUND);

    for (syscall, expected_value) in [
        syscalls::SYSCALL_DECIMAL_EQ,
        syscalls::SYSCALL_DECIMAL_NE,
        syscalls::SYSCALL_DECIMAL_LT,
        syscalls::SYSCALL_DECIMAL_LE,
        syscalls::SYSCALL_DECIMAL_GT,
        syscalls::SYSCALL_DECIMAL_GE,
    ]
    .into_iter()
    .zip(comparison_expectations)
    {
        let vm = run_decimal_binary(syscall, "1", "2");
        assert_eq!(vm.register(10), u64::from(expected_value));
        covered.insert(syscall);
    }

    for (syscall, input, mode, expected_value) in [
        (
            syscalls::SYSCALL_DECIMAL_TRY_TO_INT_EXACT,
            "7",
            None,
            7_i128,
        ),
        (syscalls::SYSCALL_DECIMAL_TO_INT_TRUNC, "7.9", None, 7_i128),
        (
            syscalls::SYSCALL_DECIMAL_TO_INT_ROUND,
            "7.5",
            Some(RoundingModeV1::NearestEven),
            8_i128,
        ),
    ] {
        let mut vm = vm_for(syscall, u64::MAX);
        let input = install_decimal(&mut vm, &input.parse().expect("conversion decimal"));
        vm.set_register(10, input);
        if let Some(mode) = mode {
            vm.set_register(13, mode.tag());
        }
        vm.run().expect("decimal to int conversion");
        assert_eq!(result_int(&vm), BigInt::from_i128(expected_value));
        covered.insert(syscall);
    }

    let mut quantity_from_int = vm_for(syscalls::SYSCALL_QUANTITY_TRY_FROM_INT, u64::MAX);
    let int = install_int(&mut quantity_from_int, &BigInt::from_i128(7));
    quantity_from_int.set_register(10, int);
    quantity_from_int.run().expect("int to quantity");
    assert_eq!(result_quantity(&quantity_from_int).to_string(), "7");
    covered.insert(syscalls::SYSCALL_QUANTITY_TRY_FROM_INT);

    let mut quantity_from_decimal = vm_for(syscalls::SYSCALL_QUANTITY_TRY_FROM_DECIMAL, u64::MAX);
    let decimal = install_decimal(
        &mut quantity_from_decimal,
        &"1.25".parse().expect("decimal"),
    );
    quantity_from_decimal.set_register(10, decimal);
    quantity_from_decimal.run().expect("decimal to quantity");
    assert_eq!(result_quantity(&quantity_from_decimal).to_string(), "1.25");
    covered.insert(syscalls::SYSCALL_QUANTITY_TRY_FROM_DECIMAL);

    let mut quantity_to_decimal = vm_for(syscalls::SYSCALL_QUANTITY_TO_DECIMAL, u64::MAX);
    let quantity = install_quantity(&mut quantity_to_decimal, &"1.25".parse().expect("quantity"));
    quantity_to_decimal.set_register(10, quantity);
    quantity_to_decimal.run().expect("quantity to decimal");
    assert_eq!(result_decimal(&quantity_to_decimal).to_string(), "1.25");
    covered.insert(syscalls::SYSCALL_QUANTITY_TO_DECIMAL);

    for (syscall, lhs, rhs, expected_value) in [
        (syscalls::SYSCALL_QUANTITY_ADD, "2", "1", "3"),
        (syscalls::SYSCALL_QUANTITY_SUB, "2", "1", "1"),
        (syscalls::SYSCALL_QUANTITY_RATIO_EXACT, "6", "4", "1.5"),
    ] {
        let vm = run_quantity_binary(syscall, lhs, rhs);
        if syscall == syscalls::SYSCALL_QUANTITY_RATIO_EXACT {
            assert_eq!(result_decimal(&vm).to_string(), expected_value);
        } else {
            assert_eq!(result_quantity(&vm).to_string(), expected_value);
        }
        covered.insert(syscall);
    }

    for (syscall, rhs, expected_value) in [
        (syscalls::SYSCALL_QUANTITY_MUL_DECIMAL, "1.5", "3"),
        (syscalls::SYSCALL_QUANTITY_DIV_DECIMAL_EXACT, "4", "0.5"),
    ] {
        let mut vm = vm_for(syscall, u64::MAX);
        let lhs = install_quantity(&mut vm, &"2".parse().expect("quantity"));
        let rhs = install_decimal(&mut vm, &rhs.parse().expect("decimal"));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
        vm.run().expect("quantity/decimal operation");
        assert_eq!(result_quantity(&vm).to_string(), expected_value);
        covered.insert(syscall);
    }

    for (syscall, ratio) in [
        (syscalls::SYSCALL_QUANTITY_DIV_DECIMAL_ROUND, false),
        (syscalls::SYSCALL_QUANTITY_RATIO_ROUND, true),
    ] {
        let mut vm = vm_for(syscall, u64::MAX);
        let lhs = install_quantity(&mut vm, &"1".parse().expect("quantity"));
        let rhs = if ratio {
            install_quantity(&mut vm, &"8".parse().expect("quantity"))
        } else {
            install_decimal(&mut vm, &"8".parse().expect("decimal"))
        };
        let scale = install_int(&mut vm, &BigInt::from_i128(2));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(12, scale);
        vm.set_register(13, RoundingModeV1::NearestEven.tag());
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
        vm.run().expect("rounded quantity division");
        if ratio {
            assert_eq!(result_decimal(&vm).to_string(), "0.12");
        } else {
            assert_eq!(result_quantity(&vm).to_string(), "0.12");
        }
        covered.insert(syscall);
    }

    for (syscall, expected_value) in [
        syscalls::SYSCALL_QUANTITY_EQ,
        syscalls::SYSCALL_QUANTITY_NE,
        syscalls::SYSCALL_QUANTITY_LT,
        syscalls::SYSCALL_QUANTITY_LE,
        syscalls::SYSCALL_QUANTITY_GT,
        syscalls::SYSCALL_QUANTITY_GE,
    ]
    .into_iter()
    .zip(comparison_expectations)
    {
        let vm = run_quantity_binary(syscall, "1", "2");
        assert_eq!(vm.register(10), u64::from(expected_value));
        covered.insert(syscall);
    }

    assert_eq!(
        covered, expected,
        "every shipping numeric syscall must execute"
    );
}

#[test]
fn actual_staged_contexts_match_the_normative_gas_identity_and_width_boundaries() {
    let mut previous = 0;
    for limbs in 1_usize..=8 {
        let syscall = syscalls::SYSCALL_INT_ADD;
        let mut vm = vm_for(syscall, u64::MAX);
        let lhs_value = positive_int_with_limbs(limbs);
        let lhs_envelope = ivm::numeric_tlv::encode_int(&lhs_value).expect("lhs envelope");
        let rhs_envelope = ivm::numeric_tlv::encode_int(&BigInt::one()).expect("rhs envelope");
        let lhs = vm.alloc_host_tlv(&lhs_envelope).expect("lhs");
        let rhs = vm.alloc_host_tlv(&rhs_envelope).expect("rhs");
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
        vm.run().expect("width-boundary addition");
        let output_value = result_int(&vm);
        let output_bytes = envelope_len(&vm, vm.register(10));
        let input_bytes = u64::try_from(lhs_envelope.len() + rhs_envelope.len()).unwrap();
        let input_hash_bytes = frame_len_for_envelope(lhs_envelope.len() as u64)
            + frame_len_for_envelope(rhs_envelope.len() as u64);
        let output_frame_bytes = frame_len_for_envelope(output_bytes);
        let validation = validation_work_for_envelope(lhs_envelope.len() as u64)
            + validation_work_for_envelope(rhs_envelope.len() as u64);
        let expected = ivm::numeric_gas::successful_call_gas(
            input_bytes,
            input_hash_bytes,
            output_bytes,
            output_frame_bytes,
            output_length_work(&output_value),
            ivm::numeric_gas::checked_int_additive_work(limbs as u64, 1)
                .expect("bounded checked-add work"),
            validation,
            0,
        )
        .expect("bounded gas identity");
        let context = vm.last_staged_syscall_context().expect("staged context");
        assert_eq!(context.charged(), expected, "limbs={limbs}");
        assert_eq!(context.completion(), Some(SyscallCompletion::Success));
        assert!(
            context.charged() > previous,
            "each defined limb transition costs more"
        );
        previous = context.charged();
    }

    let maximum = Numeric::new(max_int(), 0);
    let tiny: Numeric = "0.0000000000000000000000000001"
        .parse()
        .expect("scale-28 decimal");
    let mut compare = vm_for(syscalls::SYSCALL_DECIMAL_GT, u64::MAX);
    let lhs_envelope = ivm::numeric_tlv::encode_decimal(&maximum).expect("maximum decimal");
    let rhs_envelope = ivm::numeric_tlv::encode_decimal(&tiny).expect("tiny decimal");
    let lhs = compare.alloc_host_tlv(&lhs_envelope).expect("lhs");
    let rhs = compare.alloc_host_tlv(&rhs_envelope).expect("rhs");
    compare.set_register(10, lhs);
    compare.set_register(11, rhs);
    compare.run().expect("ten-limb aligned comparison");
    assert_eq!(compare.register(10), 1);
    let lhs_bits = u64::try_from(maximum.mantissa().bit_len()).unwrap();
    let rhs_bits = u64::try_from(tiny.mantissa().bit_len()).unwrap();
    let aligned_lhs = ivm::numeric_gas::scaled_limbs(lhs_bits, 28).expect("aligned lhs");
    let aligned_rhs = ivm::numeric_gas::scaled_limbs(rhs_bits, 0).expect("aligned rhs");
    assert_eq!(aligned_lhs, 10);
    let comparison_work = ivm::numeric_gas::aligned_work(8, 28, aligned_lhs, 1, 0, aligned_rhs)
        .expect("comparison work");
    let validation = decimal_validation_work_for_envelope(lhs_envelope.len() as u64, &maximum)
        + decimal_validation_work_for_envelope(rhs_envelope.len() as u64, &tiny);
    let expected = ivm::numeric_gas::successful_call_gas(
        (lhs_envelope.len() + rhs_envelope.len()) as u64,
        frame_len_for_envelope(lhs_envelope.len() as u64)
            + frame_len_for_envelope(rhs_envelope.len() as u64),
        0,
        0,
        0,
        comparison_work,
        validation,
        0,
    )
    .expect("comparison gas");
    assert_eq!(
        compare
            .last_staged_syscall_context()
            .expect("comparison context")
            .charged(),
        expected,
    );

    let mut product = vm_for(syscalls::SYSCALL_DECIMAL_MUL, u64::MAX);
    let lhs_envelope = ivm::numeric_tlv::encode_decimal(&maximum).expect("lhs");
    let rhs_envelope = lhs_envelope.clone();
    let lhs = product.alloc_host_tlv(&lhs_envelope).expect("lhs");
    let rhs = product.alloc_host_tlv(&rhs_envelope).expect("rhs");
    product.set_register(10, lhs);
    product.set_register(11, rhs);
    product.set_register(14, NUMERIC_FAILURE_STATUS);
    product.run().expect("recoverable 16-limb product overflow");
    let product_context = product
        .last_staged_syscall_context()
        .expect("product context");
    assert_eq!(
        product_context.completion(),
        Some(SyscallCompletion::RecoverableFailure)
    );
    assert_eq!(
        product_context.phase_charge(SyscallMeteringPhase::Arithmetic),
        4 * (8 * 8 + 16),
    );
    assert_eq!(product.register(10), 0);

    for (syscall, lhs, rhs, scale) in [
        (syscalls::SYSCALL_DECIMAL_MUL, "1.25", "0.4", None),
        (syscalls::SYSCALL_DECIMAL_DIV_EXACT, "1", "40", None),
        (syscalls::SYSCALL_DECIMAL_DIV_ROUND, "1", "7", Some(28_u32)),
    ] {
        let lhs_value: Numeric = lhs.parse().expect("lhs");
        let rhs_value: Numeric = rhs.parse().expect("rhs");
        let mut observed_work = 0_u64;
        let mut observer = |step| {
            observed_work += ivm::numeric_gas::work_step_gas(step).expect("step gas") / 4;
            Ok::<_, ()>(())
        };
        match syscall {
            syscalls::SYSCALL_DECIMAL_MUL => {
                lhs_value
                    .try_decimal_mul_observed(&rhs_value, &mut observer)
                    .expect("reference multiply");
            }
            syscalls::SYSCALL_DECIMAL_DIV_EXACT => {
                lhs_value
                    .try_decimal_div_exact_observed(&rhs_value, &mut observer)
                    .expect("reference exact division");
            }
            _ => {
                lhs_value
                    .try_decimal_div_round_observed(
                        &rhs_value,
                        scale.expect("rounded scale"),
                        iroha_primitives::numeric::RoundingMode::NearestEven,
                        &mut observer,
                    )
                    .expect("reference rounded division");
            }
        }

        let mut vm = vm_for(syscall, u64::MAX);
        let lhs_envelope = ivm::numeric_tlv::encode_decimal(&lhs_value).expect("lhs envelope");
        let rhs_envelope = ivm::numeric_tlv::encode_decimal(&rhs_value).expect("rhs envelope");
        let lhs = vm.alloc_host_tlv(&lhs_envelope).expect("lhs");
        let rhs = vm.alloc_host_tlv(&rhs_envelope).expect("rhs");
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
        let mut input_bytes = (lhs_envelope.len() + rhs_envelope.len()) as u64;
        let mut validation =
            decimal_validation_work_for_envelope(lhs_envelope.len() as u64, &lhs_value)
                + decimal_validation_work_for_envelope(rhs_envelope.len() as u64, &rhs_value);
        if let Some(scale) = scale {
            let scale_envelope = ivm::numeric_tlv::encode_int(&BigInt::from_i128(scale.into()))
                .expect("scale envelope");
            let scale_pointer = vm.alloc_host_tlv(&scale_envelope).expect("scale");
            vm.set_register(12, scale_pointer);
            vm.set_register(13, RoundingModeV1::NearestEven.tag());
            input_bytes += scale_envelope.len() as u64;
            validation += validation_work_for_envelope(scale_envelope.len() as u64);
        }
        vm.run().expect("representative decimal operation");
        let output_value = result_decimal(&vm);
        let output_bytes = envelope_len(&vm, vm.register(10));
        let expected = ivm::numeric_gas::successful_call_gas(
            input_bytes,
            input_bytes - 39 * if scale.is_some() { 3 } else { 2 },
            output_bytes,
            frame_len_for_envelope(output_bytes),
            output_length_work(output_value.mantissa()),
            observed_work,
            validation,
            0,
        )
        .expect("representative gas identity");
        assert_eq!(
            vm.last_staged_syscall_context()
                .expect("representative context")
                .charged(),
            expected,
            "syscall={syscall:#x}",
        );
    }
}

#[test]
fn signed_division_remainder_and_wrapping_endpoints_are_pinned() {
    for (lhs, rhs, quotient, remainder) in [
        (-7, 3, -2, -1),
        (7, -3, -2, 1),
        (-7, -3, 2, -1),
        (7, 3, 2, 1),
    ] {
        for (syscall, expected) in [
            (syscalls::SYSCALL_INT_DIV, quotient),
            (syscalls::SYSCALL_INT_REM, remainder),
        ] {
            let mut vm = vm_for(syscall, u64::MAX);
            let lhs = install_int(&mut vm, &BigInt::from_i128(lhs));
            let rhs = install_int(&mut vm, &BigInt::from_i128(rhs));
            vm.set_register(10, lhs);
            vm.set_register(11, rhs);
            vm.set_register(14, NUMERIC_FAILURE_TRAP);
            vm.run().expect("signed integer operation");
            assert_eq!(result_int(&vm), BigInt::from_i128(expected));
        }
    }

    let mut add = vm_for(syscalls::SYSCALL_INT_WRAP_ADD, u64::MAX);
    let max = install_int(&mut add, &max_int());
    let one = install_int(&mut add, &BigInt::one());
    add.set_register(10, max);
    add.set_register(11, one);
    add.run().expect("wrapping add");
    assert_eq!(result_int(&add), min_int());

    let mut neg = vm_for(syscalls::SYSCALL_INT_WRAP_NEG, u64::MAX);
    let minimum = install_int(&mut neg, &min_int());
    neg.set_register(10, minimum);
    neg.run().expect("wrapping negation");
    assert_eq!(result_int(&neg), min_int());

    let mut sub = vm_for(syscalls::SYSCALL_INT_WRAP_SUB, u64::MAX);
    let minimum = install_int(&mut sub, &min_int());
    let one = install_int(&mut sub, &BigInt::one());
    sub.set_register(10, minimum);
    sub.set_register(11, one);
    sub.run().expect("wrapping subtraction");
    assert_eq!(result_int(&sub), max_int());

    let mut mul = vm_for(syscalls::SYSCALL_INT_WRAP_MUL, u64::MAX);
    let maximum = install_int(&mut mul, &max_int());
    let two = install_int(&mut mul, &BigInt::from_i128(2));
    mul.set_register(10, maximum);
    mul.set_register(11, two);
    mul.run().expect("wrapping multiplication");
    assert_eq!(result_int(&mul), BigInt::from_i128(-2));
}

#[test]
fn checked_int_endpoints_fault_before_output_in_trap_and_status_modes() {
    for syscall in [
        syscalls::SYSCALL_INT_NEG,
        syscalls::SYSCALL_INT_ADD,
        syscalls::SYSCALL_INT_MUL,
        syscalls::SYSCALL_INT_DIV,
        syscalls::SYSCALL_INT_REM,
    ] {
        for mode in [NUMERIC_FAILURE_TRAP, NUMERIC_FAILURE_STATUS] {
            let mut vm = vm_for(syscall, u64::MAX);
            let (lhs, rhs) = match syscall {
                syscalls::SYSCALL_INT_NEG => (min_int(), None),
                syscalls::SYSCALL_INT_ADD => (max_int(), Some(BigInt::one())),
                syscalls::SYSCALL_INT_MUL => (max_int(), Some(BigInt::from_i128(2))),
                syscalls::SYSCALL_INT_DIV | syscalls::SYSCALL_INT_REM => {
                    (min_int(), Some(BigInt::from_i128(-1)))
                }
                _ => unreachable!(),
            };
            let lhs = install_int(&mut vm, &lhs);
            vm.set_register(10, lhs);
            if let Some(rhs) = rhs {
                let rhs = install_int(&mut vm, &rhs);
                vm.set_register(11, rhs);
            }
            vm.set_register(14, mode);
            if mode == NUMERIC_FAILURE_TRAP {
                assert_eq!(
                    vm.run(),
                    Err(VMError::NumericFault(NumericFaultV1::MantissaOverflow)),
                    "trap syscall {syscall:#x}",
                );
            } else {
                vm.run().expect("recoverable endpoint overflow");
                assert_eq!(vm.register(10), 0, "status syscall {syscall:#x}");
                assert_eq!(
                    vm.register(11),
                    NumericFaultV1::MantissaOverflow.tag(),
                    "status syscall {syscall:#x}",
                );
            }
        }
    }
}

#[test]
fn exact_and_rounded_decimal_division_have_distinct_faults_and_signed_public_modes() {
    let mut exact = vm_for(syscalls::SYSCALL_DECIMAL_DIV_EXACT, u64::MAX);
    let one = install_decimal(&mut exact, &Numeric::new(1, 0));
    let three = install_decimal(&mut exact, &Numeric::new(3, 0));
    exact.set_register(10, one);
    exact.set_register(11, three);
    exact.set_register(14, NUMERIC_FAILURE_STATUS);
    exact.run().expect("recoverable repeating division");
    assert_eq!(exact.register(10), 0);
    assert_eq!(exact.register(11), NumericFaultV1::RepeatingDecimal.tag());
    assert_eq!(
        exact
            .last_staged_syscall_context()
            .expect("staged context")
            .completion(),
        Some(SyscallCompletion::RecoverableFailure)
    );

    for (mode, expected_positive, expected_negative) in [
        (RoundingModeV1::TowardZero, "2", "-2"),
        (RoundingModeV1::AwayFromZero, "3", "-3"),
        (RoundingModeV1::Floor, "2", "-3"),
        (RoundingModeV1::Ceil, "3", "-2"),
        (RoundingModeV1::NearestEven, "2", "-2"),
        (RoundingModeV1::NearestAway, "3", "-3"),
        (RoundingModeV1::NearestTowardZero, "2", "-2"),
    ] {
        for (numerator, expected) in [(5, expected_positive), (-5, expected_negative)] {
            let mut vm = vm_for(syscalls::SYSCALL_DECIMAL_DIV_ROUND, u64::MAX);
            let lhs = install_decimal(&mut vm, &Numeric::new(numerator, 0));
            let rhs = install_decimal(&mut vm, &Numeric::new(2, 0));
            let scale = install_int(&mut vm, &BigInt::zero());
            vm.set_register(10, lhs);
            vm.set_register(11, rhs);
            vm.set_register(12, scale);
            vm.set_register(13, mode.tag());
            vm.set_register(14, NUMERIC_FAILURE_TRAP);
            vm.run().expect("rounded tie");
            assert_eq!(result_decimal(&vm).to_string(), expected);
        }
    }
}

#[test]
fn quantity_is_nominal_and_underflow_is_recoverable_without_output() {
    let mut convert = vm_for(syscalls::SYSCALL_QUANTITY_TRY_FROM_DECIMAL, u64::MAX);
    let negative = install_decimal(&mut convert, &Numeric::new(-1, 0));
    convert.set_register(10, negative);
    convert.run().expect("recoverable negative quantity");
    assert_eq!(convert.register(10), 0);
    assert_eq!(convert.register(11), NumericFaultV1::NegativeQuantity.tag());

    let mut subtract = vm_for(syscalls::SYSCALL_QUANTITY_SUB, u64::MAX);
    let one: Quantity = "1".parse().expect("quantity");
    let two: Quantity = "2".parse().expect("quantity");
    let lhs = install_quantity(&mut subtract, &one);
    let rhs = install_quantity(&mut subtract, &two);
    subtract.set_register(10, lhs);
    subtract.set_register(11, rhs);
    subtract.set_register(14, NUMERIC_FAILURE_STATUS);
    subtract.run().expect("recoverable quantity underflow");
    assert_eq!(subtract.register(10), 0);
    assert_eq!(
        subtract.register(11),
        NumericFaultV1::QuantityUnderflow.tag()
    );
    let underflow_arithmetic = subtract
        .last_staged_syscall_context()
        .expect("underflow context")
        .phase_charge(SyscallMeteringPhase::Arithmetic);

    let mut add = vm_for(syscalls::SYSCALL_QUANTITY_ADD, u64::MAX);
    let lhs = install_quantity(&mut add, &one);
    let rhs = install_quantity(&mut add, &two);
    add.set_register(10, lhs);
    add.set_register(11, rhs);
    add.set_register(14, NUMERIC_FAILURE_TRAP);
    add.run().expect("quantity add");
    assert_eq!(result_quantity(&add).to_string(), "3");

    // Quantity subtraction performs one aligned subtraction and maps a
    // negative mathematical result to underflow. It must not perform and
    // charge a separate comparison pass first.
    let mut successful_subtract = vm_for(syscalls::SYSCALL_QUANTITY_SUB, u64::MAX);
    let lhs = install_quantity(&mut successful_subtract, &two);
    let rhs = install_quantity(&mut successful_subtract, &one);
    successful_subtract.set_register(10, lhs);
    successful_subtract.set_register(11, rhs);
    successful_subtract.set_register(14, NUMERIC_FAILURE_TRAP);
    successful_subtract
        .run()
        .expect("successful quantity subtraction");
    assert_eq!(result_quantity(&successful_subtract).to_string(), "1");
    let successful_arithmetic = successful_subtract
        .last_staged_syscall_context()
        .expect("successful subtraction context")
        .phase_charge(SyscallMeteringPhase::Arithmetic);
    assert_eq!(underflow_arithmetic, successful_arithmetic);

    let mut same_operands_add = vm_for(syscalls::SYSCALL_QUANTITY_ADD, u64::MAX);
    let lhs = install_quantity(&mut same_operands_add, &two);
    let rhs = install_quantity(&mut same_operands_add, &one);
    same_operands_add.set_register(10, lhs);
    same_operands_add.set_register(11, rhs);
    same_operands_add.set_register(14, NUMERIC_FAILURE_TRAP);
    same_operands_add
        .run()
        .expect("same-width quantity addition");
    assert_eq!(result_quantity(&same_operands_add).to_string(), "3");
    assert_eq!(
        same_operands_add
            .last_staged_syscall_context()
            .expect("addition context")
            .phase_charge(SyscallMeteringPhase::Arithmetic),
        successful_arithmetic,
    );
}

#[test]
fn malformed_operand_precedes_invalid_controls_and_control_faults_are_distinct() {
    let mut malformed = vm_for(syscalls::SYSCALL_INT_ADD, u64::MAX);
    let mut envelope = ivm::numeric_tlv::encode_int(&BigInt::one()).expect("int envelope");
    *envelope.last_mut().expect("hash byte") ^= 1;
    let bad = malformed
        .alloc_host_tlv(&envelope)
        .expect("install malformed int");
    let rhs = install_int(&mut malformed, &BigInt::one());
    malformed.set_register(10, bad);
    malformed.set_register(11, rhs);
    malformed.set_register(12, 9);
    malformed.set_register(14, 99);
    assert_eq!(
        malformed.run(),
        Err(VMError::PointerAbiFault(
            PointerAbiFaultV1::PayloadHashMismatch
        ))
    );

    let mut reserved = vm_for(syscalls::SYSCALL_INT_ADD, u64::MAX);
    let lhs = install_int(&mut reserved, &BigInt::one());
    let rhs = install_int(&mut reserved, &BigInt::one());
    reserved.set_register(10, lhs);
    reserved.set_register(11, rhs);
    reserved.set_register(12, 1);
    reserved.set_register(14, 99);
    assert_eq!(
        reserved.run(),
        Err(VMError::NumericFault(
            NumericFaultV1::ReservedRegisterNonZero
        ))
    );

    let mut failure = vm_for(syscalls::SYSCALL_INT_ADD, u64::MAX);
    let lhs = install_int(&mut failure, &BigInt::one());
    let rhs = install_int(&mut failure, &BigInt::one());
    failure.set_register(10, lhs);
    failure.set_register(11, rhs);
    failure.set_register(14, 99);
    assert_eq!(
        failure.run(),
        Err(VMError::NumericFault(NumericFaultV1::InvalidFailureMode))
    );

    for invalid_tag in [7, u64::MAX] {
        let mut rounding = vm_for(syscalls::SYSCALL_DECIMAL_DIV_ROUND, u64::MAX);
        let lhs = install_decimal(&mut rounding, &Numeric::new(1, 0));
        let rhs = install_decimal(&mut rounding, &Numeric::new(2, 0));
        let scale = install_int(&mut rounding, &BigInt::zero());
        rounding.set_register(10, lhs);
        rounding.set_register(11, rhs);
        rounding.set_register(12, scale);
        rounding.set_register(13, invalid_tag);
        rounding.set_register(14, NUMERIC_FAILURE_TRAP);
        assert_eq!(
            rounding.run(),
            Err(VMError::NumericFault(NumericFaultV1::InvalidRoundingMode)),
            "rounding tag {invalid_tag} must be rejected"
        );
    }

    // A scale pointer is the third operand and is authenticated before either
    // scalar control tag is interpreted.
    let mut bad_scale_pointer = vm_for(syscalls::SYSCALL_DECIMAL_DIV_ROUND, u64::MAX);
    let lhs = install_decimal(&mut bad_scale_pointer, &Numeric::new(1, 0));
    let rhs = install_decimal(&mut bad_scale_pointer, &Numeric::new(2, 0));
    let mut scale_envelope = ivm::numeric_tlv::encode_int(&BigInt::zero()).expect("scale envelope");
    *scale_envelope.last_mut().expect("scale hash") ^= 1;
    let scale = bad_scale_pointer
        .alloc_host_tlv(&scale_envelope)
        .expect("install bad scale");
    bad_scale_pointer.set_register(10, lhs);
    bad_scale_pointer.set_register(11, rhs);
    bad_scale_pointer.set_register(12, scale);
    bad_scale_pointer.set_register(13, u64::MAX);
    bad_scale_pointer.set_register(14, u64::MAX);
    assert_eq!(
        bad_scale_pointer.run(),
        Err(VMError::PointerAbiFault(
            PointerAbiFaultV1::PayloadHashMismatch
        ))
    );

    // An out-of-domain but canonical scale is a recoverable numeric failure.
    // Malformed scalar controls are traps, so rounding and then failure-mode
    // validation precede resolving that semantic scale failure.
    for (rounding_tag, failure_tag, expected) in [
        (
            u64::MAX,
            u64::MAX,
            Err(VMError::NumericFault(NumericFaultV1::InvalidRoundingMode)),
        ),
        (
            RoundingModeV1::NearestEven.tag(),
            u64::MAX,
            Err(VMError::NumericFault(NumericFaultV1::InvalidFailureMode)),
        ),
    ] {
        let mut vm = vm_for(syscalls::SYSCALL_DECIMAL_DIV_ROUND, u64::MAX);
        let lhs = install_decimal(&mut vm, &Numeric::new(1, 0));
        let rhs = install_decimal(&mut vm, &Numeric::new(2, 0));
        let scale = install_int(&mut vm, &BigInt::from_i128(29));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(12, scale);
        vm.set_register(13, rounding_tag);
        vm.set_register(14, failure_tag);
        assert_eq!(vm.run(), expected);
    }

    let mut invalid_scale = vm_for(syscalls::SYSCALL_DECIMAL_DIV_ROUND, u64::MAX);
    let lhs = install_decimal(&mut invalid_scale, &Numeric::new(1, 0));
    let rhs = install_decimal(&mut invalid_scale, &Numeric::new(2, 0));
    let scale = install_int(&mut invalid_scale, &BigInt::from_i128(29));
    invalid_scale.set_register(10, lhs);
    invalid_scale.set_register(11, rhs);
    invalid_scale.set_register(12, scale);
    invalid_scale.set_register(13, RoundingModeV1::NearestEven.tag());
    invalid_scale.set_register(14, NUMERIC_FAILURE_STATUS);
    invalid_scale.run().expect("recoverable invalid scale");
    assert_eq!(invalid_scale.register(10), 0);
    assert_eq!(
        invalid_scale.register(11),
        NumericFaultV1::InvalidScale.tag()
    );

    // Control validity is established before the zero-divisor branch begins.
    let mut zero_with_bad_mode = vm_for(syscalls::SYSCALL_DECIMAL_DIV_EXACT, u64::MAX);
    let lhs = install_decimal(&mut zero_with_bad_mode, &Numeric::new(1, 0));
    let rhs = install_decimal(&mut zero_with_bad_mode, &Numeric::new(0, 0));
    zero_with_bad_mode.set_register(10, lhs);
    zero_with_bad_mode.set_register(11, rhs);
    zero_with_bad_mode.set_register(14, u64::MAX);
    assert_eq!(
        zero_with_bad_mode.run(),
        Err(VMError::NumericFault(NumericFaultV1::InvalidFailureMode))
    );

    // Required-zero registers have precedence over the rounded conversion's
    // rounding tag because they are earlier in the public register contract.
    let mut reserved_round = vm_for(syscalls::SYSCALL_DECIMAL_TO_INT_ROUND, u64::MAX);
    let value = install_decimal(&mut reserved_round, &Numeric::new(15, 1));
    reserved_round.set_register(10, value);
    reserved_round.set_register(11, 1);
    reserved_round.set_register(13, u64::MAX);
    assert_eq!(
        reserved_round.run(),
        Err(VMError::NumericFault(
            NumericFaultV1::ReservedRegisterNonZero
        ))
    );
}

#[test]
fn structural_failure_precedes_canonical_phase_and_body_failure_follows_it() {
    let valid_frame = IntValueV1::try_new(BigInt::one())
        .expect("bounded integer")
        .encode_frame()
        .expect("valid frame");
    let mut bad_crc_frame = valid_frame;
    *bad_crc_frame.last_mut().expect("body byte") ^= 1;
    let bad_crc_envelope = numeric_envelope_from_frame(ivm::PointerType::Int, &bad_crc_frame);
    let mut bad_crc = vm_for(syscalls::SYSCALL_INT_NEG, u64::MAX);
    let pointer = bad_crc
        .alloc_host_tlv(&bad_crc_envelope)
        .expect("install structurally invalid frame");
    bad_crc.set_register(10, pointer);
    bad_crc.set_register(14, NUMERIC_FAILURE_TRAP);
    assert!(matches!(
        bad_crc.run(),
        Err(VMError::PointerAbiFault(PointerAbiFaultV1::MalformedFrame))
    ));
    let context = bad_crc
        .last_staged_syscall_context()
        .expect("structural failure context");
    assert!(context.phase_charge(SyscallMeteringPhase::NoritoDecode) > 0);
    assert_eq!(
        context.phase_charge(SyscallMeteringPhase::CanonicalValidation),
        0,
        "body validation must not be charged or begun after structural failure"
    );

    let mut noncanonical_body = Vec::new();
    noncanonical_body.extend_from_slice(&1_u32.to_le_bytes());
    noncanonical_body.push(0);
    let noncanonical_frame =
        norito::core::frame_bare_with_header_flags::<IntValueV1>(&noncanonical_body, 0)
            .expect("structurally valid noncanonical frame");
    let noncanonical_envelope =
        numeric_envelope_from_frame(ivm::PointerType::Int, &noncanonical_frame);
    let mut noncanonical = vm_for(syscalls::SYSCALL_INT_NEG, u64::MAX);
    let pointer = noncanonical
        .alloc_host_tlv(&noncanonical_envelope)
        .expect("install noncanonical frame");
    noncanonical.set_register(10, pointer);
    noncanonical.set_register(14, NUMERIC_FAILURE_TRAP);
    assert_eq!(
        noncanonical.run(),
        Err(VMError::PointerAbiFault(PointerAbiFaultV1::NonCanonical))
    );
    let context = noncanonical
        .last_staged_syscall_context()
        .expect("canonical failure context");
    assert!(context.phase_charge(SyscallMeteringPhase::NoritoDecode) > 0);
    assert!(context.phase_charge(SyscallMeteringPhase::CanonicalValidation) > 0);
}

#[test]
fn every_decode_and_output_phase_has_a_charge_before_work() {
    let syscall = syscalls::SYSCALL_INT_NEG;
    let mut baseline = vm_for(syscall, u64::MAX);
    let operand = install_int(&mut baseline, &BigInt::one());
    baseline.set_register(10, operand);
    baseline.set_register(14, NUMERIC_FAILURE_TRAP);
    baseline.run().expect("baseline numeric call");
    let context = baseline
        .last_staged_syscall_context()
        .expect("baseline staged context")
        .clone();
    let frame_bytes = ivm::numeric_tlv::encode_int(&BigInt::one())
        .expect("reference envelope")
        .len()
        - 39;
    assert_eq!(context.phase_charge(SyscallMeteringPhase::PointerHeader), 7);
    assert_eq!(
        context.phase_charge(SyscallMeteringPhase::PointerEnvelope),
        u64::try_from(frame_bytes).expect("bounded frame")
    );
    assert_eq!(
        context.phase_charge(SyscallMeteringPhase::PayloadHash),
        32 + u64::try_from(frame_bytes).expect("bounded frame")
    );
    let instruction_gas =
        ivm::cost_of(encoding::wide::encode_syscallx(syscall)).expect("SCALLX gas");
    let phases = [
        SyscallMeteringPhase::Entry,
        SyscallMeteringPhase::PointerHeader,
        SyscallMeteringPhase::PointerEnvelope,
        SyscallMeteringPhase::PayloadHash,
        SyscallMeteringPhase::NoritoDecode,
        SyscallMeteringPhase::CanonicalValidation,
        SyscallMeteringPhase::Arithmetic,
        SyscallMeteringPhase::OutputSerialization,
    ];
    let mut completed = instruction_gas;
    for phase in phases {
        let charge = context.phase_charge(phase);
        assert!(charge > 0, "phase {phase:?} must have a nonzero charge");
        let mut vm = vm_for(syscall, completed + charge - 1);
        let operand = install_int(&mut vm, &BigInt::one());
        vm.set_register(10, operand);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
        assert_eq!(
            vm.run(),
            Err(VMError::SyscallOutOfGas {
                syscall,
                phase: phase.tag(),
            }),
            "phase {phase:?}"
        );
        assert_eq!(
            vm.register(10),
            operand,
            "failed phase must not publish output"
        );
        completed += charge;
    }
}

#[test]
fn maximum_frame_hash_and_output_traversals_are_pinned_and_oog_safe() {
    let syscall = syscalls::SYSCALL_INT_NEG;
    let value = max_int();
    let envelope = ivm::numeric_tlv::encode_int(&value).expect("maximum envelope");
    let frame_bytes = frame_len_for_envelope(envelope.len() as u64);
    assert_eq!(frame_bytes, 108);

    let mut baseline = vm_for(syscall, u64::MAX);
    let operand = baseline.alloc_host_tlv(&envelope).expect("maximum operand");
    baseline.set_register(10, operand);
    baseline.set_register(14, NUMERIC_FAILURE_TRAP);
    baseline.run().expect("maximum negation");
    let result = result_int(&baseline);
    let output_bytes = envelope_len(&baseline, baseline.register(10));
    let output_frame = frame_len_for_envelope(output_bytes);
    let context = baseline
        .last_staged_syscall_context()
        .expect("maximum context")
        .clone();
    assert_eq!(
        context.phase_charge(SyscallMeteringPhase::PayloadHash),
        32 + frame_bytes
    );
    assert_eq!(
        context.phase_charge(SyscallMeteringPhase::OutputSerialization),
        4 * output_length_work(&result) + output_bytes + 2 * output_frame
    );

    let instruction = ivm::cost_of(encoding::wide::encode_syscallx(syscall)).expect("SCALLX gas");
    let hash_prefix = instruction
        + context.phase_charge(SyscallMeteringPhase::Entry)
        + context.phase_charge(SyscallMeteringPhase::PointerHeader)
        + context.phase_charge(SyscallMeteringPhase::PointerEnvelope);
    let mut hash_oog = vm_for(
        syscall,
        hash_prefix + context.phase_charge(SyscallMeteringPhase::PayloadHash) - 1,
    );
    let operand = hash_oog
        .alloc_host_tlv(&envelope)
        .expect("maximum hash operand");
    hash_oog.set_register(10, operand);
    hash_oog.set_register(14, NUMERIC_FAILURE_TRAP);
    assert_eq!(
        hash_oog.run(),
        Err(VMError::SyscallOutOfGas {
            syscall,
            phase: SyscallMeteringPhase::PayloadHash.tag(),
        })
    );

    let output_charge = context.phase_charge(SyscallMeteringPhase::OutputSerialization);
    let before_output = instruction + context.charged() - output_charge;
    let mut output_oog = vm_for(syscall, before_output + output_charge - 1);
    let operand = output_oog
        .alloc_host_tlv(&envelope)
        .expect("maximum output operand");
    output_oog.set_register(10, operand);
    output_oog.set_register(14, NUMERIC_FAILURE_TRAP);
    assert_eq!(
        output_oog.run(),
        Err(VMError::SyscallOutOfGas {
            syscall,
            phase: SyscallMeteringPhase::OutputSerialization.tag(),
        })
    );
    assert_eq!(output_oog.register(10), operand);
}

#[test]
fn entry_oog_precedes_staged_numeric_privacy_validation() {
    let syscall = syscalls::SYSCALL_INT_NEG;
    let instruction_gas =
        ivm::cost_of(encoding::wide::encode_syscallx(syscall)).expect("SCALLX gas");
    let mut vm = zk_vm_for(
        syscall,
        instruction_gas + ivm::numeric_gas::NUMERIC_ENTRY_GAS - 1,
    );
    vm.set_register(10, 1);
    vm.registers.set_tag(10, true);
    assert_eq!(
        vm.run(),
        Err(VMError::SyscallOutOfGas {
            syscall,
            phase: SyscallMeteringPhase::Entry.tag(),
        })
    );
    let context = vm
        .last_staged_syscall_context()
        .expect("entry OOG staged context");
    assert_eq!(context.charged(), 0);
    assert_eq!(context.completion(), Some(SyscallCompletion::Trap));
}

#[test]
fn every_stable_staged_phase_tag_is_reachable_in_production_numeric_paths() {
    let mut unary = vm_for(syscalls::SYSCALL_INT_NEG, u64::MAX);
    let operand = install_int(&mut unary, &BigInt::one());
    unary.set_register(10, operand);
    unary.set_register(14, NUMERIC_FAILURE_TRAP);
    unary.run().expect("unary phase fixture");
    let unary = unary
        .last_staged_syscall_context()
        .expect("unary staged context")
        .clone();

    let mut normalized = vm_for(syscalls::SYSCALL_DECIMAL_MUL, u64::MAX);
    let lhs = install_decimal(&mut normalized, &"1.25".parse().expect("lhs"));
    let rhs = install_decimal(&mut normalized, &"0.4".parse().expect("rhs"));
    normalized.set_register(10, lhs);
    normalized.set_register(11, rhs);
    normalized.set_register(14, NUMERIC_FAILURE_TRAP);
    normalized.run().expect("normalization phase fixture");
    let normalized = normalized
        .last_staged_syscall_context()
        .expect("normalization staged context");

    let phases = [
        SyscallMeteringPhase::Entry,
        SyscallMeteringPhase::PointerHeader,
        SyscallMeteringPhase::PointerEnvelope,
        SyscallMeteringPhase::PayloadHash,
        SyscallMeteringPhase::NoritoDecode,
        SyscallMeteringPhase::CanonicalValidation,
        SyscallMeteringPhase::Arithmetic,
        SyscallMeteringPhase::Normalization,
        SyscallMeteringPhase::OutputSerialization,
    ];
    assert_eq!(phases.len(), SyscallMeteringPhase::COUNT);
    for phase in phases {
        assert!(
            unary.phase_charge(phase) > 0 || normalized.phase_charge(phase) > 0,
            "stable phase {phase:?} must be reachable"
        );
    }
}

#[test]
fn repeated_arithmetic_normalization_and_scale_steps_are_individually_oog_safe() {
    let wrapping = collect_oog_prefixes(syscalls::SYSCALL_INT_WRAP_MUL, |vm| {
        let maximum = max_int();
        let lhs = install_int(vm, &maximum);
        let rhs = install_int(vm, &maximum);
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
    });
    assert!(
        wrapping
            .get(&SyscallMeteringPhase::Arithmetic.tag())
            .is_some_and(|prefixes| prefixes.len() >= 2),
        "wrapping multiplication and its 512-bit reduction have separate OOG boundaries",
    );

    let scaled_decode = collect_oog_prefixes(syscalls::SYSCALL_DECIMAL_ADD, |vm| {
        let lhs = install_decimal(vm, &"0.1".parse().expect("lhs"));
        let rhs = install_decimal(vm, &"0.2".parse().expect("rhs"));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
    });
    assert!(
        scaled_decode
            .get(&SyscallMeteringPhase::CanonicalValidation.tag())
            .is_some_and(|prefixes| prefixes.len() >= 4),
        "each body scan and scaled-mantissa divisibility probe has its own OOG boundary",
    );

    let normalization = collect_oog_prefixes(syscalls::SYSCALL_DECIMAL_MUL, |vm| {
        let lhs = install_decimal(vm, &"1.25".parse().expect("lhs"));
        let rhs = install_decimal(vm, &"0.4".parse().expect("rhs"));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
    });
    assert!(
        normalization
            .get(&SyscallMeteringPhase::Normalization.tag())
            .is_some_and(|prefixes| prefixes.len() >= 3),
        "every divide-by-ten probe must have its own pre-work OOG boundary",
    );

    let exact = collect_oog_prefixes(syscalls::SYSCALL_DECIMAL_DIV_EXACT, |vm| {
        let lhs = install_decimal(vm, &Numeric::new(1, 0));
        let rhs = install_decimal(vm, &Numeric::new(40, 0));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
    });
    assert!(
        exact
            .get(&SyscallMeteringPhase::Arithmetic.tag())
            .is_some_and(|prefixes| prefixes.len() >= 4),
        "GCD, denominator classification, and the proven exact division are separately debited",
    );

    let rounded = collect_oog_prefixes(syscalls::SYSCALL_DECIMAL_DIV_ROUND, |vm| {
        let lhs = install_decimal(vm, &Numeric::new(1, 0));
        let rhs = install_decimal(vm, &Numeric::new(7, 0));
        let scale = install_int(vm, &BigInt::from_i128(28));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(12, scale);
        vm.set_register(13, RoundingModeV1::NearestEven.tag());
        vm.set_register(14, NUMERIC_FAILURE_TRAP);
    });
    assert!(
        rounded
            .get(&SyscallMeteringPhase::Arithmetic.tag())
            .is_some_and(|prefixes| prefixes.len() >= 2),
        "scale multiplication and rounded division are separately debited",
    );
}

#[test]
fn zero_result_uses_the_dedicated_zero_rule_without_normalization_work() {
    let mut vm = vm_for(syscalls::SYSCALL_DECIMAL_ADD, u64::MAX);
    let lhs = install_decimal(&mut vm, &"0.1".parse().expect("lhs"));
    let rhs = install_decimal(&mut vm, &"-0.1".parse().expect("rhs"));
    vm.set_register(10, lhs);
    vm.set_register(11, rhs);
    vm.set_register(14, NUMERIC_FAILURE_TRAP);
    vm.run().expect("zero-producing decimal addition");
    assert_eq!(result_decimal(&vm), Numeric::zero());
    assert_eq!(
        vm.last_staged_syscall_context()
            .expect("zero-result context")
            .phase_charge(SyscallMeteringPhase::Normalization),
        0,
        "(0, scale) canonicalizes directly without a bigint probe"
    );
}

#[test]
fn numeric_failure_paths_charge_completed_work_without_output_and_oog_precedes_controls() {
    for (rhs, expected_fault) in [
        ("0", NumericFaultV1::DivisionByZero),
        ("3", NumericFaultV1::RepeatingDecimal),
    ] {
        let mut vm = vm_for(syscalls::SYSCALL_DECIMAL_DIV_EXACT, u64::MAX);
        let lhs = install_decimal(&mut vm, &Numeric::new(1, 0));
        let rhs = install_decimal(&mut vm, &rhs.parse().expect("rhs"));
        vm.set_register(10, lhs);
        vm.set_register(11, rhs);
        vm.set_register(14, NUMERIC_FAILURE_STATUS);
        vm.run().expect("recoverable exact-division failure");
        let context = vm.last_staged_syscall_context().expect("failure context");
        assert_eq!(
            context.completion(),
            Some(SyscallCompletion::RecoverableFailure)
        );
        assert_eq!(
            context.phase_charge(SyscallMeteringPhase::OutputSerialization),
            0
        );
        assert_eq!(vm.register(10), 0);
        assert_eq!(vm.register(11), expected_fault.tag());
    }

    let mut scale_overflow = vm_for(syscalls::SYSCALL_DECIMAL_DIV_EXACT, u64::MAX);
    let lhs: Numeric = "0.0000000000000000000000000001"
        .parse()
        .expect("scale-28 lhs");
    let lhs = install_decimal(&mut scale_overflow, &lhs);
    let ten = install_decimal(&mut scale_overflow, &Numeric::new(10, 0));
    scale_overflow.set_register(10, lhs);
    scale_overflow.set_register(11, ten);
    scale_overflow.set_register(14, NUMERIC_FAILURE_STATUS);
    scale_overflow.run().expect("recoverable scale overflow");
    let context = scale_overflow
        .last_staged_syscall_context()
        .expect("scale-overflow context");
    assert_eq!(
        context.completion(),
        Some(SyscallCompletion::RecoverableFailure)
    );
    assert_eq!(
        context.phase_charge(SyscallMeteringPhase::OutputSerialization),
        0
    );
    assert_eq!(scale_overflow.register(10), 0);
    assert_eq!(
        scale_overflow.register(11),
        NumericFaultV1::ExactDivisionScaleOverflow.tag(),
    );

    let syscall = syscalls::SYSCALL_DECIMAL_DIV_ROUND;
    let instruction = ivm::cost_of(encoding::wide::encode_syscallx(syscall)).expect("SCALLX gas");
    let mut invalid = vm_for(syscall, instruction + 16 + 6);
    let lhs = install_decimal(&mut invalid, &Numeric::new(1, 0));
    let rhs = install_decimal(&mut invalid, &Numeric::new(2, 0));
    let scale = install_int(&mut invalid, &BigInt::zero());
    invalid.set_register(10, lhs);
    invalid.set_register(11, rhs);
    invalid.set_register(12, scale);
    invalid.set_register(13, u64::MAX);
    invalid.set_register(14, u64::MAX);
    assert_eq!(
        invalid.run(),
        Err(VMError::SyscallOutOfGas {
            syscall,
            phase: SyscallMeteringPhase::PointerHeader.tag(),
        }),
        "unaffordable operand validation precedes invalid rounding/failure controls",
    );
}

#[test]
fn allocation_failure_is_fully_charged_and_never_publishes_result_registers() {
    let syscall = syscalls::SYSCALL_INT_NEG;
    let mut vm = vm_for(syscall, u64::MAX);
    let operand_envelope = ivm::numeric_tlv::encode_int(&BigInt::one()).expect("operand");
    let operand = vm
        .alloc_host_tlv(&operand_envelope)
        .expect("install operand before filling input");
    let aligned = operand_envelope.len().div_ceil(8) * 8;
    let remaining = usize::try_from(Memory::INPUT_SIZE).expect("input size") - aligned;
    vm.alloc_input_tlv(&vec![0; remaining])
        .expect("fill remaining input exactly");
    vm.memory.set_heap_limit(0).expect("disable heap spill");
    vm.set_register(10, operand);
    vm.set_register(14, NUMERIC_FAILURE_TRAP);
    assert_eq!(vm.run(), Err(VMError::OutOfMemory));
    assert_eq!(vm.register(10), operand);
    let context = vm
        .last_staged_syscall_context()
        .expect("failed staged context");
    assert!(context.phase_charge(SyscallMeteringPhase::OutputSerialization) > 0);
    assert_eq!(context.completion(), Some(SyscallCompletion::Trap));
}
