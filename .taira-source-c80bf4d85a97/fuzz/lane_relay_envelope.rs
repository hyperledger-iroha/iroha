#![no_main]

use iroha_data_model::nexus::relay::LaneRelayEnvelope;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut cursor = data;
    if let Ok(envelope) = LaneRelayEnvelope::decode_all(&mut cursor) {
        let _ = envelope.verify();
    }
});
