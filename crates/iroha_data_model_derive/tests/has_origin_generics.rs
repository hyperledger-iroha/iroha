//! Tests covering `HasOrigin` derive on generic enums.

use iroha_data_model_derive::{HasOrigin, IdEqOrdHash};

/// Minimal `Identifiable` trait for testing
pub trait Identifiable: Ord + Eq {
    /// Identifier type
    type Id: Ord + Eq + core::hash::Hash;

    /// Access identifier
    fn id(&self) -> &Self::Id;
}

/// Minimal `HasOrigin` trait for testing
pub trait HasOrigin {
    /// Origin type
    type Origin: Identifiable;

    /// Identifier of the origin
    fn origin(&self) -> &<Self::Origin as Identifiable>::Id;
}

/// Simple identifier wrapper used in the tests.
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct ObjectId(pub i32);

/// Minimal object type used as an origin.
#[derive(Debug, IdEqOrdHash)]
struct Object {
    id: ObjectId,
}

#[allow(clippy::enum_variant_names)] // it's a test, duh
/// Generic event type used to verify `HasOrigin` derive support.
#[derive(Debug, HasOrigin)]
#[has_origin(origin = Object)]
enum ObjectEvent<T: Identifiable<Id = ObjectId>> {
    EventWithId(ObjectId),
    #[has_origin(event => &event.0)]
    EventWithExtractor((ObjectId, i32)),
    #[has_origin(obj => obj.id())]
    EventWithAnotherExtractor(T),
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
