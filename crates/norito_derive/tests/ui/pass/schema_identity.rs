//! Canonical generic and projected identities are independent of payload codecs.
#![deny(single_use_lifetimes)]
use norito::NoritoSchema;
use std::marker::PhantomData;

#[derive(NoritoSchema)]
#[norito_schema(name = "example::Marker")]
struct Marker;

#[derive(NoritoSchema)]
#[norito_schema(name = "example::Envelope")]
struct Envelope<'a, T: ?Sized, const N: usize>(PhantomData<&'a T>);

#[derive(NoritoSchema)]
#[norito_schema(name = "example::Projected", frame = "example.projected")]
enum Projected {
    Value,
}

fn main() {
    assert_eq!(
        Envelope::<'static, Marker, 3>::nominal_name(),
        "example::Envelope<'_, example::Marker, 3>"
    );
    assert_eq!(
        Envelope::<'static, str, 3>::nominal_name(),
        "example::Envelope<'_, str, 3>"
    );
    assert_eq!(Projected::frame_name(), "example.projected");
    let _ = Projected::Value;
}
