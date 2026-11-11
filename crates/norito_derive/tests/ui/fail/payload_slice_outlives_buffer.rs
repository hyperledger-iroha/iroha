//! Payload bytes borrowed through a marker live exactly as long as the marker,
//! so they cannot escape the buffer the marker was taken from.
#![forbid(unsafe_code)]
fn main() {
    let dangling: &'static [u8] = {
        let buffer = vec![1_u8, 2, 3];
        norito::core::with_payload_ctx(&buffer, || {
            let archived = norito::core::archived_from_slice::<u8>(&buffer).unwrap();
            norito::core::payload_slice(archived.archived()).unwrap()
        })
    };
    assert_eq!(dangling, [1, 2, 3]);
}
