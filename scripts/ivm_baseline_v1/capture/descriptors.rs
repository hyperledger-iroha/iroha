//! Frozen baseline descriptors: the bytes the anchor hashes and every value derived from them.

use std::{fs, path::Path};

use iroha_crypto::Hash;
use iroha_data_model::{
    executor::{
        ArtifactAbiHashMismatchInfo, DecodedCodeSizeLimitInfo, DecodedInstructionLimitInfo,
        IvmAdmissionError, ManifestAbiHashMismatchInfo, ManifestCodeHashMismatchInfo,
        MaxCyclesExceedsFuelInfo, MaxCyclesExceedsUpperBoundInfo, UnsupportedVersionInfo,
        VectorLengthTooLargeInfo,
        fault::{IvmFaultKindV1, NumericFaultV1, PointerAbiFaultV1},
    },
    runtime::definition::{
        DEFINITION_FORMAT_VERSION, DEFINITION_ID_DOMAIN, DefinitionDescriptorV1,
        HostProfileDescriptorV1,
    },
};
use ivm::{PointerType, ProgramMetadata, SyscallPolicy, instruction::wide as op, syscalls};
use norito::json::Value;

use crate::oracle::{h, n, obj, s, sha256_hex, wire_json};

/// Name of every opcode value the anchor defines, valid or reserved.
pub const OPCODE_NAMES: &[(u8, &str, &str)] = &[
    (op::arithmetic::ADD, "ADD", "arithmetic"),
    (op::arithmetic::SUB, "SUB", "arithmetic"),
    (op::arithmetic::AND, "AND", "arithmetic"),
    (op::arithmetic::OR, "OR", "arithmetic"),
    (op::arithmetic::XOR, "XOR", "arithmetic"),
    (op::arithmetic::SLL, "SLL", "arithmetic"),
    (op::arithmetic::SRL, "SRL", "arithmetic"),
    (op::arithmetic::SRA, "SRA", "arithmetic"),
    (op::arithmetic::SLT, "SLT", "arithmetic"),
    (op::arithmetic::SLTU, "SLTU", "arithmetic"),
    (op::arithmetic::CMOV, "CMOV", "arithmetic"),
    (op::arithmetic::NOT, "NOT", "arithmetic"),
    (op::arithmetic::NEG, "NEG", "arithmetic"),
    (op::arithmetic::SEQ, "SEQ", "arithmetic"),
    (op::arithmetic::SNE, "SNE", "arithmetic"),
    (op::arithmetic::MUL, "MUL", "arithmetic"),
    (op::arithmetic::MULH, "MULH", "arithmetic"),
    (op::arithmetic::MULHU, "MULHU", "arithmetic"),
    (op::arithmetic::MULHSU, "MULHSU", "arithmetic"),
    (op::arithmetic::DIV, "DIV", "arithmetic"),
    (op::arithmetic::DIVU, "DIVU", "arithmetic"),
    (op::arithmetic::REM, "REM", "arithmetic"),
    (op::arithmetic::REMU, "REMU", "arithmetic"),
    (op::arithmetic::ROTL, "ROTL", "arithmetic"),
    (op::arithmetic::ROTR, "ROTR", "arithmetic"),
    (op::arithmetic::POPCNT, "POPCNT", "arithmetic"),
    (op::arithmetic::CLZ, "CLZ", "arithmetic"),
    (op::arithmetic::CTZ, "CTZ", "arithmetic"),
    (op::arithmetic::ISQRT, "ISQRT", "arithmetic"),
    (op::arithmetic::MIN, "MIN", "arithmetic"),
    (op::arithmetic::MAX, "MAX", "arithmetic"),
    (op::arithmetic::ABS, "ABS", "arithmetic"),
    (op::arithmetic::DIV_CEIL, "DIV_CEIL", "arithmetic"),
    (op::arithmetic::GCD, "GCD", "arithmetic"),
    (op::arithmetic::MEAN, "MEAN", "arithmetic"),
    (op::arithmetic::ADDI, "ADDI", "arithmetic-immediate"),
    (op::arithmetic::ANDI, "ANDI", "arithmetic-immediate"),
    (op::arithmetic::ORI, "ORI", "arithmetic-immediate"),
    (op::arithmetic::XORI, "XORI", "arithmetic-immediate"),
    (op::arithmetic::CMOVI, "CMOVI", "arithmetic-immediate"),
    (op::arithmetic::ROTL_IMM, "ROTL_IMM", "arithmetic-immediate"),
    (op::arithmetic::ROTR_IMM, "ROTR_IMM", "arithmetic-immediate"),
    (op::memory::LOAD64, "LOAD64", "memory"),
    (op::memory::STORE64, "STORE64", "memory"),
    (op::memory::LOAD128, "LOAD128", "memory-128"),
    (op::memory::STORE128, "STORE128", "memory-128"),
    (op::memory::LDLIT, "LDLIT", "memory-literal"),
    (op::memory::LDI64, "LDI64", "memory-literal"),
    (op::control::BEQ, "BEQ", "control-branch"),
    (op::control::BNE, "BNE", "control-branch"),
    (op::control::BLT, "BLT", "control-branch"),
    (op::control::BGE, "BGE", "control-branch"),
    (op::control::BLTU, "BLTU", "control-branch"),
    (op::control::BGEU, "BGEU", "control-branch"),
    (op::control::JAL, "JAL", "control-jump"),
    (op::control::JR, "JR", "control-jump"),
    (op::control::JALR, "JALR", "control-jump"),
    (op::control::HALT, "HALT", "control"),
    (op::control::JMP, "JMP", "control-jump"),
    (op::control::JALS, "JALS", "control-jump"),
    (op::system::SCALL, "SCALL", "system"),
    (op::system::GETGAS, "GETGAS", "system"),
    (op::system::SYSTEM, "SYSTEM", "system"),
    (op::crypto::VADD32, "VADD32", "vector"),
    (op::crypto::VADD64, "VADD64", "vector"),
    (op::crypto::VAND, "VAND", "vector"),
    (op::crypto::VXOR, "VXOR", "vector"),
    (op::crypto::VOR, "VOR", "vector"),
    (op::crypto::VROT32, "VROT32", "vector"),
    (op::crypto::SETVL, "SETVL", "vector"),
    (op::crypto::PARBEGIN, "PARBEGIN", "parallel"),
    (op::crypto::PAREND, "PAREND", "parallel"),
    (op::crypto::SHA256BLOCK, "SHA256BLOCK", "direct-crypto"),
    (op::crypto::SHA3BLOCK, "SHA3BLOCK", "direct-crypto"),
    (op::crypto::POSEIDON2, "POSEIDON2", "direct-crypto"),
    (op::crypto::POSEIDON6, "POSEIDON6", "direct-crypto"),
    (op::crypto::AESENC, "AESENC", "direct-crypto"),
    (op::crypto::AESDEC, "AESDEC", "direct-crypto"),
    (op::crypto::BLAKE2S, "BLAKE2S", "direct-crypto"),
    (op::crypto::ED25519VERIFY, "ED25519VERIFY", "direct-crypto"),
    (
        op::crypto::ED25519BATCHVERIFY,
        "ED25519BATCHVERIFY",
        "direct-crypto",
    ),
    (op::crypto::ECDSAVERIFY, "ECDSAVERIFY", "direct-crypto"),
    (
        op::crypto::DILITHIUMVERIFY,
        "DILITHIUMVERIFY",
        "direct-crypto",
    ),
    (op::iso20022::MSG_CREATE, "MSG_CREATE", "iso20022-reserved"),
    (op::iso20022::MSG_CLONE, "MSG_CLONE", "iso20022-reserved"),
    (op::iso20022::MSG_SET, "MSG_SET", "iso20022-reserved"),
    (op::iso20022::MSG_GET, "MSG_GET", "iso20022-reserved"),
    (op::iso20022::MSG_ADD, "MSG_ADD", "iso20022-reserved"),
    (op::iso20022::MSG_REMOVE, "MSG_REMOVE", "iso20022-reserved"),
    (op::iso20022::MSG_CLEAR, "MSG_CLEAR", "iso20022-reserved"),
    (op::iso20022::MSG_PARSE, "MSG_PARSE", "iso20022-reserved"),
    (
        op::iso20022::MSG_SERIALIZE,
        "MSG_SERIALIZE",
        "iso20022-reserved",
    ),
    (
        op::iso20022::MSG_VALIDATE,
        "MSG_VALIDATE",
        "iso20022-reserved",
    ),
    (op::iso20022::MSG_SIGN, "MSG_SIGN", "iso20022-reserved"),
    (
        op::iso20022::MSG_VERIFY_SIG,
        "MSG_VERIFY_SIG",
        "iso20022-reserved",
    ),
    (op::iso20022::MSG_SEND, "MSG_SEND", "iso20022-reserved"),
    (op::iso20022::ENCODE_STR, "ENCODE_STR", "iso20022-reserved"),
    (op::iso20022::DECODE_STR, "DECODE_STR", "iso20022-reserved"),
    (
        op::iso20022::VALIDATE_FORMAT,
        "VALIDATE_FORMAT",
        "iso20022-reserved",
    ),
    (op::zk::ASSERT, "ASSERT", "zk-helper"),
    (op::zk::ASSERT_EQ, "ASSERT_EQ", "zk-helper"),
    (op::zk::FADD, "FADD", "zk-helper"),
    (op::zk::FSUB, "FSUB", "zk-helper"),
    (op::zk::FMUL, "FMUL", "zk-helper"),
    (op::zk::FINV, "FINV", "zk-helper"),
    (op::zk::ASSERT_RANGE, "ASSERT_RANGE", "zk-helper"),
];

/// Name of an opcode value, if the anchor defines one.
pub fn opcode_name(opcode: u8) -> Option<(&'static str, &'static str)> {
    OPCODE_NAMES
        .iter()
        .find(|(value, _, _)| *value == opcode)
        .map(|(_, name, family)| (*name, *family))
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| panic!("{}: {error}", parent.display()));
    }
    fs::write(path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn contiguous(table: &[Value], first: u64) {
    for (offset, entry) in table.iter().enumerate() {
        let tag = entry
            .as_object()
            .and_then(|entry| entry.get("wire_tag"))
            .and_then(Value::as_u64)
            .expect("wire table entry");
        assert_eq!(tag, first + offset as u64, "wire tags are not contiguous");
    }
}

fn fault_kinds() -> Vec<Value> {
    use IvmFaultKindV1 as F;
    let kinds = [
        F::OutOfGas,
        F::MemoryLimitExceeded,
        F::MemoryAccessViolation,
        F::MisalignedAccess,
        F::MemoryOutOfBounds,
        F::DecodeError,
        F::InvalidOpcode,
        F::UnknownSyscall,
        F::UnsupportedSyscall,
        F::GasCostOverflow,
        F::Numeric(NumericFaultV1::MantissaOverflow),
        F::PointerAbi(PointerAbiFaultV1::InvalidAddress),
        F::AssertionFailed,
        F::ExceededMaxCycles,
        F::InvalidMetadata,
        F::InvalidVectorLength,
        F::MissingHalt,
        F::VectorExtensionDisabled,
        F::ZkExtensionDisabled,
        F::NullifierAlreadyUsed,
        F::PermissionDenied,
        F::PrivacyViolation,
        F::RegisterOutOfBounds,
        F::NoritoInvalid,
        F::AbiTypeNotAllowed,
        F::HostOutputItemsExceeded,
        F::HostOutputBytesExceeded,
        F::AmxBudgetExceeded,
        F::ReentrantCall,
        F::CallDepthExceeded,
    ];
    let table: Vec<Value> = kinds.iter().map(wire_json).collect();
    contiguous(&table, 0);
    table
}

fn admission_errors() -> Vec<Value> {
    use IvmAdmissionError as E;
    let expected = Hash::prehashed([0x11; 32]);
    let actual = Hash::prehashed([0x22; 32]);
    let errors = [
        E::MissingMaxCycles,
        E::UnsupportedVersion(UnsupportedVersionInfo { major: 2, minor: 1 }),
        E::UnsupportedFeatureBits(0x80),
        E::UnsupportedAbiVersion(3),
        E::AbiVersionNotActive(1),
        E::VectorLengthTooLarge(VectorLengthTooLargeInfo {
            vector_length: 65,
            max_allowed: 64,
        }),
        E::MaxCyclesExceedsUpperBound(MaxCyclesExceedsUpperBoundInfo {
            max_cycles: 1_000_001,
            upper_bound: 1_000_000,
        }),
        E::MaxCyclesExceedsFuel(MaxCyclesExceedsFuelInfo {
            max_cycles: 55_000_001,
            fuel_limit: 55_000_000,
        }),
        E::DecodedInstructionCountExceeded(DecodedInstructionLimitInfo {
            decoded_instructions: 2,
            limit: 1,
        }),
        E::DecodedCodeSizeExceeded(DecodedCodeSizeLimitInfo {
            decoded_bytes: 8,
            limit: 4,
        }),
        E::BytecodeDecodingFailed("example".to_owned()),
        E::ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo { expected, actual }),
        E::ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo { expected, actual }),
        E::ManifestCodeHashMissing,
        E::ManifestAbiHashMissing,
        E::ManifestMalformed,
        E::ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo { expected, actual }),
        E::GenericSyscallNotAllowed(0xA0),
    ];
    let table: Vec<Value> = errors.iter().map(wire_json).collect();
    contiguous(&table, 0);
    table
}

fn numeric_faults() -> Vec<Value> {
    (0..=255_u64)
        .filter_map(NumericFaultV1::from_tag)
        .map(|fault| {
            let entry = wire_json(&fault);
            let tag = entry
                .as_object()
                .and_then(|entry| entry.get("wire_tag"))
                .and_then(Value::as_u64);
            assert_eq!(tag, Some(fault.tag()), "numeric fault wire tag");
            entry
        })
        .collect()
}

fn pointer_abi_faults() -> Vec<Value> {
    (0..=255_u64)
        .filter_map(PointerAbiFaultV1::from_tag)
        .map(|fault| {
            let entry = wire_json(&fault);
            let tag = entry
                .as_object()
                .and_then(|entry| entry.get("wire_tag"))
                .and_then(Value::as_u64);
            assert_eq!(tag, Some(fault.tag()), "pointer-ABI fault wire tag");
            entry
        })
        .collect()
}

/// Write the descriptor files under `out/descriptors` and return `frozen_values.json`.
pub fn capture(out: &Path, anchor_root: &Path, anchor_commit: &str) -> Value {
    let dir = out.join("descriptors");
    let policy = SyscallPolicy::AbiV1;

    let abi_descriptor = syscalls::baseline_capture_abi_surface_descriptor(policy);
    let abi_hash = syscalls::compute_abi_hash(policy);
    assert_eq!(
        *Hash::new(&abi_descriptor).as_ref(),
        abi_hash,
        "the captured ABI descriptor is the one the anchor hashes"
    );
    write(&dir.join("abi_v1.descriptor.bin"), &abi_descriptor);

    let gas_descriptor = ivm::gas::baseline_capture_gas_schedule_descriptor();
    let schedule_hash: [u8; 32] = ivm::gas::schedule_hash().into();
    assert_eq!(
        *Hash::new(&gas_descriptor).as_ref(),
        schedule_hash,
        "the captured gas descriptor is the one the anchor hashes"
    );
    write(&dir.join("gas_schedule_v1.descriptor.bin"), &gas_descriptor);

    // The HostProfile `abi.v1` definition, built exactly as the anchor's
    // `iroha_core::runtime::catalog` builds it.
    let host_profile = DefinitionDescriptorV1::HostProfile(HostProfileDescriptorV1 {
        abi_version: 1,
        abi_hash,
        metering: schedule_hash,
    });
    let host_profile_bytes = host_profile
        .canonical_bytes()
        .expect("canonical HostProfile descriptor");
    let host_profile_id = host_profile.id().expect("HostProfile definition id").0;
    let kind_tag = host_profile.kind().tag();
    let length = (host_profile_bytes.len() as u64).to_le_bytes();
    let recomputed: [u8; 32] = Hash::new_from_chunks(&[
        DEFINITION_ID_DOMAIN,
        &[DEFINITION_FORMAT_VERSION, kind_tag],
        &length,
        &host_profile_bytes,
    ])
    .into();
    assert_eq!(recomputed, host_profile_id, "definition id preimage");
    write(
        &dir.join("host_profile_abi_v1.descriptor.bin"),
        &host_profile_bytes,
    );

    let catalog_path = anchor_root.join("specs/runtime_baseline_catalog.json");
    let catalog_bytes = fs::read(&catalog_path)
        .unwrap_or_else(|error| panic!("{}: {error}", catalog_path.display()));
    write(&dir.join("runtime_baseline_catalog.json"), &catalog_bytes);
    let catalog: Value = norito::json::from_slice(&catalog_bytes).expect("catalog JSON");
    let catalog = catalog.as_object().expect("catalog object");
    let selection = catalog
        .get("selection")
        .and_then(Value::as_str)
        .expect("catalog selection");
    let catalog_host_profile = catalog
        .get("definitions")
        .and_then(Value::as_array)
        .expect("catalog definitions")
        .iter()
        .filter_map(Value::as_object)
        .find(|definition| definition.get("kind").and_then(Value::as_str) == Some("HostProfile"))
        .expect("catalog HostProfile entry");
    let catalog_host_profile_id = catalog_host_profile
        .get("id")
        .and_then(Value::as_str)
        .expect("catalog HostProfile id");
    assert_eq!(
        catalog_host_profile_id,
        hex::encode(host_profile_id),
        "the anchor catalog pins the rebuilt HostProfile id"
    );

    let default_header = ProgramMetadata::default().encode();
    assert_eq!(default_header.len(), ivm::HEADER_SIZE);
    let zk_schedule_hash: [u8; 32] = ivm::gas::ZkGasScheduleV1::default().hash().into();

    let opcodes: Vec<Value> = (0..=u8::MAX)
        .filter(|opcode| op::is_valid_opcode(*opcode) || opcode_name(*opcode).is_some())
        .map(|opcode| {
            let (name, family) = opcode_name(opcode).expect("every valid opcode is named");
            let word = u32::from(opcode) << 24;
            obj([
                ("opcode", n(u64::from(opcode))),
                ("hex", s(format!("0x{opcode:02x}"))),
                ("name", s(name)),
                ("family", s(family)),
                ("valid", Value::Bool(op::is_valid_opcode(opcode))),
                ("gas", ivm::gas::cost_of(word).map_or(Value::Null, n)),
                (
                    "in_gas_schedule",
                    Value::Bool(ivm::gas::SCHEDULE_OPCODES.contains(&opcode)),
                ),
            ])
        })
        .collect();
    let syscall_list: Vec<Value> = syscalls::abi_syscall_list()
        .iter()
        .map(|number| {
            obj([
                ("number", n(u64::from(*number))),
                ("hex", s(format!("0x{number:X}"))),
                (
                    "name",
                    syscalls::syscall_name(*number).map_or(Value::Null, s),
                ),
                (
                    "generic_program_allowed",
                    Value::Bool(syscalls::is_generic_program_syscall_allowed(
                        policy, *number,
                    )),
                ),
            ])
        })
        .collect();
    let pointer_types: Vec<Value> = PointerType::all()
        .iter()
        .map(|pointer_type| {
            obj([
                ("id", n(u64::from(*pointer_type as u16))),
                ("name", s(format!("{pointer_type:?}"))),
                (
                    "allowed",
                    Value::Bool(ivm::is_type_allowed_for_policy(policy, *pointer_type)),
                ),
            ])
        })
        .collect();

    obj([
        ("anchor_commit", s(anchor_commit)),
        (
            "hash_algorithm",
            s(
                "iroha_crypto::Hash::new: Blake2b-256 (Blake2bVar, 32-byte output) with the least significant bit of the final byte set to 1",
            ),
        ),
        (
            "abi_v1",
            obj([
                ("policy", s("SyscallPolicy::AbiV1")),
                ("descriptor_file", s("descriptors/abi_v1.descriptor.bin")),
                ("descriptor_len", n(abi_descriptor.len() as u64)),
                ("descriptor_sha256", s(sha256_hex(&abi_descriptor))),
                ("abi_hash", h(&abi_hash)),
            ]),
        ),
        (
            "abi_v1_runtime_lifecycle",
            obj([
                ("exists_at_anchor", Value::Bool(false)),
                (
                    "note",
                    s("the anchor's ivm_abi::SyscallPolicy has the single variant AbiV1"),
                ),
            ]),
        ),
        (
            "gas_schedule_v1",
            obj([
                (
                    "descriptor_file",
                    s("descriptors/gas_schedule_v1.descriptor.bin"),
                ),
                ("descriptor_len", n(gas_descriptor.len() as u64)),
                ("descriptor_sha256", s(sha256_hex(&gas_descriptor))),
                ("schedule_hash", h(&schedule_hash)),
                ("max_instruction_cost", n(ivm::gas::max_instruction_cost())),
                ("zk_gas_schedule_hash", h(&zk_schedule_hash)),
            ]),
        ),
        (
            "host_profile_abi_v1",
            obj([
                ("catalog_kind", s("HostProfile")),
                (
                    "catalog_name",
                    catalog_host_profile
                        .get("name")
                        .cloned()
                        .unwrap_or(Value::Null),
                ),
                (
                    "descriptor_file",
                    s("descriptors/host_profile_abi_v1.descriptor.bin"),
                ),
                ("descriptor_hex", h(&host_profile_bytes)),
                (
                    "descriptor_json",
                    norito::json::to_value(&host_profile).expect("HostProfile JSON"),
                ),
                ("abi_version", n(1)),
                ("abi_hash", h(&abi_hash)),
                ("metering", h(&schedule_hash)),
                ("definition_id", h(&host_profile_id)),
                (
                    "definition_id_preimage",
                    obj([
                        (
                            "formula",
                            s(
                                "Hash::new(domain || [format_version, kind_tag] || le64(len(descriptor)) || descriptor)",
                            ),
                        ),
                        ("domain_hex", h(DEFINITION_ID_DOMAIN)),
                        (
                            "domain_text",
                            s(String::from_utf8_lossy(DEFINITION_ID_DOMAIN).into_owned()),
                        ),
                        ("format_version", n(u64::from(DEFINITION_FORMAT_VERSION))),
                        ("kind_tag", n(u64::from(kind_tag))),
                        ("length_le64_hex", h(&length)),
                    ]),
                ),
            ]),
        ),
        (
            "runtime_baseline_catalog",
            obj([
                ("file", s("descriptors/runtime_baseline_catalog.json")),
                ("selection", s(selection)),
                (
                    "definitions",
                    n(catalog
                        .get("definitions")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len) as u64),
                ),
            ]),
        ),
        (
            "header",
            obj([
                ("size", n(ivm::HEADER_SIZE as u64)),
                ("magic_hex", h(ivm::METADATA_MAGIC)),
                ("version_major", n(1)),
                ("version_minor", n(1)),
                ("mode_zk", n(u64::from(ivm::ivm_mode::ZK))),
                ("mode_vector", n(u64::from(ivm::ivm_mode::VECTOR))),
                ("vector_length_max", n(u64::from(ivm::VECTOR_LENGTH_MAX))),
                (
                    "layout",
                    s(
                        "magic[0..4]; version_major[4]; version_minor[5]; mode[6]; vector_length[7]; max_cycles u64 le [8..16]; abi_version[16]; abi_hash[17..49]",
                    ),
                ),
                ("default_header_hex", h(&default_header)),
            ]),
        ),
        (
            "memory_layout",
            obj([
                ("heap_start", n(ivm::Memory::HEAP_START)),
                ("heap_size", n(ivm::Memory::HEAP_SIZE)),
                ("input_start", n(ivm::Memory::INPUT_START)),
                ("input_size", n(ivm::Memory::INPUT_SIZE)),
                ("output_start", n(ivm::Memory::OUTPUT_START)),
                ("output_size", n(ivm::Memory::OUTPUT_SIZE)),
                ("stack_start", n(ivm::Memory::STACK_START)),
                ("stack_size", n(ivm::Memory::STACK_SIZE)),
            ]),
        ),
        ("zk_default_max_cycles", n(ivm::zk::MAX_CYCLES)),
        ("opcodes", Value::Array(opcodes)),
        ("syscalls", Value::Array(syscall_list)),
        ("pointer_types", Value::Array(pointer_types)),
        (
            "wire_tags",
            obj([
                ("IvmAdmissionError", Value::Array(admission_errors())),
                ("IvmFaultKindV1", Value::Array(fault_kinds())),
                ("NumericFaultV1", Value::Array(numeric_faults())),
                ("PointerAbiFaultV1", Value::Array(pointer_abi_faults())),
            ]),
        ),
    ])
}
