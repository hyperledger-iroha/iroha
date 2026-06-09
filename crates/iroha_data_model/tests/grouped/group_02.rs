//! Grouped Iroha data model integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../model_parser.rs"]
mod model_parser;
#[path = "../mutators.rs"]
mod mutators;
#[path = "../norito_chain_layout.rs"]
mod norito_chain_layout;
#[path = "../norito_golden_scaffold.rs"]
mod norito_golden_scaffold;
#[path = "../norito_pointer_abi_roundtrip.rs"]
mod norito_pointer_abi_roundtrip;
#[path = "../oracle_query_roundtrip.rs"]
mod oracle_query_roundtrip;
#[path = "../oracle_reference_fixtures.rs"]
mod oracle_reference_fixtures;
#[path = "../parameter_defaults.rs"]
mod parameter_defaults;
#[path = "../peer.rs"]
mod peer;
#[path = "../precomputed.rs"]
mod precomputed;
#[path = "../print_asset_norito.rs"]
mod print_asset_norito;
#[path = "../query_accessors.rs"]
mod query_accessors;
#[path = "../query_json_envelope.rs"]
mod query_json_envelope;
#[path = "../query_response_roundtrip.rs"]
mod query_response_roundtrip;
#[path = "../registry_decode_roundtrip.rs"]
mod registry_decode_roundtrip;
#[path = "../registry_excludes_signatory_quorum.rs"]
mod registry_excludes_signatory_quorum;
#[path = "../runtime_doc_sync.rs"]
mod runtime_doc_sync;
#[path = "../signed_block_roundtrip.rs"]
mod signed_block_roundtrip;
#[path = "../sm_norito_roundtrip.rs"]
mod sm_norito_roundtrip;
#[path = "../soracloud_manifest_fixtures.rs"]
mod soracloud_manifest_fixtures;
#[path = "../streaming_events_roundtrip.rs"]
mod streaming_events_roundtrip;
#[path = "../symlink_handling.rs"]
mod symlink_handling;
#[path = "../trait_objects.rs"]
mod trait_objects;
#[path = "../transaction_parameters.rs"]
mod transaction_parameters;
#[path = "../transaction_traits.rs"]
mod transaction_traits;
#[path = "../unregistered_instruction.rs"]
mod unregistered_instruction;
#[path = "../unshield_json_defaults.rs"]
mod unshield_json_defaults;
#[path = "../zk_envelope_roundtrip.rs"]
mod zk_envelope_roundtrip;
