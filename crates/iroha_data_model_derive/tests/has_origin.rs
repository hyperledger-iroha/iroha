//! Tests for the `HasOrigin` derive helper.

use iroha_data_model_derive::{HasOrigin, IdEqOrdHash};

/// Minimal `Identifiable` trait needed for the tests
///
/// This is intentionally simplified to avoid depending on the
/// `iroha_data_model` crate.
pub trait Identifiable: Ord + Eq {
    /// Identifier type
    type Id: Ord + Eq + core::hash::Hash;

    /// Access identifier
    fn id(&self) -> &Self::Id;
}

/// Minimal `HasOrigin` trait for the tests
pub trait HasOrigin {
    /// Origin type which must be identifiable
    type Origin: Identifiable;

    /// Identifier of the origin
    fn origin(&self) -> &<Self::Origin as Identifiable>::Id;
}

/// Identifier used in test objects.
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct ObjectId(pub i32);

/// Minimal struct that derives `IdEqOrdHash` for testing.
#[derive(Debug, IdEqOrdHash)]
struct Object {
    id: ObjectId,
}

impl Object {
    fn id(&self) -> &ObjectId {
        &self.id
    }
}

#[allow(clippy::enum_variant_names)] // it's a test, duh
/// Enum used to validate the `HasOrigin` derive macro.
#[derive(Debug, HasOrigin)]
#[has_origin(origin = Object)]
enum ObjectEvent {
    EventWithId(ObjectId),
    #[has_origin(event => &event.0)]
    EventWithExtractor((ObjectId, i32)),
    #[has_origin(obj => obj.id())]
    EventWithAnotherExtractor(Object),
}

#[test]
fn has_origin() {
    let events = vec![
        ObjectEvent::EventWithId(ObjectId(1)),
        ObjectEvent::EventWithExtractor((ObjectId(2), 2)),
        ObjectEvent::EventWithAnotherExtractor(Object { id: ObjectId(3) }),
    ];
    let expected_ids = vec![ObjectId(1), ObjectId(2), ObjectId(3)];

    for (event, expected_id) in events.into_iter().zip(expected_ids) {
        assert_eq!(
            event.origin(),
            &expected_id,
            "mismatched origin id for event {event:?}",
        );
    }
}
