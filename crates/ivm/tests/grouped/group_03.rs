//! Grouped IVM integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../dynamic_memory.rs"]
mod dynamic_memory;
#[path = "../ecadd_circuit.rs"]
mod ecadd_circuit;
#[path = "../ecdsa_circuit.rs"]
mod ecdsa_circuit;
#[path = "../ed25519_batch.rs"]
mod ed25519_batch;
#[path = "../ed25519_circuit.rs"]
mod ed25519_circuit;
#[path = "../encoding.rs"]
mod encoding;
#[path = "../expanded_instruction.rs"]
mod expanded_instruction;
#[path = "../field_circuit.rs"]
mod field_circuit;
#[path = "../field_dispatch.rs"]
mod field_dispatch;
#[path = "../field_ops.rs"]
mod field_ops;
#[path = "../gas.rs"]
mod gas;
#[path = "../gas_conformance.rs"]
mod gas_conformance;
#[path = "../gas_edge.rs"]
mod gas_edge;
#[path = "../gas_golden.rs"]
mod gas_golden;
#[path = "../gas_property.rs"]
mod gas_property;
#[path = "../gas_replay.rs"]
mod gas_replay;
#[path = "../gas_schedule.rs"]
mod gas_schedule;
#[path = "../gas_schedule_hash.rs"]
mod gas_schedule_hash;
#[path = "../gpu_determinism.rs"]
mod gpu_determinism;
#[path = "../gpu_manager.rs"]
mod gpu_manager;
#[path = "../hardware_determinism.rs"]
mod hardware_determinism;
#[path = "../host_roundtrip.rs"]
mod host_roundtrip;
#[path = "../host_syscall_coverage.rs"]
mod host_syscall_coverage;
#[path = "../host_unknown_syscall.rs"]
mod host_unknown_syscall;
#[path = "../i18n.rs"]
mod i18n;
#[path = "../ilp.rs"]
mod ilp;
#[path = "../ilp_gas_error.rs"]
mod ilp_gas_error;
#[path = "../ilp_parity_mem.rs"]
mod ilp_parity_mem;
#[path = "../ilp_parity_random.rs"]
mod ilp_parity_random;
#[path = "../input_tlv_alloc.rs"]
mod input_tlv_alloc;
#[path = "../iso20022_http.rs"]
mod iso20022_http;
#[path = "../ivm_abi_doc_sync.rs"]
mod ivm_abi_doc_sync;
