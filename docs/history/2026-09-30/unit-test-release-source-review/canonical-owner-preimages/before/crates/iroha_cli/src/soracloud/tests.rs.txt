use super::*;
use iroha::data_model::soracloud::{
    SECRET_ENVELOPE_VERSION_V1, SecretEnvelopeEncryptionV1, SoraInrouGuestIsaV1,
    SoraServiceExecutionPlaneV1,
};
use iroha_crypto::Algorithm;
use iroha_version::codec::EncodeVersioned as _;
use norito::json::Value;
use rand::rand_core::{TryCryptoRng, TryRngCore};

include!("tests/part_01.rs");
include!("tests/part_02.rs");
include!("tests/part_03.rs");
include!("tests/part_04.rs");
include!("tests/part_05.rs");
include!("tests/part_06.rs");
