//! Captured nominal and independent directional identities for parameter wire owners.
//!
//! Parameter payload, validation and governance controls remain in their owning suites.

/// Decode one immutable independently captured sixteen-byte schema hash.
fn captured_hash(value: &str) -> [u8; 16] {
    hex::decode(value)
        .expect("captured hexadecimal schema hash")
        .try_into()
        .expect("captured schema hash has sixteen bytes")
}

// Each row retains its actual nominal identity and both observed codec directions.
macro_rules! parameter_owners {
    ($check:ident) => {
        $check!(
            super::custom::CustomParameterId,
            "iroha_data_model::parameter::custom::model::CustomParameterId",
            "d62f5dfb58c19805ff8c11c72d32bd77",
            "d62f5dfb58c19805ff8c11c72d32bd77",
        );
        $check!(
            super::custom::CustomParameter,
            "iroha_data_model::parameter::custom::model::CustomParameter",
            "8352c5f5a8f1dc1cd3577b52d0da2e90",
            "8352c5f5a8f1dc1cd3577b52d0da2e90",
        );
        $check!(
            super::system::ConsensusFingerprint,
            "iroha_data_model::parameter::system::ConsensusFingerprint",
            "4f219bd2d074bac5fb16250d0717b17c",
            "4f219bd2d074bac5fb16250d0717b17c",
        );
        $check!(
            super::system::KagemushaMintFinalityNextEpochParameterV1,
            "iroha_data_model::parameter::system::KagemushaMintFinalityNextEpochParameterV1",
            "9819b19e8a3e5f3735d92cd2822fb76e",
            "9819b19e8a3e5f3735d92cd2822fb76e",
        );
        $check!(
            super::system::ConsensusHandshakeMetadata,
            "iroha_data_model::parameter::system::ConsensusHandshakeMetadata",
            "847eca5152f46ee278ecc4fb1e64d7fd",
            "847eca5152f46ee278ecc4fb1e64d7fd",
        );
        $check!(
            super::system::SumeragiConsensusMode,
            "iroha_data_model::parameter::system::model::SumeragiConsensusMode",
            "871cda753b21fdf37553e46af3668304",
            "871cda753b21fdf37553e46af3668304",
        );
        $check!(
            super::system::SumeragiParameters,
            "iroha_data_model::parameter::system::model::SumeragiParameters",
            "9b9025f7ce84cd64f0a92cce4ecedbd7",
            "9b9025f7ce84cd64f0a92cce4ecedbd7",
        );
        $check!(
            super::system::SumeragiNposParameters,
            "iroha_data_model::parameter::system::model::SumeragiNposParameters",
            "16e5c6ffcb4b476a28952c49a30800c4",
            "16e5c6ffcb4b476a28952c49a30800c4",
        );
        $check!(
            super::system::SumeragiParameter,
            "iroha_data_model::parameter::system::model::SumeragiParameter",
            "7aa4400a92c2d945ddf7d6a0d89fb55c",
            "7aa4400a92c2d945ddf7d6a0d89fb55c",
        );
        $check!(
            super::system::BlockParameters,
            "iroha_data_model::parameter::system::model::BlockParameters",
            "678b0cea38c3007462ba5e023d96cad7",
            "678b0cea38c3007462ba5e023d96cad7",
        );
        $check!(
            super::system::BlockParameter,
            "iroha_data_model::parameter::system::model::BlockParameter",
            "f57233be6d9309bbb8b80c01cade3611",
            "f57233be6d9309bbb8b80c01cade3611",
        );
        $check!(
            super::system::TransactionParameters,
            "iroha_data_model::parameter::system::model::TransactionParameters",
            "46c131f3c9519197f46273807a5817ca",
            "46c131f3c9519197f46273807a5817ca",
        );
        $check!(
            super::system::TransactionParameter,
            "iroha_data_model::parameter::system::model::TransactionParameter",
            "b0fcf05781f9c34b12472390ef1d4ed9",
            "b0fcf05781f9c34b12472390ef1d4ed9",
        );
        $check!(
            super::system::SmartContractParameters,
            "iroha_data_model::parameter::system::model::SmartContractParameters",
            "f21d11996812fedc217cf06ab7cba4c1",
            "f21d11996812fedc217cf06ab7cba4c1",
        );
        $check!(
            super::system::SmartContractParameter,
            "iroha_data_model::parameter::system::model::SmartContractParameter",
            "9342e172ce36fc7c9350acfed1b90ffa",
            "9342e172ce36fc7c9350acfed1b90ffa",
        );
        $check!(
            super::system::Parameters,
            "iroha_data_model::parameter::system::model::Parameters",
            "ddabf9d1bf208d08a8502c8f7d08b2aa",
            "ddabf9d1bf208d08a8502c8f7d08b2aa",
        );
        $check!(
            super::system::Parameter,
            "iroha_data_model::parameter::system::model::Parameter",
            "11e49b10ef18bd3bf4bdb469b6e0d7ab",
            "11e49b10ef18bd3bf4bdb469b6e0d7ab",
        );
    };
}

#[test]
fn captured_parameter_nominal_identities() {
    macro_rules! check {
        ($owner:ty, $nominal:literal, $serialize:literal, $deserialize:literal,) => {
            assert_eq!(<$owner as norito::NoritoSchema>::nominal_name(), $nominal);
            assert_eq!(<$owner as norito::NoritoSchema>::frame_name(), $nominal);
        };
    }
    parameter_owners!(check);
}

#[test]
fn captured_parameter_serialize_hashes() {
    macro_rules! check {
        ($owner:ty, $nominal:literal, $serialize:literal, $deserialize:literal,) => {
            assert_eq!(
                norito::schema::identity::frame_hash::<$owner>(),
                captured_hash($serialize)
            );
            assert_eq!(
                norito::schema::identity::frame_hash::<$owner>(),
                captured_hash($serialize)
            );
        };
    }
    parameter_owners!(check);
}

#[test]
fn captured_parameter_deserialize_hashes() {
    macro_rules! check {
        ($owner:ty, $nominal:literal, $serialize:literal, $deserialize:literal,) => {
            assert_eq!(
                norito::schema::identity::frame_hash::<$owner>(),
                captured_hash($deserialize)
            );
            assert_eq!(
                norito::schema::identity::frame_hash::<$owner>(),
                captured_hash($deserialize)
            );
        };
    }
    parameter_owners!(check);
}
