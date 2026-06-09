//! Grouped IVM integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../kotodama_state_map_lowering.rs"]
mod kotodama_state_map_lowering;
#[path = "../kotodama_state_map_pointer.rs"]
mod kotodama_state_map_pointer;
#[path = "../kotodama_state_name_map_runtime.rs"]
mod kotodama_state_name_map_runtime;
#[path = "../kotodama_state_scalar.rs"]
mod kotodama_state_scalar;
#[path = "../kotodama_state_struct_pointer.rs"]
mod kotodama_state_struct_pointer;
#[path = "../kotodama_struct_fields.rs"]
mod kotodama_struct_fields;
#[path = "../kotodama_struct_fields_corehost.rs"]
mod kotodama_struct_fields_corehost;
#[path = "../kotodama_ternary_lowering.rs"]
mod kotodama_ternary_lowering;
#[path = "../kotodama_tuple_codegen_neg.rs"]
mod kotodama_tuple_codegen_neg;
#[path = "../kotodama_tuple_lowering.rs"]
mod kotodama_tuple_lowering;
#[path = "../kotodama_wrappers.rs"]
mod kotodama_wrappers;
#[path = "../kotodama_zk_syscalls.rs"]
mod kotodama_zk_syscalls;
#[path = "../load_code.rs"]
mod load_code;
#[path = "../manifest_roundtrip.rs"]
mod manifest_roundtrip;
#[path = "../memory.rs"]
mod memory;
#[path = "../memory_circuit.rs"]
mod memory_circuit;
#[path = "../memory_commit.rs"]
mod memory_commit;
#[path = "../memory_compact_helper.rs"]
mod memory_compact_helper;
#[path = "../memory_log.rs"]
mod memory_log;
#[path = "../memory_merkle.rs"]
mod memory_merkle;
#[path = "../memory_merkle_combined.rs"]
mod memory_merkle_combined;
#[path = "../merkle_circuit.rs"]
mod merkle_circuit;
#[path = "../merkle_cross.rs"]
mod merkle_cross;
#[path = "../merkle_crosscrate.rs"]
mod merkle_crosscrate;
#[path = "../merkle_dirs.rs"]
mod merkle_dirs;
#[path = "../merkle_dirs_crosscheck.rs"]
mod merkle_dirs_crosscheck;
#[path = "../merkle_super_hash.rs"]
mod merkle_super_hash;
#[path = "../merkle_unification.rs"]
mod merkle_unification;
#[path = "../metadata.rs"]
mod metadata;
#[path = "../metadata_parse.rs"]
mod metadata_parse;
#[path = "../metadata_roundtrip.rs"]
mod metadata_roundtrip;
#[path = "../metal_disable_on_mismatch.rs"]
mod metal_disable_on_mismatch;
