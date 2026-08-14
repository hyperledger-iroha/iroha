#[test]
fn quantity_to_nanos_checked_rejects_sub_nano_precision() {
    let exact = "1.000000001"
        .parse::<Quantity>()
        .expect("canonical nano quantity");
    assert_eq!(quantity_to_nanos_checked(&exact), Ok(1_000_000_001));
    let inexact = "0.0000000001"
        .parse::<Quantity>()
        .expect("canonical sub-nano quantity");
    assert_eq!(
        quantity_to_nanos_checked(&inexact),
        Err(QuantityToNanosError::InexactNanos)
    );
}
struct FailingSorafsCliNonceRng;
#[derive(Debug)]
struct FailingSorafsCliNonceRngError;
impl fmt::Display for FailingSorafsCliNonceRngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failing SoraFS CLI nonce RNG")
    }
}
impl TryRngCore for FailingSorafsCliNonceRng {
    type Error = FailingSorafsCliNonceRngError;
    fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
        Err(FailingSorafsCliNonceRngError)
    }
    fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
        Err(FailingSorafsCliNonceRngError)
    }
    fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> std::result::Result<(), Self::Error> {
        Err(FailingSorafsCliNonceRngError)
    }
}
impl TryCryptoRng for FailingSorafsCliNonceRng {}
#[test]
fn parse_receipt_id_reports_rng_failure() {
    let mut rng = FailingSorafsCliNonceRng;
    let error = parse_receipt_id_with_rng(None, &mut rng)
        .expect_err("receipt-id generation should fail when entropy fails");
    let message = format!("{error:?}");
    assert!(message.contains("SoraFS receipt-id OS RNG failed"));
    assert!(message.contains("failing SoraFS CLI nonce RNG"));
}
#[test]
fn generate_nonce_hex_reports_rng_failure() {
    let mut rng = FailingSorafsCliNonceRng;
    let error = generate_nonce_hex_with_rng(12, &mut rng)
        .expect_err("nonce generation should fail when entropy fails");
    let message = format!("{error:?}");
    assert!(message.contains("SoraFS CLI nonce OS RNG failed"));
    assert!(message.contains("failing SoraFS CLI nonce RNG"));
}
