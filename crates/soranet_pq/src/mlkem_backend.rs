//! FIPS 203 byte operations using the maintained `RustCrypto` ML-KEM implementation.
//!
//! The protocol stores the full FIPS 203 decapsulation key, so its expanded
//! encoding remains intentional even though `RustCrypto` recommends seed storage.
#![allow(deprecated)]

use super::{MlKemError, MlKemSuite};
use ml_kem::{Decapsulate, ExpandedKeyEncoding, KeyExport, TryKeyInit};
use zeroize::Zeroizing;

macro_rules! dispatch {
    ($suite:expr, $operation:ident, $($arg:expr),+ $(,)?) => {
        match $suite {
            MlKemSuite::MlKem512 => mlkem512::$operation($($arg),+),
            MlKemSuite::MlKem768 => mlkem768::$operation($($arg),+),
            MlKemSuite::MlKem1024 => mlkem1024::$operation($($arg),+),
        }
    };
}

pub(super) fn keypair_derand(
    suite: MlKemSuite,
    public: &mut [u8],
    secret: &mut [u8],
    coins: &[u8; 64],
) -> Result<(), MlKemError> {
    dispatch!(suite, keypair, public, secret, coins)
}
pub(super) fn encapsulate_derand(
    suite: MlKemSuite,
    ciphertext: &mut [u8],
    shared: &mut [u8],
    public: &[u8],
    coins: &[u8; 32],
) -> Result<(), MlKemError> {
    dispatch!(suite, encapsulate, ciphertext, shared, public, coins)
}
pub(super) fn decapsulate(
    suite: MlKemSuite,
    shared: &mut [u8],
    ciphertext: &[u8],
    secret: &[u8],
) -> Result<(), MlKemError> {
    dispatch!(suite, decapsulate, shared, ciphertext, secret)
}

macro_rules! implementation {
    ($module:ident, $parameter:ident, $suite:ident) => {
        mod $module {
            use super::*;
            use ml_kem::{DecapsulationKey, EncapsulationKey, $parameter};
            fn failure(operation: &'static str) -> MlKemError {
                MlKemError::BackendFailure {
                    suite: MlKemSuite::$suite,
                    operation,
                }
            }
            pub(super) fn keypair(
                public: &mut [u8],
                secret: &mut [u8],
                coins: &[u8; 64],
            ) -> Result<(), MlKemError> {
                let seed = Zeroizing::new(ml_kem::Seed::from(*coins));
                let key = DecapsulationKey::<$parameter>::from_seed(*seed);
                public.copy_from_slice(key.encapsulation_key().to_bytes().as_slice());
                let expanded = Zeroizing::new(key.to_expanded_bytes());
                secret.copy_from_slice(expanded.as_slice());
                Ok(())
            }
            pub(super) fn encapsulate(
                ciphertext: &mut [u8],
                shared: &mut [u8],
                public: &[u8],
                coins: &[u8; 32],
            ) -> Result<(), MlKemError> {
                let key = EncapsulationKey::<$parameter>::new_from_slice(public)
                    .map_err(|_| failure("encapsulation key decoding"))?;
                let randomness = Zeroizing::new(ml_kem::B32::from(*coins));
                let (ct, shared_key) = key.encapsulate_deterministic(&randomness);
                let shared_key = Zeroizing::new(shared_key);
                ciphertext.copy_from_slice(ct.as_slice());
                shared.copy_from_slice(shared_key.as_slice());
                Ok(())
            }
            pub(super) fn decapsulate(
                shared: &mut [u8],
                ciphertext: &[u8],
                secret: &[u8],
            ) -> Result<(), MlKemError> {
                let expanded = Zeroizing::new(
                    ml_kem::ExpandedDecapsulationKey::<$parameter>::try_from(secret)
                        .map_err(|_| failure("decapsulation key length"))?,
                );
                let key = DecapsulationKey::<$parameter>::from_expanded_bytes(&expanded)
                    .map_err(|_| failure("decapsulation key decoding"))?;
                let ciphertext = ml_kem::Ciphertext::<$parameter>::try_from(ciphertext)
                    .map_err(|_| failure("ciphertext length"))?;
                let shared_key = Zeroizing::new(key.decapsulate(&ciphertext));
                shared.copy_from_slice(shared_key.as_slice());
                Ok(())
            }
        }
    };
}
implementation!(mlkem512, MlKem512, MlKem512);
implementation!(mlkem768, MlKem768, MlKem768);
implementation!(mlkem1024, MlKem1024, MlKem1024);
