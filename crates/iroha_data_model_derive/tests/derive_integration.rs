//! Consolidated integration-test harness for data-model derives.
#[cfg(feature = "trybuild-tests")]
#[path = "ui.rs"]
mod ui;

#[path = "data_event_identity.rs"]
mod data_event_identity;
#[path = "event_set.rs"]
mod event_set;
#[path = "has_origin.rs"]
mod has_origin;
#[path = "has_origin_generics.rs"]
mod has_origin_generics;
#[path = "id_eq_ord_hash.rs"]
mod id_eq_ord_hash;
#[path = "model_macro.rs"]
mod model_macro;

#[cfg(feature = "trybuild-tests")]
#[path = "registrable_builder_ui.rs"]
mod registrable_builder_ui;

#[cfg(feature = "trybuild-tests")]
#[path = "event_set_ui.rs"]
mod event_set_ui;
