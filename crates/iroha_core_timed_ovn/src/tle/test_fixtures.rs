//! Deterministic TLE transcript fixtures for verification tests.
use super::*;
use iroha_crypto::threshold_bls::DasRenDealerSecret;
use rand::{SeedableRng as _, rngs::StdRng};
/// Deterministic development-only TLE fixture component.
#[doc(hidden)]
pub fn binding(byte: u8) -> [u8; 32] {
    [byte; 32]
}

/// Deterministic development-only TLE fixture component.
#[doc(hidden)]
pub struct Fixture {
    /// Fixture component retained for cross-crate assertions.
    pub session: ThresholdBlsSession<TleReleasePurpose>,
    /// Fixture component retained for cross-crate assertions.
    pub validated: ValidatedTleKeySessionV1,
    /// Fixture component retained for cross-crate assertions.
    pub dealer_secrets: Vec<DasRenDealerSecret<TleReleasePurpose>>,
    /// Fixture component retained for cross-crate assertions.
    pub dealers: Vec<ValidatedDealerCommitment<TleReleasePurpose>>,
}

/// Deterministic development-only TLE fixture component.
#[doc(hidden)]
pub fn fixture_for_binding(network_id: [u8; 32], key_byte: u8, roster_hash: [u8; 32]) -> Fixture {
    let session = ThresholdBlsSession::<TleReleasePurpose>::new(
        network_id,
        binding(key_byte),
        roster_hash,
        4,
        2,
    )
    .expect("session");
    let parameters = AdaptiveThresholdBlsParameters::derive(&session).expect("parameters");
    let mut rng = StdRng::from_seed([key_byte.wrapping_add(5); 32]);
    let mut dealer_secrets = Vec::new();
    let mut dealers = Vec::new();
    for index in 1_u16..=3 {
        let (secret, dealer) =
            DasRenDealerSecret::generate_with_rng(&parameters, index, &mut rng).expect("dealer");
        dealer_secrets.push(secret);
        dealers.push(dealer);
    }
    let validated =
        ValidatedTleKeySessionV1::from_qualified_dealers(session, &dealers, &[1, 2, 3], binding(4))
            .expect("validated key session");
    Fixture {
        session,
        validated,
        dealer_secrets,
        dealers,
    }
}

/// Deterministic development-only TLE fixture component.
#[doc(hidden)]
pub fn fixture_for_key(key_byte: u8) -> Fixture {
    fixture_for_binding(binding(1), key_byte, binding(3))
}

/// Build a deterministic public TLE session bound to an exact consensus context.
/// Deterministic development-only TLE fixture component.
#[doc(hidden)]
pub fn public_key_session_fixture_for_context_v1(
    network_id: [u8; 32],
    key_byte: u8,
    roster_hash: [u8; 32],
) -> TleKeySessionPublicStateV1 {
    fixture_for_binding(network_id, key_byte, roster_hash)
        .validated
        .public_state()
        .clone()
}

/// Deterministic development-only TLE fixture component.
#[doc(hidden)]
pub fn fixture() -> Fixture {
    fixture_for_key(2)
}

/// Deterministic development-only TLE fixture component.
#[doc(hidden)]
pub fn identity(session: ThresholdBlsSession<TleReleasePurpose>) -> TleReleaseIdentityV1 {
    TleReleaseIdentityV1::new(
        session,
        binding(10),
        binding(11),
        binding(12),
        binding(13),
        binding(14),
        100,
        binding(15),
    )
    .expect("identity")
}
