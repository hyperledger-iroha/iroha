//! Grouped Iroha Core integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../gov_zk_referendum_window_guard.rs"]
mod gov_zk_referendum_window_guard;
#[path = "../implicit_account_receive.rs"]
mod implicit_account_receive;
#[path = "../isi_gas_fees.rs"]
mod isi_gas_fees;
#[path = "../ivm_admission_unknown_syscall.rs"]
mod ivm_admission_unknown_syscall;
#[path = "../ivm_codec_helpers.rs"]
mod ivm_codec_helpers;
#[path = "../ivm_corehost_axt.rs"]
mod ivm_corehost_axt;
#[path = "../ivm_corehost_domain.rs"]
mod ivm_corehost_domain;
#[path = "../ivm_corehost_envelope_hash_bind.rs"]
mod ivm_corehost_envelope_hash_bind;
#[path = "../ivm_corehost_goldilocks.rs"]
mod ivm_corehost_goldilocks;
#[path = "../ivm_corehost_halo2_disabled_latch.rs"]
mod ivm_corehost_halo2_disabled_latch;
#[path = "../ivm_corehost_halo2_enabled_vendor_ok.rs"]
mod ivm_corehost_halo2_enabled_vendor_ok;
#[path = "../ivm_corehost_tlv_neg.rs"]
mod ivm_corehost_tlv_neg;
#[path = "../ivm_corehost_zk_gate.rs"]
mod ivm_corehost_zk_gate;
#[path = "../ivm_event_ordering.rs"]
mod ivm_event_ordering;
#[path = "../ivm_executor.rs"]
mod ivm_executor;
#[path = "../ivm_host_mapping.rs"]
mod ivm_host_mapping;
#[path = "../ivm_host_shadow_execute.rs"]
mod ivm_host_shadow_execute;
#[path = "../ivm_manifest_abi_reject.rs"]
mod ivm_manifest_abi_reject;
#[path = "../ivm_pointer_abi_apply.rs"]
mod ivm_pointer_abi_apply;
#[path = "../ivm_pointer_abi_policy.rs"]
mod ivm_pointer_abi_policy;
#[path = "../ivm_pointer_abi_tlv_hash.rs"]
mod ivm_pointer_abi_tlv_hash;
#[path = "../ivm_pointer_abi_tlv_types.rs"]
mod ivm_pointer_abi_tlv_types;
#[path = "../ivm_syscall_policy.rs"]
mod ivm_syscall_policy;
#[path = "../kotodama_authority_apply.rs"]
mod kotodama_authority_apply;
#[path = "../kotodama_hello_entrypoint_apply.rs"]
mod kotodama_hello_entrypoint_apply;
#[path = "../kotodama_pointer_abi_apply.rs"]
mod kotodama_pointer_abi_apply;
#[path = "../limits_enforcement.rs"]
mod limits_enforcement;
#[path = "../nexus_policies.rs"]
mod nexus_policies;
#[path = "../oracle.rs"]
mod oracle;
#[path = "../overlay_bounds.rs"]
mod overlay_bounds;
#[path = "../overlay_chunking.rs"]
mod overlay_chunking;
#[path = "../overlay_workers_parity.rs"]
mod overlay_workers_parity;
