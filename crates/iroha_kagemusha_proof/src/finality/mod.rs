//! Circuit components for ordinary validator-finalized Load receipts.
//!
//! Receipt parsing and inclusion are only parts of the finality relation. They
//! grant no Load authority without a source-qualified proof of the genesis-rooted
//! validator schedule, exact quorum certificate and certified event commitment.
//!
//! TODO: qualify the complete original-key producer through signed genesis,
//! receipt finality, the five-stage Load consumer and the compact wallet catalog.

mod receipt;
pub use receipt::LoadReceiptCells;

mod event;
pub use event::LoadEventCells;

pub mod result;

pub mod result_scan;

pub mod load_source;

pub mod consensus;

pub mod certificate;

pub mod certified_result;

pub mod scheduled_result;

pub mod history;

pub mod native;

pub mod catalog;

pub mod receipt_finality;

pub mod roster;

pub mod aggregate;

pub mod continuity;

pub mod schedule;

pub mod bls;
