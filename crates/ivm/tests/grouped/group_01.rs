//! Grouped IVM integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../_debug_koto.rs"]
mod _debug_koto;
#[path = "../abi_hash_table.rs"]
mod abi_hash_table;
#[path = "../abi_hash_versions.rs"]
mod abi_hash_versions;
#[path = "../abi_policy.rs"]
mod abi_policy;
#[path = "../abi_syscall_list_golden.rs"]
mod abi_syscall_list_golden;
#[path = "../abi_syscall_sorted.rs"]
mod abi_syscall_sorted;
#[path = "../acceleration_simd.rs"]
mod acceleration_simd;
#[path = "../add_carry_circuit.rs"]
mod add_carry_circuit;
#[path = "../aes_circuit.rs"]
mod aes_circuit;
#[path = "../alu_circuit.rs"]
mod alu_circuit;
#[path = "../api.rs"]
mod api;
#[path = "../arithmetic.rs"]
mod arithmetic;
#[path = "../assert.rs"]
mod assert;
#[path = "../assert_circuit.rs"]
mod assert_circuit;
#[path = "../axt_descriptor_builder.rs"]
mod axt_descriptor_builder;
#[path = "../axt_host_flow.rs"]
mod axt_host_flow;
#[path = "../beep_test.rs"]
mod beep_test;
#[path = "../bit_ops.rs"]
mod bit_ops;
#[path = "../bn254_backend.rs"]
mod bn254_backend;
#[path = "../bn254_vec.rs"]
mod bn254_vec;
#[path = "../branch_prediction.rs"]
mod branch_prediction;
#[path = "../burn_circuit.rs"]
mod burn_circuit;
#[path = "../byte_merkle_tree.rs"]
mod byte_merkle_tree;
#[path = "../classic_opcode_rejected.rs"]
mod classic_opcode_rejected;
#[path = "../cli_smoke.rs"]
mod cli_smoke;
#[path = "../code_hash.rs"]
mod code_hash;
#[path = "../commit_output.rs"]
mod commit_output;
#[path = "../compact_bundle_helpers.rs"]
mod compact_bundle_helpers;
#[path = "../compact_bundle_norito.rs"]
mod compact_bundle_norito;
#[path = "../comparison.rs"]
mod comparison;
#[path = "../comparison_circuit.rs"]
mod comparison_circuit;
#[path = "../contract_artifact.rs"]
mod contract_artifact;
