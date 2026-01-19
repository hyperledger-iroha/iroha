//! Data availability ingest handlers and persistence helpers for Torii.

pub mod commitments;
mod ingest;
mod persistence;
pub mod pin_intents;
mod rs16;
mod taikai;

pub use ingest::{
    DaManifestQuery, handler_get_da_manifest, handler_post_da_ingest, ipa_commitment_from_chunks,
};
use iroha_data_model::sorafs::pin_registry::StorageClass;
pub use persistence::{DaReceiptLog, DaReceiptLogEntry, ReceiptInsertOutcome, ReplayCursorStore};
pub use taikai::{compute_taikai_ingest_tags, spawn_anchor_worker};

fn storage_class_label(class: StorageClass) -> &'static str {
    match class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    }
}
