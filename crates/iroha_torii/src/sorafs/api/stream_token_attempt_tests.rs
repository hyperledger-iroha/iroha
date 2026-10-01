//! Each physical serving attempt receives a fresh identity; entropy failures remain closed.
use super::*;
use rand::rand_core::TryRngCore;

struct EntropyProbe {
    byte: Option<u8>,
    calls: usize,
    bytes: usize,
}
impl TryRngCore for EntropyProbe {
    type Error = std::io::Error;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0; 4];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0; 8];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.calls += 1;
        self.bytes += bytes.len();
        let value = self
            .byte
            .ok_or_else(|| std::io::Error::other("entropy unavailable"))?;
        bytes.fill(value);
        Ok(())
    }
}
impl TryCryptoRng for EntropyProbe {}

#[test]
fn serving_attempt_constructor_uses_distinct_nonzero_os_random_identities() {
    let first = new_serving_attempt_id().expect("first physical serving attempt");
    let second = new_serving_attempt_id().expect("independent physical serving attempt");
    assert_ne!(first, [0; 32]);
    assert_ne!(second, [0; 32]);
    assert_ne!(first, second);
}

#[test]
fn serving_attempt_entropy_is_sampled_once_and_never_replaced_after_failure() {
    for byte in [Some(0x51), Some(0), None] {
        let mut rng = EntropyProbe {
            byte,
            calls: 0,
            bytes: 0,
        };
        let result = serving_attempt_id_with_rng(&mut rng);
        assert_eq!(rng.calls, 1);
        assert_eq!(rng.bytes, 32);
        if byte == Some(0x51) {
            assert_eq!(result, Ok([0x51; 32]));
        } else {
            assert_eq!(result, Err(StreamTokenGatewayAdmissionErrorV1::Unavailable));
        }
    }
}
